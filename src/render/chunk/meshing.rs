use glam::{UVec2, UVec3, Vec2, Vec3};

use super::vertex::ChunkVertex;
use crate::{
    block::{model::BlockFace, BlockId, BLOCKS},
    terrain::chunk::{CHUNK_SIZE_SQUARED, CHUNK_SIZE_U32},
};

/// Data about a chunk needed to generate its mesh
#[derive(Clone, Copy)]
pub struct ChunkMeshInput<'a> {
    /// Array of blocks in the chunk, ordered by z, then y, then x
    pub blocks: &'a [BlockId],
    /// Translation to encode in the mesh
    pub translation: Vec3,
}

/// Creates the vertices for a chunk mesh where faces inside the volume are skipped but no
/// faces are merged.
/// The mesh should be rendered an index buffer that repeats the pattern 0, 1, 2, 2, 3, 0.
/// Compared to `mesh_greedy`, meshing is much faster but the resulting meshes
/// are more complex and therefore slower to render
pub fn mesh_culled(input: ChunkMeshInput) -> Vec<ChunkVertex> {
    let mut vertices = Vec::new();

    add_visible_faces::<face::PosX>(&mut vertices, input);
    add_visible_faces::<face::PosY>(&mut vertices, input);
    add_visible_faces::<face::PosZ>(&mut vertices, input);
    add_visible_faces::<face::NegX>(&mut vertices, input);
    add_visible_faces::<face::NegY>(&mut vertices, input);
    add_visible_faces::<face::NegZ>(&mut vertices, input);

    vertices
}

/// Creates a chunk mesh where faces inside the volume are skipped and
/// compatible faces are merged greedily.
/// Compared to `culled`, meshing is much slower but the resulting meshes
/// are simpler and therefore faster to render
pub fn mesh_greedy(input: ChunkMeshInput) -> Vec<ChunkVertex> {
    let mut vertices = Vec::new();

    add_greedy_merged_faces::<face::PosX>(&mut vertices, input);
    add_greedy_merged_faces::<face::PosY>(&mut vertices, input);
    add_greedy_merged_faces::<face::PosZ>(&mut vertices, input);
    add_greedy_merged_faces::<face::NegX>(&mut vertices, input);
    add_greedy_merged_faces::<face::NegY>(&mut vertices, input);
    add_greedy_merged_faces::<face::NegZ>(&mut vertices, input);

    vertices
}

/// Decides whether the two faces can be merged
fn can_merge_faces<FaceDir>(first: Option<BlockFace>, second: Option<BlockFace>) -> bool
where
    FaceDir: face::FaceDir,
{
    let faces_match = first == second;

    faces_match
}

/// Add a single axis-aligned face to the mesh
/// `origin` is the position of the cell with the smallest coordinates that this face covers
fn add_face<FaceDir>(
    vertices: &mut Vec<ChunkVertex>,
    origin: Vec3,
    size: Vec2,
    texture_index: usize,
) where
    FaceDir: face::FaceDir,
{
    let uvs = [[0.0, size.y], [size.x, size.y], [size.x, 0.0], [0.0, 0.0]];

    vertices.extend(
        FaceDir::vertices(size)
            .iter()
            .enumerate()
            .map(|(i, vertex_offset)| ChunkVertex {
                position: (origin + *vertex_offset).to_array(),
                uv: uvs[i],
                texture_index: texture_index as u32,
            }),
    );
}

/// Add all visible faces for the given face direction
fn add_visible_faces<FaceDir>(vertices: &mut Vec<ChunkVertex>, input: ChunkMeshInput)
where
    FaceDir: face::FaceDir,
{
    for pos_parallel_x in 0..CHUNK_SIZE_U32 {
        for pos_parallel_y in 0..CHUNK_SIZE_U32 {
            let mut visible = true;

            for pos_perpendicular in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = FaceDir::rotate_uvec3(UVec3::new(
                    pos_parallel_x,
                    pos_parallel_y,
                    // iterate backwards through the chunk
                    if FaceDir::NEGATIVE {
                        pos_perpendicular
                    } else {
                        (CHUNK_SIZE_U32 - 1) - pos_perpendicular
                    },
                ));

                let block_id = input.blocks[uvec3_to_chunk_index(pos_in_chunk)];
                let block_model = &BLOCKS[block_id.0 as usize].model;

                let face = block_model.face(FaceDir::FACE_INDEX);
                if let Some(face) = face {
                    if visible {
                        add_face::<FaceDir>(
                            vertices,
                            pos_in_chunk.as_vec3() + input.translation,
                            Vec2::ONE,
                            face.texture_index,
                        );
                    }
                }

                visible = block_model
                    .face(FaceDir::OPPOSITE_FACE_INDEX)
                    .is_none();
            }
        }
    }
}

