use glam::IVec3;
use rustc_hash::FxHashSet;

use crate::{
    terrain::{
        block::{BlockId, BLOCKS},
        chunk::{
            block_store::ChunkBlockStore, light_store, side::ChunkSideLight, CHUNK_SIZE,
            CHUNK_SIZE_I32, CHUNK_SIZE_LOG2,
        },
        position_types::LocalBlockPosition,
    },
    util::face::{FaceIndex, FACE_BITANGENTS, FACE_NORMALS, FACE_TANGENTS},
};

use super::{
    LightPropagationQueue, LightPropagationStep, LightStore, LightUpdate, LightUpdatesOutsideChunk,
    ShadowPropagationQueue, ShadowPropagationStep,
};

/// Propagate emitted light within a chunk, returning the light updates to be
/// applied outside of the chunk
pub fn propagate_emitted_light<Store: LightStore<EmittedLight>>(
    light_store: &mut Store,
    light_propagation_queue: &mut LightPropagationQueue<EmittedLight>,
    light_updates_outside_chunk: &mut LightUpdatesOutsideChunk,
    blocks: &ChunkBlockStore,
) {
    while let Some(step) = light_propagation_queue.pop_front() {
        // calculate new light value
        let light_old = light_store.read(step.position);
        let light_new = EmittedLight::max(light_old, step.light);

        // stop propagating if the new light value is less than or equal to existing light value
        // AND this propagation step wasn't queued by the shadow propagation to repair light areas
        // damaged by shadow
        if EmittedLight::less(light_old, light_new) == 0 && !step.is_repair_step {
            continue;
        }

        // update light value
        light_store.write(step.position, light_new);

        // calculate new light value to propagate to neighbours
        let light_diminished = step.light.decrement_and_saturate();
        if light_diminished == EmittedLight::ZERO {
            continue;
        }

        // Propagate light to neighbours
        for (face_index, neighbour_offset) in FACE_NORMALS.iter().enumerate() {
            if let Some(neighbour_pos) = step.position.try_add(*neighbour_offset) {
                // work out if the light can pass into the neighbouring block
                let neighbour_block_id = blocks.get_block(neighbour_pos);
                let neighbour_block = &BLOCKS[neighbour_block_id.as_usize()];
                let existing_light_value = light_store.read(neighbour_pos);

                let can_travel_into_block = neighbour_block
                    .model
                    .is_transparent_in_direction(FaceIndex(face_index).opposite());

                // work out if propagating light to the neighbouring block would increase the light
                // value in that block
                let would_increase_light =
                    EmittedLight::less(existing_light_value, light_diminished) != 0;

                if can_travel_into_block && would_increase_light {
                    light_propagation_queue.push_back(LightPropagationStep {
                        position: neighbour_pos,
                        light: light_diminished,
                        is_repair_step: false,
                    });
                }
            } else {
                // send light update to neighbouring chunk
                let pos_in_neighbour_chunk = step.position.wrapping_add(*neighbour_offset);

                light_updates_outside_chunk.push((
                    FaceIndex(face_index),
                    LightUpdate::EmittedLight(LightPropagationStep {
                        position: pos_in_neighbour_chunk,
                        light: light_diminished,
                        is_repair_step: false,
                    }),
                ))
            }
        }
    }
}

