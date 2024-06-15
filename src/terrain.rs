use std::sync::mpsc::{self, Receiver, Sender};

use generational_arena::{Arena, Index};
use glam::Vec3;
use itertools::Itertools;
use rustc_hash::FxHashSet;

use self::{
    chunk::Chunk,
    event::TerrainEvent,
    load_area::{AreaStatus, LoadArea},
    position_types::ChunkPos,
};
use crate::{
    tasks::{TaskPriority, Tasks},
    CHUNK_LOADING_PRIORITY,
};

pub mod chunk;
pub mod event;
pub mod load_area;
pub mod position_types;

mod temporary_generation;

/// Manages the voxel terrain, responsible for loading/unloading chunks and submitting terrain
/// generation tasks
#[derive(Debug)]
pub struct Terrain {
    /// Generational arena of loaded chunks
    chunks: Arena<Chunk>,
    /// Areas around which chunks are loaded
    load_areas: Arena<LoadArea>,
    /// Terrain events
    events: Vec<TerrainEvent>,
    /// Set of positions of chunks that are currently loading
    loading_chunk_positions: FxHashSet<ChunkPos>,
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
            loading_chunk_positions: FxHashSet::default(),
            loaded_chunk_tx,
            loaded_chunk_rx,
        }
    }

    /// Called each frame to update the terrain
    pub fn update(&mut self, tasks: &mut Tasks) {
        // check for newly loaded chunks
        while let Ok(chunk) = self.loaded_chunk_rx.try_recv() {
            self.finished_loading_chunk(chunk);
        }

        // check chunks to load
        let mut load_queue = Vec::new();
        for (_, area) in &self.load_areas {
            match area.status() {
                AreaStatus::Clean => (),
                AreaStatus::Dirty => {
                    for chunk_pos in area.iter_positions() {
                        let chunk_loaded = self
                            .load_areas
                            .iter()
                            .any(|(_, area)| area.has_chunk_index(&chunk_pos));
                        let chunk_loading = self
                            .loading_chunk_positions
                            .contains(&chunk_pos);

                        if !chunk_loaded && !chunk_loading {
                            load_queue.push((chunk_pos, area.center()));
                        }
                    }
                }
            }
        }
        for (chunk_pos, area_center) in load_queue {
            self.load_chunk(tasks, chunk_pos, area_center);
        }

        for (_, area) in &mut self.load_areas {
            area.set_status(AreaStatus::Clean);
        }

        // check chunks to unload
        return;
        if self
            .load_areas
            .iter()
            .any(|(_, area)| area.status().is_dirty())
        {
            let chunks_to_unload = self
                .chunks
                .iter()
                .filter(|(_, chunk)| {
                    self.load_areas
                        .iter()
                        .any(|(_, area)| area.contains_pos(&chunk.pos()))
                })
                .map(|(chunk_index, _)| chunk_index)
                .collect_vec();

            for chunk_index in chunks_to_unload {
                self.unload_chunk(chunk_index);
            }
        }
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

    /// Spawn a task to begin loading a chunk
    fn load_chunk(&mut self, tasks: &mut Tasks, chunk_pos: ChunkPos, area_center: Vec3) {
        self.loading_chunk_positions
            .insert(chunk_pos);

        // assign a higher priority to chunks closer to the center
        let priority_within_class = Vec3::distance_squared(chunk_pos.as_vec3(), area_center) as i32;

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
        // make sure the chunk is still contained by an area
        if !self
            .load_areas
            .iter()
            .any(|(_, area)| area.contains_pos(&chunk.pos()))
        {
            return;
        }

        let chunk_pos = chunk.pos();
        let chunk_index = self.chunks.insert(chunk);

        for (_, area) in &mut self.load_areas {
            if area.contains_pos(&chunk_pos) {
                area.chunk_loaded(&chunk_pos, chunk_index);
            }
        }

        self.loading_chunk_positions
            .remove(&chunk_pos);

        self.events
            .push(TerrainEvent::ChunkLoaded(chunk_pos));
    }

    /// Unload the chunk with the given position
    fn unload_chunk(&mut self, chunk_index: Index) {
        self.events
            .push(TerrainEvent::ChunkUnloaded(
                self.chunks[chunk_index].pos().clone(),
            ));
        self.chunks.remove(chunk_index);
    }
}
