use std::sync::Arc;

use itertools::Itertools;

use self::visibility_graph::VisibilityGraph;

use super::position_types::{ChunkPos, LocalBlockPos};
use crate::{
    block::{BlockId, BLOCK_AIR},
    util::measure_time::measure_time,
};

pub mod side;
pub mod visibility_graph;

pub const CHUNK_SIZE: usize = 32;
pub const CHUNK_SIZE_LOG2: usize = 5;
pub const CHUNK_SIZE_SQUARED: usize = (CHUNK_SIZE_U32 * CHUNK_SIZE_U32) as usize;
pub const CHUNK_SIZE_CUBED: usize = (CHUNK_SIZE_U32 * CHUNK_SIZE_U32 * CHUNK_SIZE_U32) as usize;
pub const CHUNK_SIZE_U32: u32 = CHUNK_SIZE as u32;
pub const CHUNK_SIZE_I32: i32 = CHUNK_SIZE as i32;

#[derive(Clone, Debug)]
pub struct Chunk {
    pos: ChunkPos,
    storage: ChunkStorage,
    visibility_graph: VisibilityGraph,
    is_empty: bool,
}

impl Chunk {
    pub fn new(pos: ChunkPos, blocks: Vec<BlockId>) -> Self {
        // this function is called from a parallel thread so it's OK to perform intensive tasks
        // here
        let visibility_graph = VisibilityGraph::compute(&blocks);
        let is_empty = blocks
            .iter()
            .all(|&block_id| block_id == BLOCK_AIR);

        Self {
            pos,
            storage: ChunkStorage::new(blocks),
            visibility_graph,
            is_empty,
        }
    }

    /// Returns the chunk data as an array of block IDs
    pub fn as_block_array(&self) -> Arc<[BlockId]> {
        self.storage.as_block_array()
    }

    /// Returns the block ID at the given position.
    /// Panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPos) -> BlockId {
        self.storage.get_block(pos)
    }

    /// Returns the block ID at the given position.
    /// Panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPos, new_id: BlockId) {
        self.storage.set_block(pos, new_id)
    }

    /// Returns this chunk's position
    pub fn pos(&self) -> ChunkPos {
        self.pos
    }

    /// Returns the computed visibility graph for this chunk
    pub fn visibility_graph(&self) -> VisibilityGraph {
        self.visibility_graph
    }

    /// True if the chunk comprises entirely of air blocks
    pub fn is_empty(&self) -> bool {
        self.is_empty
    }
}

/// Underlying block storage mechanism for the chunk.
/// This is separated so that `Chunk` and its users don't have to worry about the underlying
/// mechanism and can pretend it is just a flat array of blocks
#[derive(Clone, Debug)]
struct ChunkStorage {
    blocks: Vec<BlockId>,
}

impl ChunkStorage {
    fn new(blocks: Vec<BlockId>) -> Self {
        Self { blocks }
    }

    /// Returns the chunk data as an array of block IDs
    fn as_block_array(&self) -> Arc<[BlockId]> {
        self.blocks.clone().into()
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    fn get_block(&self, pos: LocalBlockPos) -> BlockId {
        self.blocks[pos.as_array_index()]
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    fn set_block(&mut self, pos: LocalBlockPos, new_id: BlockId) {
        self.blocks[pos.as_array_index()] = new_id;
    }
}
