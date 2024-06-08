use std::{
    collections::{HashMap, HashSet},
    sync::mpsc::{self, Receiver, Sender},
};

use glam::Vec3;

use self::{
    chunk::{Chunk, CHUNK_SIZE_LOG2},
    event::TerrainEvent,
    position_types::ChunkPos,
};

pub mod chunk;
pub mod event;
pub mod position_types;

mod temporary_generation;

/// Manages the voxel terrain, responsible for loading/unloading chunks and scheduling terrain
/// generation
#[derive(Debug)]
pub struct Terrain {
    /// Currently loaded chunks
    loaded_chunks: LoadedChunks,
    /// Outstanding terrain events
    events: Vec<TerrainEvent>,
    /// List of positions of chunks that are currently loading
    loading_chunk_positions: HashSet<ChunkPos>,
    /// Thread pool for loading chunks
    chunk_loading_threads: rayon::ThreadPool,
    /// Sender for loaded chunks
    loaded_chunk_tx: Sender<Chunk>,
    /// Receiver for loaded chunks
    loaded_chunk_rx: Receiver<Chunk>,
}

impl Terrain {
    /// Number of threads to use for chunk loading
    pub const CHUNK_LOADING_THREAD_COUNT: usize = 2;

    pub fn new() -> Self {
        let chunk_loading_threads = rayon::ThreadPoolBuilder::new()
            .num_threads(Self::CHUNK_LOADING_THREAD_COUNT)
            .build()
            .expect("creating thread pool should not fail");

        let (loaded_chunk_tx, loaded_chunk_rx) = mpsc::channel();

        Self {
            loaded_chunks: LoadedChunks::new(),
            events: Vec::new(),
            loading_chunk_positions: HashSet::new(),
            chunk_loading_threads,
            loaded_chunk_tx,
            loaded_chunk_rx,
        }
    }

    /// Called each frame to update the Terrain
    /// * `world_anchors` - points around which chunks are loaded and unloaded
    pub fn update(&mut self, anchors: &[Anchor]) {
        self.check_for_newly_loaded_chunks();
        self.load_chunks_in_range(anchors);
        self.unload_chunks_not_in_range(anchors);
    }

    /// Returns the chunk at the given position, or none if it is not yet loaded
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.loaded_chunks.get_chunk(pos)
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

    /// Schedule a thread to begin loading a chunk
    fn load_chunk(&mut self, chunk_pos: ChunkPos) {
        debug_assert!(!self.loaded_chunks.has_chunk(chunk_pos));
        debug_assert!(!self
            .loading_chunk_positions
            .contains(&chunk_pos));

        self.loading_chunk_positions
            .insert(chunk_pos);

        // make copies for the worker thread
        let chunk_pos = chunk_pos;
        let loaded_chunk_tx = self.loaded_chunk_tx.clone();

        self.chunk_loading_threads
            .spawn(move || {
                let chunk = temporary_generation::generate_chunk(chunk_pos);
                if let Err(e) = loaded_chunk_tx.send(chunk) {
                    log::trace!(
                        "sending chunk from loading thread to main thread returned error: {}",
                        e
                    );
                }
            });
    }

    /// Called once a chunk has finished loading and is ready to be added to the world
    fn finished_loading_chunk(&mut self, chunk: Chunk) {
        self.events
            .push(TerrainEvent::ChunkLoaded(chunk.pos()));
        self.loading_chunk_positions
            .remove(&chunk.pos());
        self.loaded_chunks.add(chunk);
    }

    /// Unload the chunk with the given position
    fn unload_chunk(&mut self, chunk_pos: ChunkPos) {
        self.events
            .push(TerrainEvent::ChunkUnloaded(chunk_pos.clone()));
        self.loaded_chunks.remove(chunk_pos);
    }

    /// Check receiver for any chunks loaded by the worker threads
    fn check_for_newly_loaded_chunks(&mut self) {
        while let Ok(chunk) = self.loaded_chunk_rx.try_recv() {
            self.finished_loading_chunk(chunk);
        }
    }