/// Propagate absense of emitted light within a chunk, returning the shadow
/// updates to be applied outside of the chunk
pub fn propagate_emitted_light_shadow<Store: LightStore<EmittedLight>>(
    light_store: &mut Store,
    shadow_propagation_queue: &mut ShadowPropagationQueue,
    light_propagation_queue: &mut LightPropagationQueue<EmittedLight>,
    light_updates_outside_chunk: &mut LightUpdatesOutsideChunk,
    blocks: &ChunkBlockStore,
) {
    let mut visited = FxHashSet::default();

    while let Some(step) = shadow_propagation_queue.pop_front() {
        visited.insert(step.position);

        if step.depth == 0 {
            // Repair lighting by re-queueing the light at the edge of the shadow
            // for propagation
            let light = light_store.read(step.position);

            if light != EmittedLight::ZERO {
                light_propagation_queue.push_back(LightPropagationStep {
                    position: step.position,
                    light,
                    is_repair_step: true,
                });
            }

            continue;
        }

        light_store.write(step.position, EmittedLight::ZERO);

        // Repair lighting by re-queueing any light emitting blocks encompassed in the shadow for
        // propagation
        let block_id = blocks.get_block(step.position);
        let block = &BLOCKS[block_id.as_usize()];
        if block.emission != IVec3::ZERO {
            light_propagation_queue.push_back(LightPropagationStep {
                position: step.position,
                light: EmittedLight::from_ivec3(block.emission),
                is_repair_step: true,
            });
        }
        // Propagate shadow to neighbours
        for (neighbour_index, neighbour_offset) in FACE_NORMALS.iter().enumerate() {
            if let Some(neighbour_pos) = step.position.try_add(*neighbour_offset) {
                if visited.insert(neighbour_pos) {
                    shadow_propagation_queue.push_back(ShadowPropagationStep {
                        position: neighbour_pos,
                        depth: step.depth - 1,
                    });
                }
            } else {
                let pos_in_neighbour_chunk = step.position.wrapping_add(*neighbour_offset);

                light_updates_outside_chunk.push((
                    FaceIndex(neighbour_index),
                    LightUpdate::EmittedLightShadow(ShadowPropagationStep {
                        position: pos_in_neighbour_chunk,
                        depth: step.depth - 1,
                    }),
                ))
            }
        }
    }
}

/// Returns a LightPropagationQueue for all of the light emitting blocks in the block array
pub fn get_initial_emitted_light_queue(
    blocks: &[BlockId],
    surrounding_sides_light: &[Option<ChunkSideLight>],
) -> LightPropagationQueue<EmittedLight> {
    // blocks within chunk
    let mut light_queue: LightPropagationQueue<EmittedLight> = blocks
        .iter()
        .enumerate()
        .filter_map(|(block_index, block_id)| {
            let block = &BLOCKS[block_id.0 as usize];

            if block.emission != IVec3::ZERO {
                Some(LightPropagationStep {
                    position: LocalBlockPosition::from_array_index(block_index),
                    light: EmittedLight::from_ivec3(block.emission),
                    is_repair_step: false,
                })
            } else {
                None
            }
        })
        .collect();

    // blocks in neighbouring chunks
    light_queue.extend(
        surrounding_sides_light
            .iter()
            .enumerate()
            .filter_map(|(side_index, side_opt)| side_opt.as_ref().map(|side| (side_index, side)))
            .map(move |(side_index, side)| {
                side.emitted
                    .iter()
                    .cloned()
                    .enumerate()
                    .filter(|(_, light)| light.decrement_and_saturate() != EmittedLight::ZERO)
                    .map(move |(index_in_side, light)| {
                        let u = index_in_side & (CHUNK_SIZE - 1);
                        let v = index_in_side >> CHUNK_SIZE_LOG2;

                        let position = LocalBlockPosition::ZERO.wrapping_add(
                            FACE_NORMALS[side_index].max(IVec3::ZERO) * (CHUNK_SIZE_I32 - 1)
                                + FACE_TANGENTS[side_index].abs() * u as i32
                                + FACE_BITANGENTS[side_index].abs() * v as i32,
                        );

                        LightPropagationStep {
                            position,
                            light: light.decrement_and_saturate(),
                            is_repair_step: false,
                        }
                    })
                    .filter(move |step| {
                        let block_id = blocks[step.position.get_array_index()];
                        let block = &BLOCKS[block_id.as_usize()];

                        block
                            .model
                            .is_transparent_in_direction(FaceIndex(side_index))
                    })
            })
            .flatten(),
    );

    light_queue
}

/// Emitted light values for one block packed in 16 bits
/// Bits 0 to 4   | Red component
/// Bits 4 to 8   | Green component
/// Bits 8 to 12  | Blue component
/// Bits 12 to 16 | Unused (can store skylight!)
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EmittedLight(u16);

impl EmittedLight {
    pub const ZERO: EmittedLight = EmittedLight(0);
    pub const MAX_VALUE: u32 = 15;

    const COMPONENT_MASK: u16 = 0x0f0f;
    const BORROW_GUARD: u16 = 0x2020;
    const CARRY_MASK: u16 = 0x1010;

    /// Wrap a u16 storing the 3 light values in an `EmittedLight`
    pub fn from_u16(value: u16) -> Self {
        Self(value)
    }

