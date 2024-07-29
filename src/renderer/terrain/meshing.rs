use glam::{IVec3, UVec2, UVec3, Vec2, Vec3, Vec4};

use self::face_dir::*;
use super::vertex::TerrainVertex;
use crate::{
    terrain::{
        block::{model::BlockFace, BlockId, BLOCKS},
        chunk::{
            light_store::ChunkLightStore,
            side::{ChunkSideFaces, ChunkSideLight},
            CHUNK_SIZE, CHUNK_SIZE_I32, CHUNK_SIZE_SQUARED, CHUNK_SIZE_U32,
        },
        lighting::{emitted_light::EmittedLight, LightStore},
        position_types::LocalBlockPosition,
    },
    util::face::{FaceIndex, FACE_BITANGENTS, FACE_TANGENTS},
};

/// Data about a chunk needed to generate its mesh
#[derive(Clone, Copy)]
pub struct ChunkMeshInput<'a> {
    /// Array of blocks in the chunk, ordered by z, then y, then x
    pub blocks: &'a [BlockId],
    /// Chunk light data
    pub light: &'a ChunkLightStore,
    /// Translation to encode in the mesh
    pub translation: Vec3,
    /// Sides of the surrounding chunks
    pub surrounding_sides_faces: &'a [Option<ChunkSideFaces>],
    /// Light data on the sides of the surrounding chunks
    pub surrounding_sides_light: &'a [Option<ChunkSideLight>],
}

/// Creates the vertices for a chunk mesh where faces inside the volume are skipped but no
/// faces are merged.
/// The mesh should be rendered an index buffer that repeats the pattern 0, 1, 2, 2, 3, 0.
/// Compared to `mesh_greedy`, meshing is much faster but the resulting meshes
/// are more complex and therefore slower to render
#[allow(unused)]
pub fn mesh_culled(input: ChunkMeshInput) -> Vec<TerrainVertex> {
    let mut vertices = Vec::new();

    add_visible_faces::<PosX>(&mut vertices, input);
    add_visible_faces::<PosY>(&mut vertices, input);
    add_visible_faces::<PosZ>(&mut vertices, input);
    add_visible_faces::<NegX>(&mut vertices, input);
    add_visible_faces::<NegY>(&mut vertices, input);
    add_visible_faces::<NegZ>(&mut vertices, input);

    vertices
}

/// Creates a chunk mesh where faces inside the volume are skipped and
/// compatible faces are merged greedily.
/// Compared to `culled`, meshing is much slower but the resulting meshes
/// are simpler and therefore faster to render
#[allow(unused)]
pub fn mesh_greedy(input: ChunkMeshInput) -> Vec<TerrainVertex> {
    let mut vertices = Vec::new();

    add_greedy_merged_faces::<PosX>(&mut vertices, input);
    add_greedy_merged_faces::<PosY>(&mut vertices, input);
    add_greedy_merged_faces::<PosZ>(&mut vertices, input);
    add_greedy_merged_faces::<NegX>(&mut vertices, input);
    add_greedy_merged_faces::<NegY>(&mut vertices, input);
    add_greedy_merged_faces::<NegZ>(&mut vertices, input);

    vertices
}

/// Decides whether the two faces can be merged
fn can_merge_faces<Dir>(first: Option<BlockFace>, second: Option<BlockFace>) -> bool
where
    Dir: FaceDir,
{
    let faces_match = first == second;

    faces_match
}

/// Add a single axis-aligned face to the mesh
/// `origin` is the position of the cell with the smallest coordinates that this face covers
fn add_face<Dir>(
    vertices: &mut Vec<TerrainVertex>,
    origin: Vec3,
    size: Vec2,
    texture_index: usize,
    light_data: FaceLightData,
) where
    Dir: FaceDir,
{
    let vertex_offsets = Dir::vertices(size);
    let uvs = [[0.0, size.y], [size.x, size.y], [size.x, 0.0], [0.0, 0.0]];

    // improve the anisotropy in how the lighting is interpolated along the quad when divided into
    // two triangles by flipping the orientation of the triangles based on the brightness of the
    // light at each vertex.
    // https://0fps.net/2013/07/03/ambient-occlusion-for-minecraft-like-worlds/ "Details regarding meshing"
    let flipped = should_flip_quad(&light_data);

    vertices.extend(
        (0..4)
            .map(|i| {
                // flip the quad by moving all vertices forwards by one
                if flipped {
                    (i + 1) & 3
                } else {
                    i
                }
            })
            .map(|i| TerrainVertex {
                position: (origin + vertex_offsets[i]).to_array(),
                uv: uvs[i],
                texture_index: texture_index as u32,
                light: (Dir::SHADING * light_data.0[Dir::LIGHT_INDICES[i]]).to_array(),
            }),
    );
}

