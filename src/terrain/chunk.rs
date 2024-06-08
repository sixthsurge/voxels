use std::sync::Arc;

use super::position_types::{ChunkPos, LocalBlockPos};
use crate::block::BlockId;

pub mod side;

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
}

impl Chunk {
    pub fn new(pos: ChunkPos, blocks: Vec<BlockId>) -> Self {
        Self {
            pos,
            storage: ChunkStorage::new(blocks),
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
