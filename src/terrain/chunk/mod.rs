use glam::{IVec3, Vec3};

use self::{
    block_store::ChunkBlockStore, connections::ChunkConnections, light_store::ChunkLightStore,
    side::ChunkSideLight,
};
use super::{
    block::{BlockId, BLOCKS, BLOCK_AIR},
    lighting::{
        emitted_light::{
            get_initial_emitted_light_queue, propagate_emitted_light,
            propagate_emitted_light_shadow, EmittedLight,
        },
        skylight::{get_initial_skylight_queue, propagate_skylight, Skylight},
        LightPropagationQueue, LightUpdate, LightUpdatesOutsideChunk, ShadowPropagationQueue,
        ShadowPropagationStep,
    },
    position_types::{ChunkPosition, LocalBlockPosition},
};
use crate::util::{
    face::FaceIndex,
    size::{Size2, Size3},
    vector_map::VectorMapExt,
};

pub mod block_store;
pub mod connections;
pub mod light_store;
pub mod side;

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_SIZE_2D: Size2 = Size2::splat(CHUNK_SIZE);
pub const CHUNK_SIZE_3D: Size3 = Size3::splat(CHUNK_SIZE);
pub const CHUNK_SIZE_LOG2: usize = 5;
pub const CHUNK_SIZE_SQUARED: usize = (CHUNK_SIZE_U32 * CHUNK_SIZE_U32) as usize;
pub const CHUNK_SIZE_CUBED: usize = (CHUNK_SIZE_U32 * CHUNK_SIZE_U32 * CHUNK_SIZE_U32) as usize;
pub const CHUNK_SIZE_U32: u32 = CHUNK_SIZE as u32;
pub const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;
pub const CHUNK_SIZE_RECIP: f32 = 1.0 / (CHUNK_SIZE as f32);

#[derive(Clone, Debug)]
pub struct Chunk {
    block_store: ChunkBlockStore,
    light_store: ChunkLightStore,
    emitted_light_queue: LightPropagationQueue<EmittedLight>,
    emitted_light_shadow_queue: ShadowPropagationQueue,
    skylight_queue: LightPropagationQueue<Skylight>,
    position: ChunkPosition,
    connections: ChunkConnections,
}

impl Chunk {
    pub fn new(position: ChunkPosition, blocks: &[BlockId]) -> Self {
        // this function is called from a parallel thread so it's OK to perform intensive tasks
        // here
        let block_store = ChunkBlockStore::new(blocks);
        let connections = ChunkConnections::compute(blocks);
        let light_store = ChunkLightStore::new();
        let emitted_light_queue = LightPropagationQueue::new();
        let emitted_light_shadow_queue = ShadowPropagationQueue::new();
        let skylight_queue = LightPropagationQueue::new();

        Self {
            block_store,
            light_store,
            emitted_light_queue,
            emitted_light_shadow_queue,
            skylight_queue,
            position,
            connections,
        }
    }

    /// Returns the underlying block storage
    pub fn block_store(&self) -> &ChunkBlockStore {
        &self.block_store
    }

    /// Returns the underlying light storage
    pub fn light_store(&self) -> &ChunkLightStore {
        &self.light_store
    }