/// Add all visible faces for the given face direction
fn add_visible_faces<Dir>(vertices: &mut Vec<TerrainVertex>, input: ChunkMeshInput)
where
    Dir: FaceDir,
{
    for pos_parallel_x in 0..CHUNK_SIZE_U32 {
        for pos_parallel_y in 0..CHUNK_SIZE_U32 {
            let index_in_layer = (CHUNK_SIZE_U32 * pos_parallel_y + pos_parallel_x) as usize;
            let mut visible = input.surrounding_sides_faces[Dir::FACE_INDEX.as_usize()]
                .as_ref()
                .map(|side| side.faces[index_in_layer])
                .unwrap_or(true);

            for pos_perpendicular in 0..CHUNK_SIZE_U32 {
                let pos_in_chunk = Dir::rotate_uvec3(UVec3::new(
                    pos_parallel_x,
                    pos_parallel_y,
                    // iterate backwards through the chunk
                    if Dir::NEGATIVE {
                        pos_perpendicular
                    } else {
                        (CHUNK_SIZE_U32 - 1) - pos_perpendicular
                    },
                ));

                let block_id = input.blocks[uvec3_to_chunk_index(pos_in_chunk)];
                let block_model = &BLOCKS[block_id.0 as usize].model;

                let face = block_model.face(Dir::FACE_INDEX);
                if let Some(face) = face {
                    if visible {
                        let light_data = interpolate_light_for_face::<Dir>(
                            input,
                            LocalBlockPosition::from(pos_in_chunk),
                        );

                        add_face::<Dir>(
                            vertices,
                            pos_in_chunk.as_vec3() + input.translation,
                            Vec2::ONE,
                            face.texture_index,
                            light_data,
                        );
                    }
                }

                visible = block_model.face(Dir::OPPOSITE_FACE_INDEX).is_none();
            }
        }
    }
}

