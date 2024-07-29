use glam::IVec3;

use crate::{
    terrain::{
        block::{BlockId, BLOCKS},
        chunk::{
            block_store::ChunkBlockStore, side::ChunkSideLight, CHUNK_SIZE, CHUNK_SIZE_I32,
            CHUNK_SIZE_LOG2, CHUNK_SIZE_SQUARED,
        },
        lighting::skylight,
        position_types::LocalBlockPosition,
    },
    util::face::{FaceIndex, FACE_BITANGENTS, FACE_NORMALS, FACE_TANGENTS},
};

use super::{
    LightPropagationQueue, LightPropagationStep, LightStore, LightUpdate, LightUpdatesOutsideChunk,
};

/// Propagate skylight within a chunk, returning the light updates to be
/// applied outside of the chunk
pub fn propagate_skylight<Store: LightStore<Skylight>>(
    light_store: &mut Store,
    light_propagation_queue: &mut LightPropagationQueue<Skylight>,
    light_updates_outside_chunk: &mut LightUpdatesOutsideChunk,
    blocks: &ChunkBlockStore,
) {
    while let Some(step) = light_propagation_queue.pop_front() {
        // calculate new light value
        let light_old = light_store.read(step.position);
        let light_new = step.light.max(light_old);

        // stop propagating if the new light value is less than or equal to existing light value
        // AND this propagation step wasn't queued by the shadow propagation to repair light areas
        // damaged by shadow
        if step.light <= light_old && !step.is_repair_step {
            continue;
        }

        // update light value
        light_store.write(step.position, light_new);

        for (face_index, neighbour_offset) in FACE_NORMALS.iter().enumerate() {
            // calculate new light value to propagate to neighbours
            let light_diminished = if FaceIndex(face_index) == FaceIndex::NEG_Y {
                // Propagate infinitely downwards
                step.light
            } else {
                step.light.decrement_and_saturate()
            };

            if light_diminished == Skylight::ZERO {
                continue;
            }

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
                let would_increase_light = existing_light_value < light_diminished;

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
                    LightUpdate::Skylight(LightPropagationStep {
                        position: pos_in_neighbour_chunk,
                        light: light_diminished,
                        is_repair_step: false,
                    }),
                ))
            }
        }
    }
}

/// Returns a LightPropagationQueue for all of the blocks on the sides of the chunk into which
/// skylight can propagate from neighbouring chunks
pub fn get_initial_skylight_queue(
    blocks: &[BlockId],
    surrounding_sides_light: &[Option<ChunkSideLight>],
) -> LightPropagationQueue<Skylight> {
    fn add_light_values_for_side(
        result: &mut LightPropagationQueue<Skylight>,
        blocks: &[BlockId],
        side_index: usize,
        get_light_fn: impl Fn(usize) -> Skylight,
    ) {
        for index_in_side in 0..CHUNK_SIZE_SQUARED {
            let light = get_light_fn(index_in_side);
            if light == Skylight::ZERO {
                continue;
            }

            let u = index_in_side & (CHUNK_SIZE - 1);
            let v = index_in_side >> CHUNK_SIZE_LOG2;

            let position = LocalBlockPosition::ZERO.wrapping_add(
                FACE_NORMALS[side_index].max(IVec3::ZERO) * (CHUNK_SIZE_I32 - 1)
                    + FACE_TANGENTS[side_index].abs() * u as i32
                    + FACE_BITANGENTS[side_index].abs() * v as i32,
            );

            let block_id = blocks[position.get_array_index()];
            let block = &BLOCKS[block_id.as_usize()];

            if !block
                .model
                .is_transparent_in_direction(FaceIndex(side_index))
            {
                continue;
            }

            result.push_back(LightPropagationStep {
                position,
                light,
                is_repair_step: false,
            });
        }
    }

    let mut result = LightPropagationQueue::new();

    for (side_index, side_opt) in surrounding_sides_light.iter().enumerate() {
        if FaceIndex(side_index) == FaceIndex::POS_Y {
            if let Some(side) = side_opt {
                add_light_values_for_side(&mut result, blocks, side_index, |index_in_side| {
                    side.sky[index_in_side]
                })
            } else {
                add_light_values_for_side(&mut result, blocks, side_index, |_| {
                    Skylight(Skylight::MAX_VALUE)
                })
            }
        } else {
            if let Some(side) = side_opt {
                add_light_values_for_side(&mut result, blocks, side_index, |index_in_side| {
                    side.sky[index_in_side].decrement_and_saturate()
                })
            }
        }
    }

    result
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd, Ord)]
pub struct Skylight(pub u8);

impl Skylight {
    pub const ZERO: Skylight = Skylight(0);
    pub const MAX_VALUE: u8 = 15;

    pub fn decrement_and_saturate(self) -> Self {
        if self.0 == 0 {
            Self(0)
        } else {
            Self(self.0 - 1)
        }
    }
}
