use std::{mem::MaybeUninit, usize};

use either::Either;
use itertools::{repeat_n, Itertools};
use rustc_hash::{FxHashMap, FxHashSet};

use crate::{
    block::BlockId,
    terrain::{
        chunk::{CHUNK_SIZE_CUBED, CHUNK_SIZE_U32},
        position_types::LocalBlockPosition,
    },
};

use super::{CHUNK_SIZE, CHUNK_SIZE_SQUARED};

/// Storage for a chunk's block data in memory
#[derive(Clone, Debug)]
pub enum ChunkBlockStorage {
    Uniform(BlockId),
    Layered([BlockLayer; CHUNK_SIZE]),
}

impl ChunkBlockStorage {
    /// Initialize the block storage from an array of block IDs, ordered by y, then z, then x
    pub fn new(blocks: Vec<BlockId>) -> Self {
        debug_assert!(blocks.len() == CHUNK_SIZE_CUBED);

        if let Ok(&block_id) = blocks.iter().all_equal_value() {
            Self::Uniform(block_id)
        } else {
            // safe because `MaybeUninit` does not require initialization
            let mut layers: [MaybeUninit<BlockLayer>; CHUNK_SIZE] =
                unsafe { MaybeUninit::uninit().assume_init() };

            for (layer_index, layer_data) in blocks.chunks_exact(CHUNK_SIZE_SQUARED).enumerate() {
                layers[layer_index].write(BlockLayer::new(layer_data));
            }

            // safe because all layers have been initialized
            // (given the assertion that blocks.len() == CHUNK_SIZE_CUBED)
            Self::Layered(unsafe { std::mem::transmute(layers) })
        }
    }

    /// Returns the chunk data as an array of block IDs, ordered by y, then z, then x
    pub fn as_block_array(&self) -> Box<[BlockId]> {
        match self {
            Self::Uniform(block_id) => vec![*block_id; CHUNK_SIZE_CUBED].into_boxed_slice(),
            Self::Layered(layers) => {
                let mut v = Vec::with_capacity(CHUNK_SIZE_CUBED);

                for layer in layers {
                    v.extend(layer.iter());
                }

                v.into_boxed_slice()
            }
        }
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn get_block(&self, pos: LocalBlockPosition) -> BlockId {
        match self {
            Self::Uniform(block_id) => *block_id,
            Self::Layered(layers) => layers[pos.y() as usize].get(pos.x(), pos.z()),
        }
    }

    /// Returns the block ID at the given position
    /// panics if the position is out of bounds
    pub fn set_block(&mut self, pos: LocalBlockPosition, new_id: BlockId) {
        let set_block_in_layers = |layers: &mut [BlockLayer]| {
            layers[pos.y() as usize].set(pos.x(), pos.z(), new_id);
        };

        match self {
            ChunkBlockStorage::Uniform(block_id) => {
                let mut layers = array_init::array_init(|_| BlockLayer::new(&[*block_id]));
                set_block_in_layers(&mut layers);
                *self = Self::Layered(layers);
            }
            ChunkBlockStorage::Layered(layers) => set_block_in_layers(layers),
        };
    }
}

/// Represents a horizontal layer of blocks in a chunk.
/// Layers are compressed using a block palette, where each block is encoded as
/// the index in the palette of that block.
/// The length of each index is the smallest power of 2 number of bits needed to
/// store the highest index.
/// The length of the indices is confined to be a power of two so that each index
/// will be stored in exactly one `usize` in the underlying array, rather than being
/// split over multiple `usize`s. This makes accessing the data much faster at the
/// expense of compression.
#[derive(Clone, Debug)]
struct BlockLayer {
    segments: Box<[usize]>,
    element_size_bits: usize,
    palette: BlockPalette,
}

impl BlockLayer {
    /// - `blocks` may have length 1, indicating that the layer is comprised
    ///   of a single block type, or CHUNK_SIZE_SQUARED
    fn new(blocks: &[BlockId]) -> Self {
        debug_assert!(blocks.len() == 1 || blocks.len() == CHUNK_SIZE_SQUARED);

        let palette = BlockPalette::new(blocks);
        let (segments, element_size_bits) = Self::compress(blocks, &palette);

        Self {
            segments,
            element_size_bits,
            palette,
        }
    }

