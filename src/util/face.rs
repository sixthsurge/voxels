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

    pub fn from_dir(dir: IVec3) -> Option<FaceIndex> {
        let face_index = FaceIndex(
            (IVec3::dot(dir, IVec3::new(0, 1, 2)).abs() + 3 * (dir.max_element() == 0) as i32)
                as usize,
        );

        if Some(dir) == FACE_NORMALS.get(face_index.as_usize()).copied() {
            Some(face_index)
        } else {
            None
        }
    }

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

// Direction of the first texture coordinate
pub const FACE_TANGENTS: [IVec3; 6] = [
    IVec3::Z,
    IVec3::Z,
    IVec3::X,
    IVec3::NEG_Z,
    IVec3::NEG_Z,
    IVec3::NEG_X,
];

// Direction of the second texture coordinate
pub const FACE_BITANGENTS: [IVec3; 6] = [
    IVec3::Y,
    IVec3::X,
    IVec3::Y,
    IVec3::NEG_Y,
    IVec3::NEG_X,
    IVec3::NEG_Y,
];
