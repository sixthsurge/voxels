use glam::IVec3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FaceIndex(pub usize);

impl FaceIndex {
    pub const POS_X: FaceIndex = FaceIndex(0);
    pub const POS_Y: FaceIndex = FaceIndex(1);
    pub const POS_Z: FaceIndex = FaceIndex(2);
    pub const NEG_X: FaceIndex = FaceIndex(3);
    pub const NEG_Y: FaceIndex = FaceIndex(4);
    pub const NEG_Z: FaceIndex = FaceIndex(5);

    /// Returns the index of the opposite face, e.g. +x -> -x
    pub fn opposite(self) -> Self {
        Self((self.0 + 3) % 6)
    }

    pub fn as_usize(self) -> usize {
        self.0
    }
}

pub const FACE_NORMALS: [IVec3; 6] = [
    IVec3::X,
    IVec3::Y,
    IVec3::Z,
    IVec3::NEG_X,
    IVec3::NEG_Y,
    IVec3::NEG_Z,
];