/// Greedily merge visible faces with the given direction and add them to the mesh
fn add_greedy_merged_faces<Dir>(vertices: &mut Vec<TerrainVertex>, input: ChunkMeshInput)
where
    Dir: FaceDir,
{
    // references:
    // - https://eddieabbondanz.io/post/voxel/greedy-mesh/

    // note about coordinates:
    //   U and V refer to the cardinal directions perpendicular to the face direction
    //   U is the direction of the first texture coordinate
    //   V is the direction of the second texture coordinate

    // this will track whether each face in the next layer is visible
    // a face is visible if the block in the previous layer had no face in
    // the opposite direction
    let mut visible: [bool; CHUNK_SIZE_SQUARED] =
        if let Some(side) = &input.surrounding_sides_faces[Dir::FACE_INDEX.as_usize()] {
            *side.faces
        } else {
            [true; CHUNK_SIZE_SQUARED]
        };

    // iterate over each layer of faces we will create
    for layer_index in 0..CHUNK_SIZE_U32 {
        // position of this layer, moving backwards through the chunk with respect to the face
        // direction
        let layer_pos = if Dir::NEGATIVE {
            layer_index
        } else {
            (CHUNK_SIZE_U32 - 1) - layer_index
        };

        // this will track which faces have already been merged with another
        // already merged faces can safely be ignored
        let mut already_merged = [false; CHUNK_SIZE_SQUARED];

        // this is used to cache the interpolated light data for faces, to avoid computing it twice
        // for the same face
        let mut interpolated_light_cache = [None; CHUNK_SIZE_SQUARED];

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
                let original_pos = Dir::rotate_uvec3(UVec3::new(original_u, original_v, layer_pos));

                let original_id = input.blocks[uvec3_to_chunk_index(original_pos) as usize];
                let original_model = &BLOCKS[original_id.0 as usize].model;
                let original_face = original_model.face(Dir::FACE_INDEX);
                let original_visible = visible[original_index];

                let original_light_data = if let Some(cached_light_data) =
                    interpolated_light_cache[original_index]
                {
                    cached_light_data
                } else {
                    interpolate_light_for_face::<Dir>(input, LocalBlockPosition::from(original_pos))
                    // no need to insert it into the cache because this face will never be
                    // considered as a merge candidate
                };

                // update `visible` for the next layer
                visible[original_index] = original_model.face(Dir::OPPOSITE_FACE_INDEX).is_none();

                // skip if there is no face or the face is invisible
                if original_face.is_none() || !original_visible {
                    continue;
                }
                let original_face = original_face.unwrap();

                // march to see how many faces can be merged in the U direction
                let mut face_size = UVec2::ONE;
                for merge_candidate_u in (original_u + 1)..CHUNK_SIZE_U32 {
                    let (can_merge, next_visible) = consider_merge_candidate::<Dir>(
                        input,
                        &visible,
                        &mut interpolated_light_cache,
                        layer_pos,
                        original_face,
                        original_light_data,
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
                    let mut visibility_flags: u64 = 0;

                    // see if we can merge the next layer down by checking all blocks on this
                    // layer in the U direction
                    for merge_candidate_u in original_u..(original_u + face_size.x) {
                        let (can_merge, next_visible) = consider_merge_candidate::<Dir>(
                            input,
                            &visible,
                            &mut interpolated_light_cache,
                            layer_pos,
                            original_face,
                            original_light_data,
                            merge_candidate_u,
                            merge_candidate_v,
                        );

                        // stop counting when we can't merge any more faces
                        if !can_merge {
                            break 'v;
                        }

                        // update visibility flags
                        visibility_flags |= (next_visible as u64) << merge_candidate_u;
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
                add_face::<Dir>(
                    vertices,
                    original_pos.as_vec3() + input.translation,
                    face_size.as_vec2(),
                    original_face.texture_index,
                    original_light_data,
                );
            }
        }
    }
}

/// Evaluate whether the original face can be merged with the face with coordinates
/// `merge_candidate_u` and `merge_candidate_v` in the layer with position `layer_pos`
/// returns two booleans: whether the face can be merged, and whether the block with the
/// same U and V coordinates in the following layer is visible
fn consider_merge_candidate<Dir>(
    input: ChunkMeshInput,
    visible: &[bool; CHUNK_SIZE_SQUARED],
    interpolated_light_cache: &mut [Option<FaceLightData>; CHUNK_SIZE_SQUARED],
    layer_pos: u32,
    original_face: BlockFace,
    original_light_data: FaceLightData,
    merge_candidate_u: u32,
    merge_candidate_v: u32,
) -> (bool, bool)
where
    Dir: FaceDir,
{
    let merge_candidate_pos = UVec3::new(merge_candidate_u, merge_candidate_v, layer_pos);
    let merge_candidate_pos = Dir::rotate_uvec3(merge_candidate_pos);

    let merge_candidate_index = (CHUNK_SIZE_U32 * merge_candidate_v + merge_candidate_u) as usize;

    let merge_candidate_id = input.blocks[uvec3_to_chunk_index(merge_candidate_pos) as usize];
    let merge_candidate_model = &BLOCKS[merge_candidate_id.0 as usize].model;
    let merge_candidate_face = merge_candidate_model.face(Dir::FACE_INDEX);
    let merge_candidate_visible = visible[merge_candidate_index];

    let next_visible = merge_candidate_model
        .face(Dir::OPPOSITE_FACE_INDEX)
        .is_none();

    let can_merge = can_merge_faces::<Dir>(Some(original_face), merge_candidate_face)
        && merge_candidate_visible;

    if !can_merge {
        return (false, next_visible);
    }

    let merge_candidate_light_data = if let Some(cached_light_data) =
        interpolated_light_cache[merge_candidate_index]
    {
        cached_light_data
    } else {
        let interpolated =
            interpolate_light_for_face::<Dir>(input, LocalBlockPosition::from(merge_candidate_pos));

        interpolated_light_cache[merge_candidate_index] = Some(interpolated);
        interpolated
    };
    let light_matches = merge_candidate_light_data == original_light_data;

    (light_matches, next_visible)
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct FaceLightData([Vec4; 4]);

/// Interpolate the light values for each vertex of the given face
fn interpolate_light_for_face<Dir>(
    input: ChunkMeshInput,
    block_pos: LocalBlockPosition,
) -> FaceLightData
where
    Dir: FaceDir,
{
    // will track whether each orthogonal block is opaque
    // used to prevent light leaking from corner blocks cases like
    // L O
    // O C
    // (L: light, O: opaque, C: center)
    let mut orthogonal = [false; 4];

    // read the 9x9 neighbourhood of blocks in front of the face
    #[rustfmt::skip]
    let mut samples = [
        [
            // -1 -1
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL - Dir::TANGENT - Dir::BITANGENT,
                None,
            ),
            // -1 0
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL - Dir::TANGENT,
                Some(&mut orthogonal[0]),
            ),
            // -1 1
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL - Dir::TANGENT + Dir::BITANGENT,
                None,
            ),
        ],
        [
            // 0 -1
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL - Dir::BITANGENT,
                Some(&mut orthogonal[1]),
            ),
            // 0 0
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL,
                None,
            ),
            // 0 1
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL + Dir::BITANGENT,
                Some(&mut orthogonal[2]),
            ),
        ],
        [
            // 1 -1
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL + Dir::TANGENT - Dir::BITANGENT,
                None,
            ),
            // 1 0
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL + Dir::TANGENT,
                Some(&mut orthogonal[3]),
            ),
            // 1 -1
            sample_light_at(
                input,
                block_pos,
                Dir::NORMAL + Dir::TANGENT + Dir::BITANGENT,
                None,
            ),
        ],
    ];

    // prevent light leaking from corner blocks
    if orthogonal[0] && orthogonal[1] {
        samples[0][0] = Vec4::ZERO;
    }
    if orthogonal[0] && orthogonal[2] {
        samples[0][2] = Vec4::ZERO;
    }
    if orthogonal[1] && orthogonal[3] {
        samples[2][0] = Vec4::ZERO;
    }
    if orthogonal[2] && orthogonal[3] {
        samples[2][2] = Vec4::ZERO;
    }

    FaceLightData([
        0.25 * (samples[0][0] + samples[0][1] + samples[1][0] + samples[1][1]),
        0.25 * (samples[0][1] + samples[0][2] + samples[1][1] + samples[1][2]),
        0.25 * (samples[1][0] + samples[1][1] + samples[2][0] + samples[2][1]),
        0.25 * (samples[1][1] + samples[1][2] + samples[2][1] + samples[2][2]),
    ])
}