    /// Created a packed `EmittedLight` value from the 3 light values
    /// Values must be in 0..16
    pub fn from_rgb(r: u16, g: u16, b: u16) -> Self {
        debug_assert!((0..16).contains(&r));
        debug_assert!((0..16).contains(&g));
        debug_assert!((0..16).contains(&b));

        Self(r | g << 4 | b << 8)
    }

    /// Create a packed `EmittedLight` from the 3 light values
    pub fn from_ivec3(rgb: IVec3) -> Self {
        Self::from_rgb(rgb.x as u16, rgb.y as u16, rgb.z as u16)
    }

    /// Returns the underlying u16 storing the 3 light values
    pub fn as_u16(&self) -> u16 {
        self.0
    }

    /// Returns the individual RGB light values represented by this packed `EmittedLight` value
    /// Values are in 0..16
    pub fn as_rgb(&self) -> (u16, u16, u16) {
        ((self.0 >> 0) & 15, (self.0 >> 4) & 15, (self.0 >> 8) & 15)
    }

    /// Pair-wise < operation
    /// Compare two sets of light values and determine which components are < the other
    pub fn less(a: Self, b: Self) -> u16 {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)
        Self::half_less(a.0, b.0) | (Self::half_less(a.0 >> 4, b.0 >> 4) << 4)
    }

    /// Pair-wise `max` operation
    pub fn max(a: Self, b: Self) -> Self {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)
        let result = a.0 ^ ((a.0 ^ b.0) & Self::less(a, b));
        Self(result)
    }

    /// Subtract one from each component, saturating on underflow
    pub fn decrement_and_saturate(&self) -> EmittedLight {
        let result = Self::decrement_and_saturate_half(self.0)
            | (Self::decrement_and_saturate_half(self.0 >> 4) << 4);
        Self(result)
    }

    fn half_less(a: u16, b: u16) -> u16 {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)
        let d = (((a & Self::COMPONENT_MASK) | Self::BORROW_GUARD) - (b & Self::COMPONENT_MASK))
            & Self::CARRY_MASK;
        (d >> 1) | (d >> 2) | (d >> 3) | (d >> 4)
    }

    fn decrement_and_saturate_half(x: u16) -> u16 {
        // https://0fps.net/2018/02/21/voxel-lighting/ (I did not invent this)

        // compute component-wise decrement
        let d = ((x & Self::COMPONENT_MASK) | Self::BORROW_GUARD) - 0x0101;

        // check for underflow
        let b = d & Self::CARRY_MASK;

        // saturate underflowed values
        (d + (b >> 4)) & Self::COMPONENT_MASK
    }
}

#[cfg(test)]
mod tests {
    use super::EmittedLight;

    #[test]
    fn emitted_light_from_and_as_rgb() {
        let r = 1;
        let g = 2;
        let b = 3;
        let emitted_light = EmittedLight::from_rgb(r, g, b);
        assert_eq!((r, g, b), emitted_light.as_rgb())
    }

    #[test]
    fn emitted_light_less() {
        {
            let a = EmittedLight::from_rgb(1, 0, 1);
            let b = EmittedLight::from_rgb(0, 1, 0);
            assert_eq!(0b0000_1111_0000, EmittedLight::less(a, b));
        }

        {
            let a = EmittedLight::from_rgb(0, 0, 0);
            let b = EmittedLight::from_rgb(15, 15, 15);
            assert_eq!(0b1111_1111_1111, EmittedLight::less(a, b));
        }

        {
            let a = EmittedLight::from_rgb(0, 1, 2);
            let b = EmittedLight::from_rgb(0, 1, 2);
            assert_eq!(0b0000_0000_0000, EmittedLight::less(a, b));
        }
    }

    #[test]
    fn emitted_light_max() {
        let a = EmittedLight::from_rgb(15, 10, 5);
        let b = EmittedLight::from_rgb(5, 10, 15);
        assert_eq!((15, 10, 15), EmittedLight::max(a, b).as_rgb())
    }

    #[test]
    fn emitted_light_decrement_and_saturate() {
        let x = EmittedLight::from_rgb(0, 1, 2);
        assert_eq!((0, 0, 1), x.decrement_and_saturate().as_rgb())
    }
}
