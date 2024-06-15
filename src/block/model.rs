use crate::util::face::FaceIndex;

#[derive(Clone, Debug)]
pub enum BlockModel {
    Empty,
    FullBlock([BlockFace; 6]),
}

impl BlockModel {
    pub fn face(&self, face_index: FaceIndex) -> Option<BlockFace> {
        match self {
            BlockModel::Empty => None,
            BlockModel::FullBlock(faces) => Some(faces[face_index.as_usize()]),
        }
    }

    pub fn is_opaque(&self) -> bool {
        match self {
            BlockModel::Empty => false,
            BlockModel::FullBlock(_) => true,
        }
    }
}

/// represents one axis-aligned face of a block model
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockFace {
    pub texture_index: usize,
}