/// Sample the light value at the block offset by `block_offset` from `block_pos`
/// If `opaque` is `Some(p)`, `*p` will be set to whether there is an opaque block at the position
fn sample_light_at(
    input: ChunkMeshInput,
    block_pos: LocalBlockPosition,
    block_offset: IVec3,
    opaque: Option<&mut bool>,
) -> Vec4 {
    let emitted_light = if let Some(block_pos) = block_pos.try_add(block_offset) {
        opaque.map(|p| {
            let block_id = input.blocks[block_pos.get_array_index()];
            let block = &BLOCKS[block_id.as_usize()];

            *p = block.model.is_opaque();
        });

        input.light.read(block_pos)
    } else {
        let offset_pos = block_pos.as_ivec3() + block_offset;
        let wrapped_pos = offset_pos & (CHUNK_SIZE_I32 - 1);
        let chunk_offset = offset_pos.div_euclid(IVec3::splat(CHUNK_SIZE_I32));

        if let Some(side_index) = FaceIndex::from_dir(chunk_offset) {
            macro_rules! get_aligned_coordinate {
                ($directions:expr) => {
                    IVec3::dot(wrapped_pos, $directions[side_index.as_usize()].abs())
                        & (CHUNK_SIZE_I32 - 1)
                };
            }

            let u = get_aligned_coordinate!(FACE_TANGENTS);
            let v = get_aligned_coordinate!(FACE_BITANGENTS);

            let index_in_side = (u as usize) + (v as usize) * CHUNK_SIZE;

            opaque.map(|p| {
                *p = input
                    .surrounding_sides_faces
                    .get(side_index.as_usize())
                    .is_some_and(|side_opt| {
                        side_opt
                            .as_ref()
                            .is_some_and(|side| side.faces[index_in_side])
                    })
            });

            input.surrounding_sides_light[side_index.as_usize()]
                .as_ref()
                .map(|side| side.emitted[index_in_side])
                .unwrap_or(EmittedLight::ZERO)
        } else {
            // No information for chunk corner so use center block
            input.light.read(block_pos)
        }
    };

    let emitted_light_rgb = emitted_light.as_rgb();

    Vec4::new(
        emitted_light_rgb.0 as f32 / EmittedLight::MAX_VALUE as f32,
        emitted_light_rgb.1 as f32 / EmittedLight::MAX_VALUE as f32,
        emitted_light_rgb.2 as f32 / EmittedLight::MAX_VALUE as f32,
        0.0,
    )
}

