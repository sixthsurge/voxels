use self::model::{BlockFace, BlockModel};

pub mod model;

/// Numeric identifier for a `Block`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub u16);

/// Represents a kind of block in the world
#[derive(Clone, Debug)]
pub struct Block {
    pub model: BlockModel,
}

// ----------------------------------------------------------------------------
// temporary block registry
// TODO: replace this with a proper system for registering block types
pub const BLOCK_AIR: BlockId = BlockId(0);
pub const BLOCK_DIRT: BlockId = BlockId(1);
pub const BLOCK_GRASS: BlockId = BlockId(2);
pub const BLOCK_WOOD: BlockId = BlockId(3);
pub const BLOCK_COUNT: usize = 4;

pub const BLOCKS: [Block; BLOCK_COUNT] = [
    // Air
    Block {
        model: BlockModel::Empty,
    },
    // Dirt
    Block {
        model: BlockModel::FullBlock([
            BlockFace { texture_index: 0 },
            BlockFace { texture_index: 0 },
            BlockFace { texture_index: 0 },
            BlockFace { texture_index: 0 },
            BlockFace { texture_index: 0 },
            BlockFace { texture_index: 0 },
        ]),
    },
    // Grass
    Block {
        model: BlockModel::FullBlock([
            BlockFace { texture_index: 1 },
            BlockFace { texture_index: 2 },
            BlockFace { texture_index: 1 },
            BlockFace { texture_index: 1 },
            BlockFace { texture_index: 0 },
            BlockFace { texture_index: 1 },
        ]),
    },
    // Wood
    Block {
        model: BlockModel::FullBlock([
            BlockFace { texture_index: 3 },
            BlockFace { texture_index: 3 },
            BlockFace { texture_index: 3 },
            BlockFace { texture_index: 3 },
            BlockFace { texture_index: 3 },
            BlockFace { texture_index: 3 },
        ]),
    },
];