/// Greedily merge visible faces with the given direction and add them to the mesh
fn add_greedy_merged_faces<FaceDir>(vertices: &mut Vec<ChunkVertex>, input: ChunkMeshInput)
where
    FaceDir: face::FaceDir,
{
    // references:
    // - https://eddieabbondanz.io/post/voxel/greedy-mesh/

    // note about coordinates:
    //   U and V refer to the cardinal directions perpendicular to the face direction
    //   U is the direction of the first texture coordinate
    //   V is the direction of the second texture coordinate

    /// Evaluate whether the original face can be merged with the face with coordinates
    /// `merge_candidate_u` and `merge_candidate_v` in the layer with position `layer_pos`
    /// returns two booleans: whether the face can be merged, and whether the block with the
    /// same U and V coordinates in the following layer is visible
    fn consider_merge_candidate<FaceDir>(
        blocks: &[BlockId],
        visible: &[bool; CHUNK_SIZE_SQUARED],
        layer_pos: u32,
        original_face: BlockFace,
        merge_candidate_u: u32,
        merge_candidate_v: u32,
    ) -> (bool, bool)
    where
        FaceDir: face::FaceDir,
    {
        let merge_candidate_pos = UVec3::new(merge_candidate_u, merge_candidate_v, layer_pos);
        let merge_candidate_pos = FaceDir::rotate_uvec3(merge_candidate_pos);

        let merge_candidate_index_in_layer =
            (CHUNK_SIZE_U32 * merge_candidate_v + merge_candidate_u) as usize;

        let merge_candidate_id = blocks[uvec3_to_chunk_index(merge_candidate_pos) as usize];
        let merge_candidate_model = &BLOCKS[merge_candidate_id.0 as usize].model;
        let merge_candidate_face = merge_candidate_model.face(FaceDir::FACE_INDEX);
        let merge_candidate_visible = visible[merge_candidate_index_in_layer];

        let can_merge = can_merge_faces::<FaceDir>(Some(original_face), merge_candidate_face)
            && merge_candidate_visible;
        let next_visible = merge_candidate_model
            .face(FaceDir::OPPOSITE_FACE_INDEX)
            .is_none();

        (can_merge, next_visible)
    }

    // this will track whether each face in the next layer is visible
    // a face is visible if the block in the previous layer had no face in
    // the opposite direction
    let mut visible = [true; CHUNK_SIZE_SQUARED];

    // iterate over each layer of faces we will create
    for layer_index in 0..CHUNK_SIZE_U32 {
        // position of this layer, moving backwards through the chunk with respect to the face
        // direction
        let layer_pos = if FaceDir::NEGATIVE {
            layer_index
        } else {
            (CHUNK_SIZE_U32 - 1) - layer_index
        };

        // this will track which faces have already been merged with another
        // already merged faces can safely be ignored
        let mut already_merged = [false; CHUNK_SIZE_SQUARED];

        // iterate over each block in the layer
        for original_v in 0..CHUNK_SIZE_U32 {
            for original_u in 0..CHUNK_SIZE_U32 {
                // index of this block in the current layer
                let original_index = (original_v * CHUNK_SIZE_U32 + original_u) as usize;

                // skip if already merged
                if already_merged[original_index] {
                    continue;
                }

                // position of this block in the chunk
                let original_pos =
                    FaceDir::rotate_uvec3(UVec3::new(original_u, original_v, layer_pos));

                let original_id = input.blocks[uvec3_to_chunk_index(original_pos) as usize];
                let original_model = &BLOCKS[original_id.0 as usize].model;
                let original_face = original_model.face(FaceDir::FACE_INDEX);
                let original_visible = visible[original_index];

                // update `visible` for the next layer
                visible[original_index] = original_model
                    .face(FaceDir::OPPOSITE_FACE_INDEX)
                    .is_none();

                // skip if there is no face or the face is invisible
                if original_face.is_none() || !original_visible {
                    continue;
                }
                let original_face = original_face.unwrap();

                // march to see how many faces can be merged in the U direction
                let mut face_size = UVec2::ONE;
                for merge_candidate_u in (original_u + 1)..CHUNK_SIZE_U32 {
                    let (can_merge, next_visible) = consider_merge_candidate::<FaceDir>(
                        input.blocks,
                        &visible,
                        layer_pos,
                        original_face,
                        merge_candidate_u,
                        original_v,
                    );

                    // stop counting when we can't merge any more faces
                    if !can_merge {
                        break;
                    }

                    let merged_index_in_layer =
                        (CHUNK_SIZE_U32 * original_v + merge_candidate_u) as usize;

                    // grow the face
                    face_size.x += 1;

                    // mark that this face is already merged
                    already_merged[merged_index_in_layer] = true;

                    // update `visible` for the same block in the next layer
                    // (this would not otherwise occur)
                    visible[merged_index_in_layer] = next_visible;
                }

                // march to see how many faces can be merged in the V direction
                'v: for merge_candidate_v in (original_v + 1)..CHUNK_SIZE_U32 {
                    // bit flags for whether the block adjacent to a block being considered for
                    // merging will be visible
                    // this avoids having to check the model again once it has been decided
                    // the layers can be merged
                    let mut visibility_flags: u32 = 0;

                    // see if we can merge the next layer down by checking all blocks on this
                    // layer in the U direction
                    for merge_candidate_u in original_u..(original_u + face_size.x) {
                        let (can_merge, next_visible) = consider_merge_candidate::<FaceDir>(
                            input.blocks,
                            &visible,
                            layer_pos,
                            original_face,
                            merge_candidate_u,
                            merge_candidate_v,
                        );

                        // stop counting when we can't merge any more faces
                        if !can_merge {
                            break 'v;
                        }

                        // update visibility flags
                        visibility_flags |= (next_visible as u32) << merge_candidate_u;
                    }

                    // merge layers
                    face_size.y += 1;

                    // mark all faces in the layer as merged
                    for merged_x in original_u..(original_u + face_size.x) {
                        let merged_index_in_layer =
                            (merge_candidate_v * CHUNK_SIZE_U32 + merged_x) as usize;

                        already_merged[merged_index_in_layer] = true;

                        // update `visible` for the same block in the next layer
                        // visibility flags already computed
                        // (this would not otherwise occur)
                        visible[merged_index_in_layer] = (visibility_flags & (1 << merged_x)) != 0;
                    }
                }

                // create the merged face
                add_face::<FaceDir>(
                    vertices,
                    original_pos.as_vec3() + input.translation,
                    face_size.as_vec2(),
                    original_face.texture_index,
                );
            }
        }
    }
}