/// Decide whether to generate a flipped quad based on the light data, in order to improve the
/// anisotropy artifact caused by the division of the quad into two triangles
fn should_flip_quad(light_data: &FaceLightData) -> bool {
    fn sum_vec4(v: Vec4) -> f32 {
        v.x + v.y + v.z + v.z
    }

    sum_vec4(light_data.0[0] + light_data.0[3]) <= sum_vec4(light_data.0[1] + light_data.0[2])
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
    ((CHUNK_SIZE_U32 * CHUNK_SIZE_U32) * pos.y + CHUNK_SIZE_U32 * pos.z + pos.x) as usize
}

mod face_dir {
    use glam::{IVec3, UVec3, Vec2, Vec3, Vec3Swizzles};

    use crate::util::face::FaceIndex;

    /// Information about each face direction needed for meshing
    pub trait FaceDir {
        /// Direction parallel to the first texture coordinate
        const TANGENT: IVec3;

        /// Direction parallel to the second texture coordinate
        const BITANGENT: IVec3;

        /// Direction pointing away from the face
        const NORMAL: IVec3;

        /// `FaceIndex` matching this face direction
        const FACE_INDEX: FaceIndex;

        /// `FaceIndex` matching the opposite face direction
        const OPPOSITE_FACE_INDEX: FaceIndex;

        /// Whether this face direction points away from its axis
        const NEGATIVE: bool;

        /// Hardcoded directional shading for this face
        const SHADING: f32;

        /// Which of the interpolated light values corresponds to each vertex
        const LIGHT_INDICES: [usize; 4];

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
        const TANGENT: IVec3 = IVec3::Z;
        const BITANGENT: IVec3 = IVec3::Y;
        const NORMAL: IVec3 = IVec3::X;
        const FACE_INDEX: FaceIndex = FaceIndex::POS_X;
        const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::NEG_X;
        const NEGATIVE: bool = false;
        const SHADING: f32 = 0.7;
        const LIGHT_INDICES: [usize; 4] = [2, 0, 1, 3];

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
        const TANGENT: IVec3 = IVec3::Z;
        const BITANGENT: IVec3 = IVec3::X;
        const NORMAL: IVec3 = IVec3::Y;
        const FACE_INDEX: FaceIndex = FaceIndex::POS_Y;
        const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::NEG_Y;
        const NEGATIVE: bool = false;
        const SHADING: f32 = 1.0;
        const LIGHT_INDICES: [usize; 4] = [0, 2, 3, 1];

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
        const NORMAL: IVec3 = IVec3::Z;
        const TANGENT: IVec3 = IVec3::X;
        const BITANGENT: IVec3 = IVec3::Y;
        const FACE_INDEX: FaceIndex = FaceIndex::POS_Z;
        const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::NEG_Z;
        const NEGATIVE: bool = false;
        const SHADING: f32 = 0.8;
        const LIGHT_INDICES: [usize; 4] = [0, 2, 3, 1];

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
        const NORMAL: IVec3 = IVec3::NEG_X;
        const TANGENT: IVec3 = IVec3::NEG_Z;
        const BITANGENT: IVec3 = IVec3::NEG_Y;
        const FACE_INDEX: FaceIndex = FaceIndex::NEG_X;
        const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::POS_X;
        const NEGATIVE: bool = true;
        const SHADING: f32 = 0.7;
        const LIGHT_INDICES: [usize; 4] = [3, 1, 0, 2];

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
        const NORMAL: IVec3 = IVec3::NEG_Y;
        const TANGENT: IVec3 = IVec3::NEG_Z;
        const BITANGENT: IVec3 = IVec3::NEG_X;
        const FACE_INDEX: FaceIndex = FaceIndex::NEG_Y;
        const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::POS_Y;
        const NEGATIVE: bool = true;
        const SHADING: f32 = 0.5;
        const LIGHT_INDICES: [usize; 4] = [2, 0, 1, 3];

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
        const NORMAL: IVec3 = IVec3::NEG_Z;
        const TANGENT: IVec3 = IVec3::NEG_X;
        const BITANGENT: IVec3 = IVec3::NEG_Y;
        const FACE_INDEX: FaceIndex = FaceIndex::NEG_Z;
        const OPPOSITE_FACE_INDEX: FaceIndex = FaceIndex::POS_Z;
        const NEGATIVE: bool = true;
        const SHADING: f32 = 0.6;
        const LIGHT_INDICES: [usize; 4] = [1, 3, 2, 0];

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
