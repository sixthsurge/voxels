use glam::{UVec3, Vec2, Vec3, Vec3Swizzles};

use crate::util::face_index::FaceIndex;

/// Face directions
pub trait FaceDir {
    /// Index assigned to this face direction
    const FACE_INDEX: FaceIndex;

    /// Index assigned to the opposite face direction
    const OPPOSITE_FACE_INDEX: FaceIndex;

    /// Whether this face direction points away from its axis
    const NEGATIVE: bool;

    /// Directional shading for this face
    const SHADING: f32;

    /// Returns the 4 vertices for a face pointing in this direction
    /// * `size`: The size of the face on the two perpendicular directions
    /// When looking at the face head on, the first vertex is at the bottom left and the
    /// following vertices proceed anticlockwise
    fn vertices(size: Vec2) -> [Vec3; 4];

    /// Given a vector whose x and y components are specified parallel to the face and whose z
    /// component is specified perpendicular to the face, converts it to absolute coordinates
    /// by swizzling
    /// rotate_vec3(Vec3::new(0.0, 0.0, 1.0)) gives the axis of the face
    /// rotate_vec3(Vec3::new(1.0, 0.0, 0.0)) gives a tangent of the face
    /// rotate_vec3(Vec3::new(0.0, 1.0, 0.0)) gives another tangent of the face
    fn rotate_vec3(v: Vec3) -> Vec3;

    /// Given a vector whose x and y components are specified parallel to the face and whose z
    /// component is specified perpendicular to the face, converts it to absolute coordinates by
    /// swizzling
    /// rotate_uvec3(UVec3::new(0, 0, 1)) gives the axis of the face
    /// rotate_uvec3(UVec3::new(1, 0, 0)) gives a tangent of the face
    /// rotate_uvec3(UVec3::new(0, 1, 0)) gives another tangent of the face
    fn rotate_uvec3(v: UVec3) -> UVec3;
}

/// +x
pub struct PosX;

impl FaceDir for PosX {
    const FACE_INDEX: FaceIndex = FaceIndex::POS_X;
    const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::NEG_X;
    const NEGATIVE: bool = false;
    const SHADING: f32 = 0.7;

    fn vertices(size: Vec2) -> [Vec3; 4] {
        [
            Vec3::new(1.0, 0.0, size.x),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(1.0, size.y, 0.0),
            Vec3::new(1.0, size.y, size.x),
        ]
    }

    fn rotate_vec3(v: Vec3) -> Vec3 {
        v.zyx()
    }

    fn rotate_uvec3(v: UVec3) -> UVec3 {
        v.zyx()
    }
}

/// +y
pub struct PosY;

impl FaceDir for PosY {
    const FACE_INDEX: FaceIndex = FaceIndex::POS_Y;
    const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::NEG_Y;
    const NEGATIVE: bool = false;
    const SHADING: f32 = 1.0;

    fn vertices(size: Vec2) -> [Vec3; 4] {
        [
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 1.0, size.x),
            Vec3::new(size.y, 1.0, size.x),
            Vec3::new(size.y, 1.0, 0.0),
        ]
    }

    fn rotate_vec3(v: Vec3) -> Vec3 {
        v.yzx()
    }

    fn rotate_uvec3(v: UVec3) -> UVec3 {
        v.yzx()
    }
}

/// +z
pub struct PosZ;

impl FaceDir for PosZ {
    const FACE_INDEX: FaceIndex = FaceIndex::POS_Z;
    const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::NEG_Z;
    const NEGATIVE: bool = false;
    const SHADING: f32 = 0.8;

    fn vertices(size: Vec2) -> [Vec3; 4] {
        [
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(size.x, 0.0, 1.0),
            Vec3::new(size.x, size.y, 1.0),
            Vec3::new(0.0, size.y, 1.0),
        ]
    }

    fn rotate_vec3(v: Vec3) -> Vec3 {
        v
    }

    fn rotate_uvec3(v: UVec3) -> UVec3 {
        v
    }
}

/// -x
pub struct NegX;

impl FaceDir for NegX {
    const FACE_INDEX: FaceIndex = FaceIndex::NEG_X;
    const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::POS_X;
    const NEGATIVE: bool = true;
    const SHADING: f32 = 0.7;

    fn vertices(size: Vec2) -> [Vec3; 4] {
        [
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, size.x),
            Vec3::new(0.0, size.y, size.x),
            Vec3::new(0.0, size.y, 0.0),
        ]
    }

    fn rotate_vec3(v: Vec3) -> Vec3 {
        v.zyx()
    }

    fn rotate_uvec3(v: UVec3) -> UVec3 {
        v.zyx()
    }
}

/// -y
pub struct NegY;

impl FaceDir for NegY {
    const FACE_INDEX: FaceIndex = FaceIndex::NEG_Y;
    const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::POS_Y;
    const NEGATIVE: bool = true;
    const SHADING: f32 = 0.5;

    fn vertices(size: Vec2) -> [Vec3; 4] {
        [
            Vec3::new(size.y, 0.0, 0.0),
            Vec3::new(size.y, 0.0, size.x),
            Vec3::new(0.0, 0.0, size.x),
            Vec3::new(0.0, 0.0, 0.0),
        ]
    }

    fn rotate_vec3(v: Vec3) -> Vec3 {
        v.yzx()
    }

    fn rotate_uvec3(v: UVec3) -> UVec3 {
        v.yzx()
    }
}

/// -z
pub struct NegZ;

impl FaceDir for NegZ {
    const FACE_INDEX: FaceIndex = FaceIndex::NEG_Z;
    const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::POS_Z;
    const NEGATIVE: bool = true;
    const SHADING: f32 = 0.6;

    fn vertices(size: Vec2) -> [Vec3; 4] {
        [
            Vec3::new(size.x, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, size.y, 0.0),
            Vec3::new(size.x, size.y, 0.0),
        ]
    }

    fn rotate_vec3(v: Vec3) -> Vec3 {
        v
    }

    fn rotate_uvec3(v: UVec3) -> UVec3 {
        v
    }
}