    fn get(&self, x: u32, y: u32) -> BlockId {
        if self.element_size_bits == 0 {
            self.palette.get_block_for_index(0)
        } else {
            let (segment_index, bit_index_in_segment) =
                self.get_segment_index_and_bit_index_in_segment(x, y);

            let palette_index = mod_pow2(
                self.segments[segment_index] >> bit_index_in_segment,
                1 << self.element_size_bits,
            );

            self.palette.get_block_for_index(palette_index)
        }
    }

    fn set(&mut self, x: u32, y: u32, block_id: BlockId) {
        // find the palette index for this block, adding it if necessary
        let palette_index = self.palette.get_or_add_index_for_block(block_id);
        let element_size_bits = self.palette.index_size_bits();

        // resize the elements if necessary
        if self.element_size_bits != element_size_bits {
            self.resize_elements(element_size_bits);
        }

        // set the corresponding bits
        let (segment_index, bit_index_in_segment) =
            self.get_segment_index_and_bit_index_in_segment(x, y);

        self.segments[segment_index] &= !(((1 << element_size_bits) - 1) << bit_index_in_segment); // clear
        self.segments[segment_index] |= palette_index << bit_index_in_segment; // set
    }

    fn iter<'a>(&'a self) -> impl Iterator<Item = BlockId> + 'a {
        if self.element_size_bits == 0 {
            Either::Left(repeat_n(
                self.palette.get_block_for_index(0),
                CHUNK_SIZE_SQUARED,
            ))
        } else {
            let elements_per_segment = div_pow2(CHUNK_SIZE_SQUARED, self.segments.len());

            Either::Right(self.segments.iter().flat_map(move |segment_value| {
                (0..elements_per_segment).map(move |index_in_segment| {
                    let bit_index_in_segment = index_in_segment * self.element_size_bits;

                    let palette_index = mod_pow2(
                        segment_value >> bit_index_in_segment,
                        1 << self.element_size_bits,
                    );

                    self.palette.get_block_for_index(palette_index)
                })
            }))
        }
    }

    fn resize_elements(&mut self, new_element_size_bits: usize) {
        let elements_per_segment = div_pow2(usize::BITS as usize, new_element_size_bits);
        let segment_count = div_pow2(CHUNK_SIZE_SQUARED, elements_per_segment);

        if self.element_size_bits == 0 {
            self.segments = repeat_n(0, segment_count).collect();
            self.element_size_bits = new_element_size_bits;
            return;
        }

        let mut old_bit_index = 0;

        self.segments = (0..segment_count)
            .map(|_| {
                let mut segment_value = 0;

                for index_in_segment in 0..elements_per_segment {
                    let palette_index = {
                        let segment_index = div_pow2(old_bit_index, usize::BITS as usize);
                        let bit_index_in_segment = mod_pow2(old_bit_index, usize::BITS as usize);

                        mod_pow2(
                            self.segments[segment_index] >> bit_index_in_segment,
                            1 << self.element_size_bits,
                        )
                    };

                    segment_value |= palette_index << (new_element_size_bits * index_in_segment);
                    old_bit_index += self.element_size_bits;
                }

                segment_value
            })
            .collect();
        self.element_size_bits = new_element_size_bits;
    }

    /// Returns the index of the segment containing the block at the given
    /// position and the index of the first bit of the section within that
    /// segment respresenting this position
    fn get_segment_index_and_bit_index_in_segment(&self, x: u32, y: u32) -> (usize, usize) {
        let block_index = CHUNK_SIZE * (y as usize) + (x as usize);
        let bit_index = block_index * self.element_size_bits;
        let segment_index = div_pow2(bit_index, usize::BITS as usize);
        let bit_index_in_segment = mod_pow2(bit_index, usize::BITS as usize);

        (segment_index, bit_index_in_segment)
    }

    fn compress(blocks: &[BlockId], palette: &BlockPalette) -> (Box<[usize]>, usize) {
        if palette.len() <= 1 {
            return (Box::new([]), 0);
        }

        let element_size_bits = palette.index_size_bits();

        let elements_per_segment = div_pow2(usize::BITS as usize, element_size_bits);
        let segment_count = div_pow2(CHUNK_SIZE_SQUARED, elements_per_segment);
        let mut block_index = 0;

        let segments = (0..segment_count)
            .map(|_| {
                let mut segment_value = 0;

                for index_in_segment in 0..elements_per_segment {
                    let palette_index = palette.index_lookup[&blocks[block_index]];
                    segment_value |= palette_index << (index_in_segment * element_size_bits);
                    block_index += 1;
                }

                segment_value
            })
            .collect();

        (segments, element_size_bits)
    }
}

