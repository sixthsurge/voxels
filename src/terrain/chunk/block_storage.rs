use std::sync::Arc;

use crate::{block::BlockId, terrain::position_types::LocalBlockPos};

/// Underlying storage mechanism for block data in the chunk.
/// This is separated so that `Chunk` and its users don't have to worry about the underlying
/// mechanism
#[derive(Clone, Debug)]
pub struct ChunkBlockStorage {
    data: Vec<BlockId>,
}

impl ChunkBlockStorage {
    pub fn new(data: Vec<BlockId>) -> Self {
        Self { data }
    }

    /// Returns the chunk data as an array of block IDs
    pub fn as_block_array(&self) -> Arc<[BlockId]> {
        self.data.clone().into()
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPos) -> BlockId {
        self.data[pos.get_array_index()]
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPos, new_id: BlockId) {
        self.data[pos.get_array_index()] = new_id;
    }
}
