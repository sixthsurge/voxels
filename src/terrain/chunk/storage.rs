use glam::{UVec2, UVec3};

use crate::{
    block::BlockId, terrain::position_types::LocalBlockPos,
    util::dictionary_encoding::DictionaryEncodedArray,
};

use super::{
    CHUNK_SIZE, CHUNK_SIZE_2D, CHUNK_SIZE_3D, CHUNK_SIZE_CUBED, CHUNK_SIZE_LOG2, CHUNK_SIZE_SQUARED,
};

/// Storage for a chunk's block data in memory
#[derive(Clone, Debug)]
pub struct ChunkBlockStorage {
    layers: [DictionaryEncodedArray<BlockId>; CHUNK_SIZE],
}

impl ChunkBlockStorage {
    /// Initialize the block storage from an array of block IDs, ordered by y, then z, then x
    pub fn new(blocks: Vec<BlockId>) -> Self {
        let mut layers =
            array_init::array_init(|_| DictionaryEncodedArray::new(CHUNK_SIZE_SQUARED));

        for (y, x, z) in itertools::iproduct!(
            0..(CHUNK_SIZE as u32),
            0..(CHUNK_SIZE as u32),
            0..(CHUNK_SIZE as u32)
        ) {
            let index_in_layer = CHUNK_SIZE_2D.flatten(UVec2::new(x, z));
            let index_in_blocks = CHUNK_SIZE_3D.flatten(UVec3::new(x, y, z));

            unsafe { layers[y as usize].set_unchecked(index_in_layer, blocks[index_in_blocks]) };
        }

        Self { layers }
    }

    /// Returns the chunk data as an array of block IDs, ordered by y, then z, then x
    pub fn as_block_array(&self) -> Box<[BlockId]> {
        (0..CHUNK_SIZE_CUBED)
            .map(|index| {
                let y_index = index >> (2 * CHUNK_SIZE_LOG2);
                let xz_index = index & (CHUNK_SIZE - 1);
                unsafe { self.layers[y_index].get_unchecked(xz_index) }
            })
            .collect()
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPos) -> BlockId {
        let pos = pos.as_uvec3();
        let index_in_layer = CHUNK_SIZE_2D.flatten(UVec2::new(pos.x, pos.z));

        self.layers[pos.y as usize]
            .get(index_in_layer)
            .expect("index should be in layer")
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPos, new_id: BlockId) {
        let pos = pos.as_uvec3();
        let index_in_layer = CHUNK_SIZE_2D.flatten(UVec2::new(pos.x, pos.z));

        self.layers[pos.y as usize]
            .set(index_in_layer, new_id)
            .expect("index should be in layer")
    }
}

/// Storage for a chunk's emitted light data in memory
#[derive(Clone, Debug)]
pub struct ChunkLightStorage {
    pub emitted: EmittedLightStorage,
    pub sky: SkylightStorage,
}

impl ChunkLightStorage {
    pub fn new() -> Self {
        Self {
            emitted: EmittedLightStorage {},
            sky: SkylightStorage {},
        }
    }
}

/// Storage for a chunk's emitted light data in memory
#[derive(Clone, Debug)]
pub struct EmittedLightStorage {}

/// Storage mechanism for a chunk's skylight data in memory
#[derive(Clone, Debug)]
pub struct SkylightStorage {}
