use std::sync::mpsc::{self, Receiver, Sender};

use generational_arena::{Arena, Index};
use glam::{IVec3, Vec3};
use itertools::Itertools;

use self::{
    block::BlockId,
    chunk::{Chunk, CHUNK_SIZE, CHUNK_SIZE_RECIP},
    event::TerrainEvent,
    load_area::{LoadArea, LoadAreaState},
    position_types::{ChunkPosition, GlobalBlockPosition},
};
use crate::{
    core::tasks::{TaskPriority, Tasks},
    util::vector_map::VectorMapExt,
    CHUNK_LOADING_PRIORITY,
};

pub mod block;
pub mod chunk;
pub mod event;
pub mod lighting;
pub mod load_area;
pub mod position_types;
pub mod temporary_generation;

/// Manages the voxel terrain, responsible for loading/unloading chunks and submitting terrain
/// generation tasks
#[derive(Debug)]
pub struct Terrain {
    /// Currently loaded chunks
    chunks: Arena<Chunk>,
    /// Areas around which chunks are loaded
    load_areas: Arena<LoadArea>,
    /// Terrain events
    events: Vec<TerrainEvent>,
    /// Sender for loaded chunks
    loaded_chunk_tx: Sender<Chunk>,
    /// Receiver for loaded chunks
    loaded_chunk_rx: Receiver<Chunk>,
}

impl Terrain {
    pub fn new() -> Self {
        let (loaded_chunk_tx, loaded_chunk_rx) = mpsc::channel();

        Self {
            chunks: Arena::new(),
            load_areas: Arena::new(),
            events: Vec::new(),
            loaded_chunk_tx,
            loaded_chunk_rx,
        }
    }

    /// Called each frame to update the chunks
    pub fn update(&mut self, tasks: &mut Tasks, camera_pos: Vec3) {
        // check for newly loaded chunks
        while let Ok(chunk) = self.loaded_chunk_rx.try_recv() {
            self.finished_loading_chunk(chunk);
        }

        self.check_chunks_to_unload();
        self.check_chunks_to_load(tasks, camera_pos);

        // mark all areas as clean
        for (_, area) in &mut self.load_areas {
            area.set_state(LoadAreaState::Clean);
        }
    }

    /// If the chunk at the given position is loaded and within the specified load area, returns a
    /// shared reference to that chunk in the chunk arena. Otherwise returns None
    pub fn get_chunk(&self, load_area_index: Index, chunk_pos: &ChunkPosition) -> Option<&Chunk> {
        let load_area = self
            .load_areas
            .get(load_area_index)
            .expect("the load area at index `load_area_index` should exist");

        load_area
            .get_chunk_index(&chunk_pos)
            .and_then(|chunk_index| self.chunks.get(chunk_index))
    }

    /// If the chunk at the given position is loaded and within the specified load area, returns a
    /// mutable reference to that chunk in the chunk arena. Otherwise returns None
    pub fn get_chunk_mut(
        &mut self,
        load_area_index: Index,
        chunk_pos: &ChunkPosition,
    ) -> Option<&mut Chunk> {
        let load_area = self
            .load_areas
            .get(load_area_index)
            .expect("the load area at index `load_area_index` should exist");

        load_area
            .get_chunk_index(&chunk_pos)
            .and_then(|chunk_index| self.chunks.get_mut(chunk_index))
    }

    /// If the position is inside a loaded chunk within the given load area, returns the block ID
    /// at that position. Otherwise returns None
    pub fn get_block(
        &self,
        load_area_index: Index,
        global_block_pos: &GlobalBlockPosition,
    ) -> Option<BlockId> {
        let (local_block_pos, chunk_pos) = global_block_pos.get_local_and_chunk_pos();

        self.get_chunk(load_area_index, &chunk_pos)
            .map(|chunk| chunk.get_block(local_block_pos))
    }

