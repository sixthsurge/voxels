use derive_more::{Add, From, Sub};
use glam::{IVec3, UVec3, Vec3};

use super::chunk::{CHUNK_SIZE, CHUNK_SIZE_I32, CHUNK_SIZE_LOG2, CHUNK_SIZE_U32};

/// position of a block in the world
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Add, From, Sub)]
pub struct GlobalBlockPos(IVec3);

impl GlobalBlockPos {
    pub const ZERO: Self = Self(IVec3::ZERO);

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    pub fn from_local_and_chunk_pos(local_pos: LocalBlockPos, chunk_pos: ChunkPos) -> Self {
        (local_pos.0.as_ivec3() + chunk_pos.0 * CHUNK_SIZE_I32).into()
    }
}

/// position of a block in a chunk
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Add, From, Sub)]
pub struct LocalBlockPos(UVec3);

impl LocalBlockPos {
    pub const ZERO: Self = Self(UVec3::ZERO);

    pub fn new(x: u32, y: u32, z: u32) -> Self {
        Self(UVec3::new(x, y, z))
    }

    pub fn from_array_index(block_index: usize) -> Self {
        Self(UVec3::new(
            ((block_index >> (0 * CHUNK_SIZE_LOG2)) & (CHUNK_SIZE - 1)) as u32,
            ((block_index >> (1 * CHUNK_SIZE_LOG2)) & (CHUNK_SIZE - 1)) as u32,
            ((block_index >> (2 * CHUNK_SIZE_LOG2)) & (CHUNK_SIZE - 1)) as u32,
        ))
    }

    pub fn from_global_pos(global_pos: GlobalBlockPos) -> Self {
        (global_pos.0 & (CHUNK_SIZE_I32 - 1))
            .as_uvec3()
            .into()
    }

    pub fn as_array_index(&self) -> usize {
        ((CHUNK_SIZE_U32 * CHUNK_SIZE_U32) * self.0.z + CHUNK_SIZE_U32 * self.0.y + self.0.x)
            as usize
    }
}

/// position of a chunk in the world
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Add, From, Sub)]
pub struct ChunkPos(IVec3);

impl ChunkPos {
    pub const ZERO: Self = Self(IVec3::ZERO);

    pub fn new(x: i32, y: i32, z: i32) -> Self {
        Self(IVec3::new(x, y, z))
    }

    pub fn as_ivec3(&self) -> IVec3 {
        self.0
    }

    pub fn as_vec3(&self) -> Vec3 {
        self.0.as_vec3()
    }
}