/// Generate indices for the meshes returned by `mesh_culled` and `mesh_greedy`
pub fn generate_indices(vertex_count: usize) -> Vec<u32> {
    const INDICES: [u32; 6] = [0, 1, 2, 2, 3, 0];

    let index_count = vertex_count / 2 * 3;
    let mut indices = Vec::with_capacity(index_count);

    for i in 0..((index_count / 6) as u32) {
        let first_index = i * 4;
        indices.push(INDICES[0] + first_index);
        indices.push(INDICES[1] + first_index);
        indices.push(INDICES[2] + first_index);
        indices.push(INDICES[3] + first_index);
        indices.push(INDICES[4] + first_index);
        indices.push(INDICES[5] + first_index);
    }

    indices
}

fn uvec3_to_chunk_index(pos: UVec3) -> usize {
    ((CHUNK_SIZE_U32 * CHUNK_SIZE_U32) * pos.z + CHUNK_SIZE_U32 * pos.y + pos.x) as usize
}

mod face {
    use glam::{UVec3, Vec2, Vec3, Vec3Swizzles};

    use crate::block::model::BlockFaceIndex;

    /// Face directions
    pub trait FaceDir {
        /// Index assigned to this face direction
        const FACE_INDEX: BlockFaceIndex;

        /// Index assigned to the opposite face direction
        const OPPOSITE_FACE_INDEX: BlockFaceIndex;

        /// Whether this face direction points away from its axis
        const NEGATIVE: bool;

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
        const FACE_INDEX: BlockFaceIndex = BlockFaceIndex::POS_X;
        const OPPOSITE_FACE_INDEX: BlockFaceIndex = BlockFaceIndex::NEG_X;
        const NEGATIVE: bool = false;

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
        const FACE_INDEX: BlockFaceIndex = BlockFaceIndex::POS_Y;
        const OPPOSITE_FACE_INDEX: BlockFaceIndex = BlockFaceIndex::NEG_Y;
        const NEGATIVE: bool = false;

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
        const FACE_INDEX: BlockFaceIndex = BlockFaceIndex::POS_Z;
        const OPPOSITE_FACE_INDEX: BlockFaceIndex = BlockFaceIndex::NEG_Z;
        const NEGATIVE: bool = false;

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
        const FACE_INDEX: BlockFaceIndex = BlockFaceIndex::NEG_X;
        const OPPOSITE_FACE_INDEX: BlockFaceIndex = BlockFaceIndex::POS_X;
        const NEGATIVE: bool = true;

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
        const FACE_INDEX: BlockFaceIndex = BlockFaceIndex::NEG_Y;
        const OPPOSITE_FACE_INDEX: BlockFaceIndex = BlockFaceIndex::POS_Y;
        const NEGATIVE: bool = true;

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
        const FACE_INDEX: BlockFaceIndex = BlockFaceIndex::NEG_Z;
        const OPPOSITE_FACE_INDEX: BlockFaceIndex = BlockFaceIndex::POS_Z;
        const NEGATIVE: bool = true;

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
}
