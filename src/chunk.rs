use glam::UVec3;

use crate::block::BlockId;

/// Size of a chunk along on the X axis, in blocks
pub const CHUNK_SIZE_X: u32 = 32;
/// Size of a chunk along on the Y axis, in blocks
pub const CHUNK_SIZE_Y: u32 = 32;
/// Size of a chunk along on the Z axis, in blocks
pub const CHUNK_SIZE_Z: u32 = 32;
/// Total number of blocks in a chunk
pub const CHUNK_SIZE_FLAT: usize = (CHUNK_SIZE_X * CHUNK_SIZE_Y * CHUNK_SIZE_Z) as usize;

pub type Chunk = GenericChunk<SimpleChunkStorage>;

pub struct GenericChunk<Storage>
where
    Storage: ChunkStorage,
{
    storage: Storage,
}

impl<Storage> GenericChunk<Storage>
where
    Storage: ChunkStorage,
{
    pub fn new() -> Self {
        Self {
            storage: Storage::from_block_array([BlockId(0); CHUNK_SIZE_FLAT]),
        }
    }
}

pub trait ChunkStorage: Clone {
    /// Initialize the chunk storage with an array of block IDs
    fn from_block_array(blocks: [BlockId; CHUNK_SIZE_FLAT]) -> Self;

    /// Returns the chunk data as an array of block IDs
    fn as_block_array(&self) -> [BlockId; CHUNK_SIZE_FLAT];

    /// Returns an iterator over the block IDs in the chunk
    /// Blocks are ordered by x, then y, then z
    fn iter(&self) -> impl Iterator<Item = BlockId>;

    /// Returns the block ID at the given position
    /// Panics if the position is out of bounds
    fn get(&self, pos: UVec3) -> BlockId;

    /// Returns the block ID at the given position
    /// Panics if the position is out of bounds
    fn set(&mut self, pos: UVec3, id: BlockId);
}

#[allow(unused)]
#[derive(Clone)]
pub struct SimpleChunkStorage {
    blocks: [BlockId; CHUNK_SIZE_FLAT],
}

impl ChunkStorage for SimpleChunkStorage {
    fn from_block_array(blocks: [BlockId; CHUNK_SIZE_FLAT]) -> Self {
        Self { blocks }
    }

    fn as_block_array(&self) -> [BlockId; CHUNK_SIZE_FLAT] {
        self.blocks.clone()
    }

    fn iter(&self) -> impl Iterator<Item = BlockId> {
        self.blocks.iter().copied()
    }

    fn get(&self, pos: UVec3) -> BlockId {
        self.blocks[uvec3_to_chunk_index(pos)]
    }

    fn set(&mut self, pos: UVec3, new_id: BlockId) {
        self.blocks[uvec3_to_chunk_index(pos)] = new_id;
    }
}

pub fn uvec3_to_chunk_index(pos: UVec3) -> usize {
    ((CHUNK_SIZE_X * CHUNK_SIZE_Y) * pos.z + CHUNK_SIZE_X * pos.y + pos.x) as usize
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