#[derive(Clone, Debug)]
struct BlockPalette {
    block_lookup: Vec<BlockId>,
    index_lookup: FxHashMap<BlockId, usize>,
}

impl BlockPalette {
    /// Create a new `BlockPalette` including the given blocks
    fn new(blocks: &[BlockId]) -> Self {
        let mut block_lookup = Vec::new();
        let mut index_lookup = FxHashMap::default();

        for block_id in blocks.iter().copied() {
            if !index_lookup.contains_key(&block_id) {
                let index = block_lookup.len();

                block_lookup.push(block_id);
                index_lookup.insert(block_id, index);
            }
        }

        Self {
            block_lookup,
            index_lookup,
        }
    }

    /// If the block palette has an entry for the given block, returns the
    /// index of the block in the palette.
    /// Otherwise adds the given block to the palette and returns its new index.
    fn get_or_add_index_for_block(&mut self, block_id: BlockId) -> usize {
        if let Some(&index) = self.index_lookup.get(&block_id) {
            index
        } else {
            let index = self.block_lookup.len();

            self.block_lookup.push(block_id);
            self.index_lookup.insert(block_id, index);

            index
        }
    }

    /// Returns the block ID associated with the given index in the palette.
    /// Panics if the index is out of bounds
    fn get_block_for_index(&self, index: usize) -> BlockId {
        self.block_lookup[index]
    }

    fn len(&self) -> usize {
        self.block_lookup.len()
    }

    fn index_size_bits(&self) -> usize {
        ceil_log2(self.block_lookup.len()).next_power_of_two() as usize
    }
}

/// Returns the value of `ceil(log_2(x))`
/// Panics if x == 0
#[inline(always)]
const fn ceil_log2(x: usize) -> u32 {
    usize::BITS - (x - 1).leading_zeros()
}

/// Returns the value of `a / b` when `b` is a power of two
/// Panics if x == 0
#[inline(always)]
const fn div_pow2(a: usize, b: usize) -> usize {
    a >> (ceil_log2(b) as usize)
}

/// Returns the value of `a % b` when `b` is a power of two
#[inline(always)]
const fn mod_pow2(a: usize, b: usize) -> usize {
    a & (b - 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_layer() {
        {
            let blocks = (0..CHUNK_SIZE_SQUARED)
                .map(|i| BlockId(i as u16))
                .collect_vec();

            let mut layer = BlockLayer::new(&blocks);

            assert_eq!(layer.get(0, 0), BlockId(0));
            assert_eq!(layer.get(1, 0), BlockId(1));
            assert_eq!(layer.get(2, 0), BlockId(2));

            for (left, right) in layer.iter().zip(blocks.iter().copied()) {
                assert_eq!(left, right);
            }

            layer.set(7, 4, BlockId(4096));
            assert_eq!(layer.get(7, 4), BlockId(4096));
        }

        {
            let mut layer = BlockLayer::new(&[BlockId(0); CHUNK_SIZE_SQUARED]);

            layer.set(0, 0, BlockId(1));
            layer.set(1, 0, BlockId(2));
            layer.set(2, 0, BlockId(3));

            assert_eq!(layer.get(0, 0), BlockId(1));
            assert_eq!(layer.get(1, 0), BlockId(2));
            assert_eq!(layer.get(2, 0), BlockId(3));
            assert_eq!(layer.get(3, 0), BlockId(0));
        }
    }
}
