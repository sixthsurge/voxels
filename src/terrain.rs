use std::sync::mpsc::{self, Receiver, Sender};

use generational_arena::{Arena, Index};
use glam::Vec3;
use itertools::Itertools;
use rustc_hash::FxHashSet;

use self::{
    chunk::{Chunk, CHUNK_SIZE},
    event::TerrainEvent,
    load_area::{LoadArea, LoadAreaState},
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

    /// Called each frame to update the terrain
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
                area.iter_positions()
                    .filter(|chunk_pos| {
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
                        .all(|(_, area)| !area.is_within_area(&chunk.pos()))
                })
                .map(|(chunk_index, _)| chunk_index)
                .collect_vec();

            for chunk_index in unload_queue {
                self.unload_chunk(chunk_index);
            }
        }
    }

    /// Spawn a task to begin loading a chunk
    fn load_chunk(&mut self, tasks: &mut Tasks, chunk_pos: ChunkPos, camera_pos: Vec3) {
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
            .any(|(_, area)| area.is_within_area(&chunk.pos()))
        {
            return;
        }

        let chunk_pos = chunk.pos();
        let chunk_index = self.chunks.insert(chunk);

        // inform the load areas that the chunk is loaded
        self.load_areas
            .iter_mut()
            .filter(|(_, load_area)| load_area.is_within_bounds(&chunk_pos))
            .for_each(|(_, load_area)| load_area.mark_loaded(&chunk_pos, chunk_index));

        self.events
            .push(TerrainEvent::ChunkLoaded(chunk_pos));
    }

    /// Unload the chunk with the given position
    fn unload_chunk(&mut self, chunk_index: Index) {
        let chunk = &self.chunks[chunk_index];
        let chunk_pos = chunk.pos();

        // inform the load areas that the chunk is unloaded
        self.load_areas
            .iter_mut()
            .filter(|(_, load_area)| load_area.is_within_bounds(&chunk_pos))
            .for_each(|(_, load_area)| load_area.mark_unloaded(&chunk_pos));

        self.events
            .push(TerrainEvent::ChunkUnloaded(
                self.chunks[chunk_index].pos().clone(),
            ));
        self.chunks.remove(chunk_index);
    }
}
