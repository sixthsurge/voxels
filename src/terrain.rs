use std::sync::mpsc::{self, Receiver, Sender};

use glam::{IVec3, Vec3};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    tasks::{TaskPriority, Tasks},
    CHUNK_LOADING_PRIORITY,
};

use self::{
    chunk::{Chunk, CHUNK_SIZE_LOG2},
    event::TerrainEvent,
    position_types::ChunkPos,
};

pub mod chunk;
pub mod event;
pub mod position_types;

mod temporary_generation;

/// Manages the voxel terrain, responsible for loading/unloading chunks and submitting terrain
/// generation tasks
#[derive(Debug)]
pub struct Terrain {
    /// Currently loaded chunks
    loaded_chunks: LoadedChunks,
    /// Outstanding terrain events
    events: Vec<TerrainEvent>,
    /// Anchors that the terrain is loaded around
    anchors: Vec<Anchor>,
    /// Positions of chunks that are currently loading
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
            loaded_chunks: LoadedChunks::new(),
            events: Vec::new(),
            anchors: Vec::new(),
            loading_chunk_positions: FxHashSet::default(),
            loaded_chunk_tx,
            loaded_chunk_rx,
        }
    }

    /// Called each frame to update the Terrain
    /// * `world_anchors` - points around which chunks are loaded and unloaded
    pub fn update(&mut self, tasks: &mut Tasks) {
        self.check_for_newly_loaded_chunks();
        self.check_chunks_to_load(tasks);
        self.check_chunks_to_unload();

        // update anchors
        for anchor in &mut self.anchors {
            anchor.update();
        }
    }

    /// Returns the chunk at the given position, or none if it is not yet loaded
    pub fn get_chunk(&self, pos: ChunkPos) -> Option<&Chunk> {
        self.loaded_chunks.get_chunk(pos)
    }

    /// True if the chunk with the given position is currently loaded
    pub fn has_chunk(&self, pos: ChunkPos) -> bool {
        self.loaded_chunks.has_chunk(pos)
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

    /// The anchors around which chunks are loaded
    pub fn anchors(&self) -> &[Anchor] {
        &self.anchors
    }

    /// Mutable access to the vec of anchors around which chunks are loaded
    pub fn anchors_mut(&mut self) -> &mut Vec<Anchor> {
        &mut self.anchors
    }

    /// Spawn a task to begin loading a chunk
    fn load_chunk(&mut self, tasks: &mut Tasks, chunk_pos: ChunkPos, anchor_chunk: ChunkPos) {
        debug_assert!(!self.loaded_chunks.has_chunk(chunk_pos));
        debug_assert!(!self
            .loading_chunk_positions
            .contains(&chunk_pos));

        self.loading_chunk_positions
            .insert(chunk_pos);

        // assign a higher priority to chunks closer to the anchor
        let priority_within_class = (chunk_pos - anchor_chunk)
            .as_ivec3()
            .length_squared();

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
        // make sure this chunk is still loaded by an anchor
        // this is necessary because the anchor may have moved away from the chunk by the time it
        // finished loading
        if self
            .anchors
            .iter()
            .any(|anchor| anchor.is_in_range(chunk.pos()))
        {
            self.events
                .push(TerrainEvent::ChunkLoaded(chunk.pos()));
            self.loading_chunk_positions
                .remove(&chunk.pos());
            self.loaded_chunks.add(chunk);
        }
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
    fn check_chunks_to_load(&mut self, tasks: &mut Tasks) {
        // select positions of chunks that need to be loaded
        let mut chunks_to_load = Vec::new();
        for anchor in &self.anchors {
            // originally I iterated over all the chunks in the range, but this ate a huge chunk of
            // the frame time, especially in debug builds
            // instead I only iterate over all the chunks when the anchor is new (has never loaded
            // any chunks before), and create an iterator that only covers the new chunks when
            // an anchor moves
            if anchor.is_new() {
                for chunk_pos in anchor.iter_all_chunks_in_range() {
                    chunks_to_load.push((chunk_pos, anchor.get_center_chunk()));
                }
            } else if anchor.has_moved_between_chunks() {
                for chunk_pos in anchor.iter_new_chunks_in_range() {
                    chunks_to_load.push((chunk_pos, anchor.get_center_chunk()));
                }
            }
        }

        for (chunk_pos, anchor_chunk) in chunks_to_load {
            let chunk_loaded = self.loaded_chunks.has_chunk(chunk_pos);
            let chunk_loading = self
                .loading_chunk_positions
                .contains(&chunk_pos);

            if !chunk_loaded && !chunk_loading {
                self.load_chunk(tasks, chunk_pos, anchor_chunk);
            }
        }
    }

    /// Unload any chunks not in range of an anchor
    fn check_chunks_to_unload(&mut self) {
        let mut chunks_to_unload = Vec::new();

        // for each anchor, if it has moved get the list of chunks that are no longer loaded by
        // that anchor, and check if each chunk in that list is loaded by a different anchor
        // - if it isn't, it should be unloaded
        for anchor in &self.anchors {
            // skip anchors that are new or have not moved between chunks
            if anchor.is_new() || !anchor.has_moved_between_chunks() {
                continue;
            }

            // get the list of chunks that are no longer loaded by this anchor
            let chunks_to_check = anchor
                .iter_chunks_no_longer_in_range()
                .collect_vec();

            // will store whether the chunk with the matching index in `chunks_to_check`
            // is maintained by any anchor
            let mut maintained = vec![false; chunks_to_check.len()];

            // for each chunk to check and anchor, check if the chunk should be loaded by that
            // anchor
            for (anchor, (chunk_index, chunk_pos)) in
                itertools::iproduct!(&self.anchors, chunks_to_check.iter().enumerate())
            {
                maintained[chunk_index] |= anchor.is_in_range(*chunk_pos);
            }

            // mark any chunks that aren't maintained by another anchor for unloading
            chunks_to_unload.extend(
                chunks_to_check
                    .iter()
                    .zip(maintained.iter().copied())
                    .filter(|(_, maintained)| !maintained)
                    .map(|(chunk_pos, _)| chunk_pos),
            )
        }

        for chunk_pos in chunks_to_unload {
            if self.has_chunk(chunk_pos) {
                self.unload_chunk(chunk_pos);
            }
        }
    }
}

/// Points around which the world is loaded
#[derive(Clone, Debug)]
pub struct Anchor {
    /// Position of the anchor in the world
    pos: Vec3,
    /// Whether `update` has ever been called on this `Anchor`
    is_new: bool,
    /// Position of the chunk containing this anchor when `update` was last called
    last_center_chunk: Option<ChunkPos>,
    /// Number of chunks to load on each axis in both directions
    load_distance: IVec3,
}

impl Anchor {
    pub fn new(pos: Vec3, load_distance: IVec3) -> Self {
        Self {
            pos,
            is_new: true,
            last_center_chunk: None,
            load_distance,
        }
    }

    /// Update the anchor's position
    pub fn set_pos(&mut self, new_pos: Vec3) {
        self.pos = new_pos;
    }

    /// Called after loading chunks to inform the anchor that it has no chunks to load
    fn update(&mut self) {
        self.is_new = false;
        self.last_center_chunk = Some(self.get_center_chunk());
    }

    /// True if `update` has never been called on this `Anchor`
    fn is_new(&self) -> bool {
        self.is_new
    }

    /// True if this anchor has moved between chunks since the last time `update` was called
    fn has_moved_between_chunks(&self) -> bool {
        if let Some(last_chunk_pos) = self.last_center_chunk {
            self.get_center_chunk() != last_chunk_pos
        } else {
            false
        }
    }

    /// Returns the position of the chunk that the anchor resides in
    fn get_center_chunk(&self) -> ChunkPos {
        ChunkPos::from(self.pos.as_ivec3() >> (CHUNK_SIZE_LOG2 as i32))
    }

    /// True if this chunk would be loaded by the anchor
    fn is_in_range(&self, chunk_pos: ChunkPos) -> bool {
        let center_chunk = self.get_center_chunk();
        let min_pos = center_chunk.as_ivec3() - self.load_distance;
        let max_pos = center_chunk.as_ivec3() + self.load_distance;
        chunk_pos
            .as_ivec3()
            .cmpge(min_pos)
            .all()
            && chunk_pos
                .as_ivec3()
                .cmple(max_pos)
                .all()
    }

    /// Returns an iterator over the chunk positions that are in range of the anchor
    fn iter_all_chunks_in_range<'a>(&'a self) -> impl Iterator<Item = ChunkPos> + 'a {
        let center_chunk = self.get_center_chunk();
        let min_pos = center_chunk.as_ivec3() - self.load_distance;
        let max_pos = center_chunk.as_ivec3() + self.load_distance;

        itertools::iproduct!(
            min_pos.x..=max_pos.x,
            min_pos.y..=max_pos.y,
            min_pos.z..=max_pos.z
        )
        .map(|(x, y, z)| ChunkPos::new(x, y, z))
    }

    /// Returns an iterator over the chunk positions that are in range of the anchor now,
    /// but weren't in range of the anchor when `update` was last called
    /// This takes advantage of the fact that the fact that chunks are loaded in a square rather
    /// than a circular shape
    /// Note: this may return the same position twice
    fn iter_new_chunks_in_range<'a>(&'a self) -> impl Iterator<Item = ChunkPos> + 'a {
        let center_chunk = self.get_center_chunk();
        let last_center_chunk = self.last_center_chunk.expect(
            "`last_center_chunk` should not be `None` when `iter_new_chunks_in_range` is called",
        );

        // calculate 3 AABBs for the chunks that have just entered the loading range
        let new_center = center_chunk.as_ivec3();
        let old_center = last_center_chunk.as_ivec3();
        let diff_signum = (new_center - old_center).signum();
        let min_corner = new_center - self.load_distance;
        let max_corner = new_center + self.load_distance;
        let new_frontier = new_center + self.load_distance * diff_signum;
        let old_frontier = old_center + self.load_distance * diff_signum;
        let min_frontier = IVec3::min(new_frontier, old_frontier);
        let max_frontier = IVec3::max(new_frontier, old_frontier);

        // chunks loaded in the X direction
        let iter_0 = itertools::iproduct!(
            min_frontier.x..=max_frontier.x - (diff_signum.x == 0) as i32,
            min_corner.y..=max_corner.y,
            min_corner.z..=max_corner.z,
        );

        // chunks loaded in the Y direction
        let iter_1 = itertools::iproduct!(
            min_corner.x..=max_corner.x,
            min_frontier.y..=max_frontier.y - (diff_signum.y == 0) as i32,
            min_corner.z..=max_corner.z,
        );

        // chunks loaded in the Z direction
        let iter_2 = itertools::iproduct!(
            min_corner.x..=max_corner.x,
            min_corner.y..=max_corner.y,
            min_frontier.z..=max_frontier.z - (diff_signum.z == 0) as i32,
        );

        iter_0
            .chain(iter_1)
            .chain(iter_2)
            .map(|(x, y, z)| ChunkPos::new(x, y, z))
    }

    /// Returns an iterator over all the chunk positions that were in range of the anchor last time
    /// `update` was called, but aren't in range now
    fn iter_chunks_no_longer_in_range<'a>(&'a self) -> impl Iterator<Item = ChunkPos> + 'a {
        let center_chunk = self.get_center_chunk();
        let last_center_chunk = self.last_center_chunk.expect(
            "`last_center_chunk` should not be `None` when `iter_chunks_no_longer_in_range` is called",
        );

        // calculate 3 AABBs for the chunks that have just exited the loading range
        let new_center = center_chunk.as_ivec3();
        let old_center = last_center_chunk.as_ivec3();
        let diff_signum = (new_center - old_center).signum();
        let min_corner = old_center - self.load_distance;
        let max_corner = old_center + self.load_distance;
        let new_frontier = new_center - (self.load_distance + 1) * diff_signum;
        let old_frontier = old_center - (self.load_distance + 1) * diff_signum;
        let min_frontier = IVec3::min(new_frontier, old_frontier);
        let max_frontier = IVec3::max(new_frontier, old_frontier);

        // chunks left behind in the X direction
        let iter_0 = itertools::iproduct!(
            min_frontier.x..=max_frontier.x - (diff_signum.x == 0) as i32,
            min_corner.y..=max_corner.y,
            min_corner.z..=max_corner.z,
        );

        // chunks left behind in the Y direction
        let iter_1 = itertools::iproduct!(
            min_corner.x..=max_corner.x,
            min_frontier.y..=max_frontier.y - (diff_signum.y == 0) as i32,
            min_corner.z..=max_corner.z,
        );

        // chunks left behind in the Z direction
        let iter_2 = itertools::iproduct!(
            min_corner.x..=max_corner.x,
            min_corner.y..=max_corner.y,
            min_frontier.z..=max_frontier.z - (diff_signum.z == 0) as i32,
        );

        iter_0
            .chain(iter_1)
            .chain(iter_2)
            .map(|(x, y, z)| ChunkPos::new(x, y, z))
    }
}

/// Data structure for storing loaded chunks
/// combines a HashMap for O(1) access with a Vec of keys for faster iteration
#[derive(Debug)]
struct LoadedChunks {
    chunks: FxHashMap<ChunkPos, Chunk>,
    loaded_chunk_positions: Vec<ChunkPos>,
}

impl LoadedChunks {
    fn new() -> Self {
        Self {
            chunks: FxHashMap::default(),
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
}