    /// Returns the block ID at the given position.
    /// Panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPosition) -> BlockId {
        self.block_store.get_block(pos)
    }

    /// Update the block ID at the given position and perform light updates
    /// Panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPosition, new_id: BlockId) {
        let old_id = self.block_store.get_block(pos);
        if new_id == old_id {
            return;
        }

        self.block_store.set_block(pos, new_id);

        // Update emitted light shadow propagation queue
        self.emitted_light_shadow_queue
            .push_back(ShadowPropagationStep {
                position: pos,
                depth: EmittedLight::MAX_VALUE,
            });
    }

    /// Returns this chunk's position
    pub fn position(&self) -> ChunkPosition {
        self.position
    }

    /// Returns the computed connections for this chunk
    pub fn connections(&self) -> ChunkConnections {
        self.connections
    }

    /// Marches through the chunk along the ray with the given origin and direction, using the DDA
    /// algorithm
    /// If a block was hit, returns the position of that block in the chunk and face index of the
    /// hit face
    pub fn raymarch(
        &self,
        ray_origin: Vec3,
        ray_direction: Vec3,
        previous_chunk_pos: Option<ChunkPosition>,
        maximum_distance: f32,
    ) -> Option<ChunkHit> {
        pub const EPS: f32 = 1e-3;

        let dir_step = ray_direction.map(|component| if component >= 0.0 { 1.0 } else { 0.0 });
        let dir_recip = ray_direction.recip();

        let mut t = 0.0;
        let mut previous_block_pos: Option<LocalBlockPosition> = None;

        while t < maximum_distance {
            let ray_pos = ray_origin + ray_direction * t;
            let block_pos = ray_pos.floor().as_ivec3();

            if !Size3::splat(CHUNK_SIZE).contains_ivec3(block_pos) {
                // escaped chunk; no intersection
                return None;
            }

            let block_pos = LocalBlockPosition::from(block_pos.as_uvec3());
            if self.get_block(block_pos) != BLOCK_AIR {
                // hit a block
                return Some(ChunkHit {
                    local_hit_pos: block_pos,
                    hit_normal: previous_block_pos
                        .map(|previous_block_pos| {
                            previous_block_pos.as_ivec3() - block_pos.as_ivec3()
                        })
                        .or_else(|| {
                            previous_chunk_pos.map(|previous_chunk_pos| {
                                previous_chunk_pos.as_ivec3() - self.position().as_ivec3()
                            })
                        }),
                });
            }

            // advance to the next block position
            let deltas = (dir_step - ray_pos.fract_gl()) * dir_recip;
            t += deltas.min_element().max(EPS);

            previous_block_pos = Some(block_pos);
        }

        None
    }

    /// True if the chunk has pending light updates
    pub fn requires_light_updates(&self) -> bool {
        [
            self.emitted_light_queue.len(),
            self.emitted_light_shadow_queue.len(),
            self.skylight_queue.len(),
        ]
        .into_iter()
        .any(|len| len == 0)
    }

    /// Setup the emitted light queue for a new chunk
    pub fn fill_emitted_light_queue(&mut self, surrounding_sides_light: &[Option<ChunkSideLight>]) {
        self.emitted_light_queue = get_initial_emitted_light_queue(
            &self.block_store.as_block_array(),
            surrounding_sides_light,
        );
    }
    /// Setup the skylight queue for a new chunk
    pub fn fill_skylight_queue(&mut self, surrounding_sides_light: &[Option<ChunkSideLight>]) {
        self.skylight_queue =
            get_initial_skylight_queue(&self.block_store.as_block_array(), surrounding_sides_light);
    }

    /// Add the emitted light value to the lighting queue, if it is greater
    /// than the existing light value and can pass into the chunk
    pub fn inform_light_update_from_neighbouring_chunk(
        &mut self,
        light_update: LightUpdate,
        neighbour_index: FaceIndex,
    ) {
        match light_update {
            LightUpdate::EmittedLight(step) => {
                let existing_light_value = self.light_store.get_emitted_light(step.position);
                let would_increase_light =
                    EmittedLight::less(existing_light_value, step.light) != 0;

                let block_id = self.block_store.get_block(step.position);
                let block = &BLOCKS[block_id.as_usize()];
                let can_pass_into_chunk = block
                    .model
                    .is_transparent_in_direction(neighbour_index.opposite());

                if would_increase_light && can_pass_into_chunk {
                    self.emitted_light_queue.push_back(step)
                }
            }
            LightUpdate::EmittedLightShadow(step) => {
                self.emitted_light_shadow_queue.push_back(step);
            }
            LightUpdate::Skylight(step) => {
                let existing_light_value = self.light_store.get_skylight(step.position);
                let would_increase_light = existing_light_value < step.light;

                let block_id = self.block_store.get_block(step.position);
                let block = &BLOCKS[block_id.as_usize()];
                let can_pass_into_chunk = block
                    .model
                    .is_transparent_in_direction(neighbour_index.opposite());

                if would_increase_light && can_pass_into_chunk {
                    self.skylight_queue.push_back(step)
                }
            }
        }
    }

    /// Propagate light and shadow within the chunk, returning the light updates to be applied
    /// outside of the chunk
    pub fn update_lighting(&mut self) -> LightUpdatesOutsideChunk {
        let mut light_updates_outside_chunk = LightUpdatesOutsideChunk::new();

        propagate_emitted_light_shadow(
            &mut self.light_store,
            &mut self.emitted_light_shadow_queue,
            &mut self.emitted_light_queue,
            &mut light_updates_outside_chunk,
            &self.block_store,
        );

        propagate_emitted_light(
            &mut self.light_store,
            &mut self.emitted_light_queue,
            &mut light_updates_outside_chunk,
            &self.block_store,
        );

        propagate_skylight(
            &mut self.light_store,
            &mut self.skylight_queue,
            &mut light_updates_outside_chunk,
            &self.block_store,
        );

        light_updates_outside_chunk
    }
}

/// Returned by `Chunk::raymarch` if a block was hit
pub struct ChunkHit {
    pub local_hit_pos: LocalBlockPosition,
    pub hit_normal: Option<IVec3>,
}
