#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub u32);

#[derive(Clone, Debug)]
pub struct Block {
    pub model: BlockModel,
}

#[derive(Clone, Debug)]
pub enum BlockModel {
    Empty,
    FullBlock(usize),
}

impl BlockModel {
    pub fn has_face(&self, face_index: usize) -> bool {
        match self {
            &BlockModel::Empty => false,
            &BlockModel::FullBlock(_) => true,
        }
    }

    pub fn texture_index(&self) -> usize {
        match self {
            &BlockModel::Empty => 0,
            &BlockModel::FullBlock(i) => i,
        }
    }
}

pub enum BlockFace {
    PosX,
    PosY,
    PosZ,
    NegX,
    NegY,
    NegZ,
}

pub const BLOCK_AIR: BlockId = BlockId(0);
pub const BLOCK_HAPPY: BlockId = BlockId(1);
pub const BLOCK_SAD: BlockId = BlockId(2);
pub const BLOCK_COUNT: usize = 3;

pub const BLOCKS: [Block; BLOCK_COUNT] = [
    // Air
    Block {
        model: BlockModel::Empty,
    },
    // White
    Block {
        model: BlockModel::FullBlock(0),
    },
    // White
    Block {
        model: BlockModel::FullBlock(1),
    },
];
