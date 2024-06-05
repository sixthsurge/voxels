use glam::UVec3;

use crate::block::BlockId;

pub const CHUNK_SIZE: u32 = 32;
pub const CHUNK_SIZE_SQUARED: usize = (CHUNK_SIZE * CHUNK_SIZE) as usize;
pub const CHUNK_SIZE_CUBED: usize = (CHUNK_SIZE * CHUNK_SIZE * CHUNK_SIZE) as usize;

#[derive(Clone, Debug)]
pub struct Chunk {
    storage: ChunkStorage,
}

impl Chunk {
    pub fn new() -> Self {
        Self {
            storage: ChunkStorage::from_block_array([BlockId(0); CHUNK_SIZE_CUBED]),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChunkStorage {
    blocks: [BlockId; CHUNK_SIZE_CUBED],
}

impl ChunkStorage {
    /// Initialize the chunk storage with an array of block IDs
    pub fn from_block_array(blocks: [BlockId; CHUNK_SIZE_CUBED]) -> Self {
        Self { blocks }
    }

    /// Returns the chunk data as an array of block IDs
    pub fn as_block_array(&self) -> [BlockId; CHUNK_SIZE_CUBED] {
        self.blocks.clone()
    }

    /// Returns the block ID at the given position
    /// Panics if the position is out of bounds
    pub fn get(&self, pos: UVec3) -> BlockId {
        self.blocks[uvec3_to_chunk_index(pos)]
    }

    /// Returns the block ID at the given position
    /// Panics if the position is out of bounds
    pub fn set(&mut self, pos: UVec3, new_id: BlockId) {
        self.blocks[uvec3_to_chunk_index(pos)] = new_id;
    }
}

pub fn uvec3_to_chunk_index(pos: UVec3) -> usize {
    ((CHUNK_SIZE * CHUNK_SIZE) * pos.z + CHUNK_SIZE * pos.y + pos.x) as usize
}

/*
pub fn ivec3_is_within_chunk_bounds(pos: IVec3) -> bool {
    pos.x >= 0
        && pos.x < CHUNK_SIZE_X as i32
        && pos.y >= 0
        && pos.y < CHUNK_SIZE_Y as i32
        && pos.z >= 0
        && pos.z < CHUNK_SIZE_Z as i32
}

pub fn uvec3_is_within_chunk_bounds(pos: UVec3) -> bool {
    pos.x < CHUNK_SIZE_X && pos.y < CHUNK_SIZE_Y && pos.z < CHUNK_SIZE_Z
}
*/
