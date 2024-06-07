#[derive(Clone, Debug)]
pub enum BlockModel {
    Empty,
    FullBlock([BlockFace; 6]),
}

impl BlockModel {
    pub fn face(&self, face_index: BlockFaceIndex) -> Option<BlockFace> {
        match self {
            BlockModel::Empty => None,
            BlockModel::FullBlock(faces) => Some(faces[face_index.as_usize()]),
        }
    }
}

/// represents one axis-aligned face of a block model
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockFace {
    pub texture_index: usize,
}

pub struct BlockFaceIndex(usize);

impl BlockFaceIndex {
    pub const POS_X: BlockFaceIndex = BlockFaceIndex(0);
    pub const POS_Y: BlockFaceIndex = BlockFaceIndex(1);
    pub const POS_Z: BlockFaceIndex = BlockFaceIndex(2);
    pub const NEG_X: BlockFaceIndex = BlockFaceIndex(3);
    pub const NEG_Y: BlockFaceIndex = BlockFaceIndex(4);
    pub const NEG_Z: BlockFaceIndex = BlockFaceIndex(5);

    pub fn as_usize(self) -> usize {
        self.0
    }
}
