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

    // Returns a bitmask of the faces of the block that are opaque, i.e. light
    // cannot pass through that face
    // If the result is all zeroes, the block is completely transparent
    pub fn opaque_faces_mask(&self) -> u8 {
        match self {
            BlockModel::Empty => 0b000000,
            BlockModel::FullBlock(_) => 0b111111,
        }
    }

    // True if light can pass through the given face of the block
    pub fn is_transparent_in_direction(&self, face_index: FaceIndex) -> bool {
        self.opaque_faces_mask() & (1 << face_index.as_usize()) == 0
    }
}

/// represents one axis-aligned face of a block model
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BlockFace {
    pub texture_index: usize,
}
