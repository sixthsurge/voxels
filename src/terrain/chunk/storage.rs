use glam::{UVec2, UVec3};
use itertools::Itertools;

use crate::{
    block::BlockId, terrain::position_types::LocalBlockPos,
    util::dictionary_encoding::DictionaryEncodedArray,
};

use super::{
    CHUNK_SIZE, CHUNK_SIZE_2D, CHUNK_SIZE_3D, CHUNK_SIZE_CUBED, CHUNK_SIZE_LOG2, CHUNK_SIZE_SQUARED,
};

/// Storage for a chunk's block data in memory
#[derive(Clone, Debug)]
pub enum ChunkBlockStorage {
    Uniform(BlockId),
    Layered([DictionaryEncodedArray<BlockId>; CHUNK_SIZE]),
}

impl ChunkBlockStorage {
    /// Initialize the block storage from an array of block IDs, ordered by y, then z, then x
    pub fn new(blocks: Vec<BlockId>) -> Self {
        if let Ok(&block_id) = blocks.iter().all_equal_value() {
            // uniform
            Self::Uniform(block_id)
        } else {
            // layered
            let mut layers =
                array_init::array_init(|_| DictionaryEncodedArray::new(CHUNK_SIZE_SQUARED));

            for (y, x, z) in itertools::iproduct!(
                0..(CHUNK_SIZE as u32),
                0..(CHUNK_SIZE as u32),
                0..(CHUNK_SIZE as u32)
            ) {
                let index_in_layer = CHUNK_SIZE_2D.flatten(UVec2::new(x, z));
                let index_in_blocks = CHUNK_SIZE_3D.flatten(UVec3::new(x, y, z));

                unsafe {
                    layers[y as usize].set_unchecked(index_in_layer, blocks[index_in_blocks])
                };
            }

            Self::Layered(layers)
        }
    }

    /// Returns the chunk data as an array of block IDs, ordered by y, then z, then x
    pub fn as_block_array(&self) -> Box<[BlockId]> {
        match self {
            Self::Uniform(block_id) => vec![*block_id; CHUNK_SIZE_CUBED].into_boxed_slice(),
            Self::Layered(layers) => (0..CHUNK_SIZE_CUBED)
                .map(|index| {
                    let y_index = index >> (CHUNK_SIZE_LOG2 * 2);
                    let xz_index = index & (CHUNK_SIZE_SQUARED - 1);
                    unsafe { layers[y_index].get_unchecked(xz_index) }
                })
                .collect(),
        }
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPos) -> BlockId {
        match self {
            Self::Uniform(block_id) => *block_id,
            Self::Layered(layers) => {
                let pos = pos.as_uvec3();
                let index_in_layer = CHUNK_SIZE_2D.flatten(UVec2::new(pos.x, pos.z));

                layers[pos.y as usize]
                    .get(index_in_layer)
                    .expect("index should be in layer")
            }
        }
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPos, new_id: BlockId) {
        let layers = match self {
            ChunkBlockStorage::Uniform(block_id) => {
                *self = Self::uniform_layers(*block_id);
                if let Self::Layered(layers) = self {
                    layers
                } else {
                    panic!()
                }
            }
            ChunkBlockStorage::Layered(layers) => layers,
        };

        let pos = pos.as_uvec3();
        let index_in_layer = CHUNK_SIZE_2D.flatten(UVec2::new(pos.x, pos.z));

        layers[pos.y as usize]
            .set(index_in_layer, new_id)
            .expect("index should be in layer")
    }

    /// Returns a `Self::Layered` where all layers contain the single value
    fn uniform_layers(block_id: BlockId) -> Self {
        Self::Layered(array_init::array_init(|_| {
            DictionaryEncodedArray::repeat(CHUNK_SIZE_SQUARED, block_id)
        }))
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
