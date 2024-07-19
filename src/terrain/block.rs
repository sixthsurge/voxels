use glam::IVec3;

use self::model::{BlockFace, BlockModel};

pub mod model;

/// Numeric identifier for a `Block`
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BlockId(pub u16);

/// Represents a kind of block in the world
#[derive(Clone, Debug)]
pub struct Block {
    pub model: BlockModel,
    pub emission: IVec3,
}

// ----------------------------------------------------------------------------
// temporary block registry
// TODO: replace this with a proper system for registering block types
pub const BLOCK_AIR: BlockId = BlockId(0);
pub const BLOCK_DIRT: BlockId = BlockId(1);
pub const BLOCK_GRASS: BlockId = BlockId(2);
pub const BLOCK_WOOD: BlockId = BlockId(3);
pub const BLOCK_LAMP_ORANGE: BlockId = BlockId(4);
pub const BLOCK_COUNT: usize = 5;

pub const BLOCKS: [Block; BLOCK_COUNT] = [
    // Air
    Block {
        model: BlockModel::Empty,
        emission: IVec3::ZERO,
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
        emission: IVec3::ZERO,
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
        emission: IVec3::ZERO,
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
        emission: IVec3::ZERO,
    },
    // Orange lamp
    Block {
        model: BlockModel::FullBlock([
            BlockFace { texture_index: 4 },
            BlockFace { texture_index: 4 },
            BlockFace { texture_index: 4 },
            BlockFace { texture_index: 4 },
            BlockFace { texture_index: 4 },
            BlockFace { texture_index: 4 },
        ]),
        emission: IVec3::new(15, 10, 5),
    },
];