    /// If the global block position is inside a loaded chunk within this area, sets the block
    /// ID at the given index to the provided ID and fire a `BlockModified` event
    /// Otherwise returns false
    pub fn set_block(
        &mut self,
        load_area_index: Index,
        global_block_pos: &GlobalBlockPosition,
        new_id: BlockId,
    ) -> bool {
        let (local_block_pos, chunk_pos) = global_block_pos.get_local_and_chunk_pos();

        if let Some(chunk) = self.get_chunk_mut(load_area_index, &chunk_pos) {
            chunk.set_block(local_block_pos, new_id);
            self.events
                .push(TerrainEvent::BlockModified(chunk_pos, local_block_pos));
            true
        } else {
            false
        }
    }

    /// Raymarch through the chunks in the given load area, returning the position and normal of
    /// the first block intersected by the ray
    pub fn raymarch(
        &self,
        load_area_index: Index,
        ray_origin: Vec3,
        ray_direction: Vec3,
        maximum_distance: f32,
    ) -> Option<TerrainHit> {
        pub const EPS: f32 = 1e-3;

        let dir_step = ray_direction.map(|component| if component >= 0.0 { 1.0 } else { 0.0 });
        let dir_recip = ray_direction.recip();

        let mut t = 0.0;
        let mut previous_chunk_pos = None;

        while t < maximum_distance {
            let ray_pos = ray_origin + ray_direction * t;

            let chunk_pos = ChunkPosition::from((ray_pos * CHUNK_SIZE_RECIP).floor().as_ivec3());
            if let Some(chunk) = self.get_chunk(load_area_index, &chunk_pos) {
                let ray_origin = ray_pos - chunk_pos.as_vec3() * (CHUNK_SIZE as f32);

                if let Some(hit) = chunk.raymarch(
                    ray_origin,
                    ray_direction,
                    previous_chunk_pos,
                    maximum_distance - t,
                ) {
                    return Some(TerrainHit {
                        hit_pos: GlobalBlockPosition::from_local_and_chunk_pos(
                            hit.local_hit_pos,
                            chunk_pos,
                        ),
                        hit_normal: hit.hit_normal,
                    });
                }
            }

            // advance to the next chunk position
            let deltas = (dir_step - ray_pos * CHUNK_SIZE_RECIP).fract_gl()
                * dir_recip
                * (CHUNK_SIZE as f32);
            t += deltas.min_element().max(EPS);

            previous_chunk_pos = Some(chunk_pos);
        }

        None
    }

    /// The arena of loaded chunks
    pub fn chunks(&self) -> &Arena<Chunk> {
        &self.chunks
    }

    /// Mutable reference to the arena of loaded chunks
    pub fn chunks_mut(&mut self) -> &mut Arena<Chunk> {
        &mut self.chunks
    }

    /// The arena of areas around which chunks are loaded
    pub fn load_areas(&self) -> &Arena<LoadArea> {
        &self.load_areas
    }

    /// Mutable reference to the arena of areas around which chunks are loaded
    pub fn load_areas_mut(&mut self) -> &mut Arena<LoadArea> {
        &mut self.load_areas
    }

    /// Returns an iterator over all events that have occurred since the last call to
    /// `clear_events()` in chronological order
    pub fn events(&self) -> impl Iterator<Item = &TerrainEvent> {
        self.events.iter()
    }

    /// Clear the list of outstanding events
    pub fn clear_events(&mut self) {
        self.events.clear();
    }

    /// Called each frame to check for new chunks to load
    fn check_chunks_to_load(&mut self, tasks: &mut Tasks, camera_pos: Vec3) {
        let load_queue = self
            .load_areas
            .iter()
            .filter(|(_, area)| area.state().is_dirty())
            .map(|(_, area)| {
                area.iter_positions().filter(|chunk_pos| {
                    self.load_areas
                        .iter()
                        .all(|(_, area)| area.is_unloaded(&chunk_pos))
                })
            })
            .flatten()
            .collect_vec();

        for chunk_pos in load_queue {
            self.load_chunk(tasks, chunk_pos, camera_pos);
        }
    }