    /// Start loading any chunks in range of an anchor that aren't already loaded or loading
    fn load_chunks_in_range(&mut self, anchors: &[Anchor]) {
        for anchor in anchors {
            for chunk_pos in anchor.iter_chunk_positions_in_range() {
                let chunk_loaded = self.loaded_chunks.has_chunk(chunk_pos);
                let chunk_loading = self
                    .loading_chunk_positions
                    .contains(&chunk_pos);

                if !chunk_loaded && !chunk_loading {
                    self.load_chunk(chunk_pos);
                }
            }
        }
    }

    /// Unload any chunks not in range of an anchor
    fn unload_chunks_not_in_range(&mut self, anchors: &[Anchor]) {
        // must be cloned to avoid iterating and mutating at the same time
        let positions: Vec<_> = self.loaded_chunks.positions().into();

        // will store whether the chunk with the matching index in `loaded_chunks.positions()`
        // is maintain.ed by any anchor
        let mut maintained: Vec<bool> = (0..positions.len())
            .map(|_| false)
            .collect();

        for (anchor, (chunk_index, chunk_pos)) in itertools::iproduct!(
            anchors,
            self.loaded_chunks
                .positions()
                .iter()
                .enumerate()
        ) {
            let center = anchor.get_center_chunk();
            let diff = (*chunk_pos - center).as_ivec3();

            maintained[chunk_index] |=
                diff.length_squared() <= anchor.load_radius * anchor.load_radius;
        }

        for (chunk_pos, maintained) in positions.iter().zip(maintained.iter()) {
            if !maintained {
                self.unload_chunk(*chunk_pos);
            }
        }
    }
}

/// Points that the world is loaded around
pub struct Anchor {
    pub position: Vec3,
    pub load_radius: i32,
}

impl Anchor {
    /// Returns the position of the chunk that the anchor resides in
    pub fn get_center_chunk(&self) -> ChunkPos {
        ChunkPos::from(self.position.as_ivec3() >> (CHUNK_SIZE_LOG2 as i32))
    }

    /// Returns an iterator over all chunk positions that are in range of the anchor
    #[inline]
    pub fn iter_chunk_positions_in_range<'a>(&'a self) -> impl Iterator<Item = ChunkPos> + 'a {
        let load_range = -self.load_radius..=self.load_radius;
        let center_chunk = self.get_center_chunk();

        itertools::iproduct!(load_range.clone(), load_range.clone(), load_range.clone())
            .filter(|(x, y, z)| x * x + y * y + z * z <= self.load_radius * self.load_radius)
            .map(move |(x, y, z)| ChunkPos::new(x, y, z) + center_chunk)
    }
}

/// data structure for storing loaded chunks
/// combines a HashMap for O(1) access with a Vec of keys for faster iteration
/// (iterating `std::collections::HashMap` currently has to visit empty buckets)
#[derive(Debug)]
struct LoadedChunks {
    chunks: HashMap<ChunkPos, Chunk>,
    loaded_chunk_positions: Vec<ChunkPos>,
}

impl LoadedChunks {
    fn new() -> Self {
        Self {
            chunks: HashMap::new(),
            loaded_chunk_positions: Vec::new(),
        }
    }

    fn add(&mut self, chunk: Chunk) {
        debug_assert!(!self
            .loaded_chunk_positions
            .contains(&chunk.pos()));

        self.loaded_chunk_positions
            .push(chunk.pos());
        self.chunks.insert(chunk.pos(), chunk);
    }

    fn remove(&mut self, chunk_pos: ChunkPos) {
        debug_assert!(self
            .loaded_chunk_positions
            .contains(&chunk_pos));

        self.loaded_chunk_positions.remove(
            self.loaded_chunk_positions
                .iter()
                .position(|pos| *pos == chunk_pos)
                .unwrap(),
        );
        self.chunks.remove(&chunk_pos);
    }

    fn get_chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.chunks.get(&pos)
    }

    fn has_chunk(&self, pos: ChunkPos) -> bool {
        self.chunks.contains_key(&pos)
    }

    fn positions(&self) -> &[ChunkPos] {
        &self.loaded_chunk_positions
    }
}
