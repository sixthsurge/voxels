#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Clone, Debug)]
pub struct Block {
    model: BlockModel,
}

#[derive(Clone, Debug)]
pub enum BlockModel {
    Empty,
    FullBlock,
}

pub const BLOCK_AIR: BlockId = BlockId(0);
pub const BLOCK_WHITE: BlockId = BlockId(1);
pub const BLOCK_COUNT: usize = 2;

pub const BLOCKS: [Block; BLOCK_COUNT] = [
    // Air
    Block {
        model: BlockModel::Empty,
    },
    // White
    Block {
        model: BlockModel::FullBlock,
    },
];