    /// Called each frame to check if any chunks should be unloaded
    fn check_chunks_to_unload(&mut self) {
        if self
            .load_areas
            .iter()
            .any(|(_, area)| area.state().is_dirty())
        {
            let unload_queue = self
                .chunks
                .iter()
                .filter(|(_, chunk)| {
                    self.load_areas
                        .iter()
                        .all(|(_, area)| !area.is_within_area(&chunk.position()))
                })
                .map(|(chunk_index, _)| chunk_index)
                .collect_vec();

            for chunk_index in unload_queue {
                self.unload_chunk(chunk_index);
            }
        }
    }

    /// Spawn a task to begin loading a chunk
    fn load_chunk(&mut self, tasks: &mut Tasks, chunk_pos: ChunkPosition, camera_pos: Vec3) {
        // don't load a chunk if it is already loaded or loading
        if self
            .load_areas
            .iter()
            .any(|(_, load_area)| !load_area.is_unloaded(&chunk_pos))
        {
            log::info!("fiesta");
            return;
        }

        // inform the load areas that the chunk is loading
        self.load_areas
            .iter_mut()
            .filter(|(_, load_area)| load_area.is_within_bounds(&chunk_pos))
            .for_each(|(_, load_area)| load_area.mark_loading(&chunk_pos));

        // assign a higher priority to chunks closer to the camera
        let priority_within_class =
            Vec3::distance_squared(chunk_pos.as_vec3(), camera_pos / (CHUNK_SIZE as f32)) as i32;

        // clone sender for the worker thread
        let loaded_chunk_tx = self.loaded_chunk_tx.clone();

        tasks.submit(
            TaskPriority {
                class_priority: CHUNK_LOADING_PRIORITY,
                priority_within_class,
            },
            move || {
                let chunk = temporary_generation::generate_chunk(chunk_pos);
                if let Err(e) = loaded_chunk_tx.send(chunk) {
                    log::trace!(
                        "sending chunk from loading thread to main thread returned error: {}",
                        e
                    );
                }
            },
        );
    }

    /// Called once a chunk has finished loading and is ready to be added to the world
    fn finished_loading_chunk(&mut self, chunk: Chunk) {
        // make sure the chunk is still within a load area
        // this could be false if the area has moved since the chunk was queued for loading
        if !self
            .load_areas
            .iter()
            .any(|(_, area)| area.is_within_area(&chunk.position()))
        {
            return;
        }

        let chunk_pos = chunk.position();
        let chunk_index = self.chunks.insert(chunk);

        // inform the load areas that the chunk is loaded
        self.load_areas
            .iter_mut()
            .filter(|(_, load_area)| load_area.is_within_bounds(&chunk_pos))
            .for_each(|(_, load_area)| load_area.mark_loaded(&chunk_pos, chunk_index));

        self.events.push(TerrainEvent::ChunkLoaded(chunk_pos));
    }

    /// Unload the chunk with the given position
    fn unload_chunk(&mut self, chunk_index: Index) {
        let chunk = &self.chunks[chunk_index];
        let chunk_pos = chunk.position();

        // inform the load areas that the chunk is unloaded
        self.load_areas
            .iter_mut()
            .filter(|(_, load_area)| load_area.is_within_bounds(&chunk_pos))
            .for_each(|(_, load_area)| load_area.mark_unloaded(&chunk_pos));

        self.events.push(TerrainEvent::ChunkUnloaded(
            self.chunks[chunk_index].position().clone(),
        ));
        self.chunks.remove(chunk_index);
    }
}

/// Returned by `Terrain::raymarch` when a block is intersected
pub struct TerrainHit {
    pub hit_pos: GlobalBlockPosition,
    pub hit_normal: Option<IVec3>,
}
