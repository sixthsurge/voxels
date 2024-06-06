use glam::{UVec2, UVec3, Vec2, Vec2Swizzles, Vec3, Vec3Swizzles};

use crate::{
    block::{BlockId, BlockModel, BLOCKS},
    chunk::{uvec3_to_chunk_index, CHUNK_SIZE, CHUNK_SIZE_CUBED, CHUNK_SIZE_SQUARED},
    render::util::mesh::Vertex,
};

pub struct ChunkMeshData {
    pub vertices: Vec<ChunkVertex>,
    pub indices: Vec<ChunkIndex>,
}

impl ChunkMeshData {
    /// creates an empty chunk mesh
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// creates a chunk mesh where faces inside the volume are skipped but no
    /// faces are merged
    /// compared to `greedy`, meshing is much faster but the resulting meshes
    /// are more complex and therefore slower to render
    pub fn culled(blocks: &[BlockId; CHUNK_SIZE_CUBED]) -> Self {
        let mut result = Self::empty();

        result.add_visible_faces::<PosX>(blocks);
        result.add_visible_faces::<PosY>(blocks);
        result.add_visible_faces::<PosZ>(blocks);
        result.add_visible_faces::<NegX>(blocks);
        result.add_visible_faces::<NegY>(blocks);
        result.add_visible_faces::<NegZ>(blocks);

        result
    }

    /// creates a chunk mesh where faces inside the volume are skipped and
    /// compatible faces are merged greedily
    /// compared to `culled`, meshing is much slower but the resulting meshes
    /// are simpler and therefore faster to render
    pub fn greedy(blocks: &[BlockId; CHUNK_SIZE_CUBED]) -> Self {
        let mut result = Self::empty();

        result.add_greedy_merged_faces::<PosX>(blocks);
        result.add_greedy_merged_faces::<PosY>(blocks);
        result.add_greedy_merged_faces::<PosZ>(blocks);
        result.add_greedy_merged_faces::<NegX>(blocks);
        result.add_greedy_merged_faces::<NegY>(blocks);
        result.add_greedy_merged_faces::<NegZ>(blocks);

        result
    }

    /// add all visible faces with the given direction
    fn add_visible_faces<Dir>(&mut self, blocks: &[BlockId; CHUNK_SIZE_CUBED])
    where
        Dir: FaceDir,
    {
        for pos_parallel_x in 0..CHUNK_SIZE {
            for pos_parallel_y in 0..CHUNK_SIZE {
                let mut visible = true;

                for pos_perpendicular in 0..CHUNK_SIZE {
                    let pos_in_chunk = Dir::rotate_uvec3(UVec3::new(
                        pos_parallel_x,
                        pos_parallel_y,
                        // iterate backwards through the chunk
                        if Dir::NEGATIVE {
                            pos_perpendicular
                        } else {
                            (CHUNK_SIZE - 1) - pos_perpendicular
                        },
                    ));

                    let block_id = blocks[uvec3_to_chunk_index(pos_in_chunk)];
                    let block_model = &BLOCKS[block_id.0 as usize].model;

                    if block_model.has_face(Dir::FACE_INDEX) {
                        if visible {
                            self.add_face::<Dir>(
                                pos_in_chunk.as_vec3(),
                                Vec2::ONE,
                                block_model.texture_index(),
                            );
                        }
                    }

                    visible = !block_model.has_face(Dir::OPPOSITE_FACE_INDEX);
                }
            }
        }
    }

    /// greedily merge faces with the given direction andadd them to the mesh
    fn add_greedy_merged_faces<Dir>(&mut self, blocks: &[BlockId; CHUNK_SIZE_CUBED])
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
        let mut visible = [true; CHUNK_SIZE_SQUARED];

        // iterate over each layer of faces we will create
        for layer_index in 0..CHUNK_SIZE {
            // position of this layer, moving backwards through the chunk with respect to the face
            // direction
            let layer_pos = if Dir::NEGATIVE {
                layer_index
            } else {
                (CHUNK_SIZE - 1) - layer_index
            };

            // this will track which faces have already been merged with another
            // already merged faces can safely be ignored
            let mut already_merged = [false; CHUNK_SIZE_SQUARED];

            // iterate over each block in the layer
            for original_v in 0..CHUNK_SIZE {
                for original_u in 0..CHUNK_SIZE {
                    // index of this block in the current layer
                    let original_index = (original_v * CHUNK_SIZE + original_u) as usize;

                    // skip if already merged
                    if already_merged[original_index] {
                        continue;
                    }

                    // position of this block in the chunk
                    let original_pos =
                        Dir::rotate_uvec3(UVec3::new(original_u, original_v, layer_pos));

                    let original_id = blocks[uvec3_to_chunk_index(original_pos) as usize];
                    let original_model = &BLOCKS[original_id.0 as usize].model;
                    let original_visible = visible[original_index];

                    // update `visible` for the next layer
                    visible[original_index] = !original_model.has_face(Dir::OPPOSITE_FACE_INDEX);

                    // skip if there is no face or the face is invisible
                    if !original_model.has_face(Dir::FACE_INDEX) || !original_visible {
                        continue;
                    }

                    // march to see how many faces can be merged in the U direction
                    let mut face_size = UVec2::ONE;
                    for merge_candidate_u in (original_u + 1)..CHUNK_SIZE {
                        let (can_merge, next_visible) = Self::consider_merge_candidate::<Dir>(
                            blocks,
                            &visible,
                            layer_pos,
                            original_model,
                            merge_candidate_u,
                            original_v,
                        );

                        // stop counting when we can't merge any more faces
                        if !can_merge {
                            break;
                        }

                        let merged_index_in_layer =
                            (CHUNK_SIZE * original_v + merge_candidate_u) as usize;

                        // grow the face
                        face_size.x += 1;

                        // mark that this face is already merged
                        already_merged[merged_index_in_layer] = true;

                        // update `visible` for the same block in the next layer
                        // (this would not otherwise occur)
                        visible[merged_index_in_layer] = next_visible;
                    }

                    // march to see how many faces can be merged in the V direction
                    'v: for merge_candidate_v in (original_v + 1)..CHUNK_SIZE {
                        // bit flags for whether the block adjacent to a block being considered for
                        // merging will be visible
                        // this avoids having to check the model again once it has been decided
                        // the layers can be merged
                        let mut visibility_flags: u32 = 0;

                        // see if we can merge the next layer down by checking all blocks on this
                        // layer in the U direction
                        for merge_candidate_u in original_u..(original_u + face_size.x) {
                            let (can_merge, next_visible) = Self::consider_merge_candidate::<Dir>(
                                blocks,
                                &visible,
                                layer_pos,
                                original_model,
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
                                (merge_candidate_v * CHUNK_SIZE + merged_x) as usize;

                            already_merged[merged_index_in_layer] = true;

                            // update `visible` for the same block in the next layer
                            // visibility flags already computed
                            // (this would not otherwise occur)
                            visible[merged_index_in_layer] =
                                (visibility_flags & (1 << merged_x)) != 0;
                        }
                    }

                    // create the merged face
                    self.add_face::<Dir>(
                        original_pos.as_vec3(),
                        face_size.as_vec2(),
                        original_model.texture_index(),
                    );
                }
            }
        }
    }

    /// add a single axis-aligned face to the mesh
    /// `first_block_pos` is the position of the cell with the smallest coordinates that this face
    /// covers
    fn add_face<Dir>(&mut self, origin: Vec3, size: Vec2, texture_index: usize)
    where
        Dir: FaceDir,
    {
        const INDICES: [ChunkIndex; 6] = [0, 1, 2, 2, 3, 0];

        let uvs = [[0.0, size.y], [size.x, size.y], [size.x, 0.0], [0.0, 0.0]];

        let first_index = self.vertices.len() as ChunkIndex;

        self.vertices.extend(
            Dir::vertices(size)
                .iter()
                .enumerate()
                .map(|(i, vertex_offset)| ChunkVertex {
                    position: (origin + *vertex_offset).to_array(),
                    uv: uvs[i],
                    texture_index: texture_index as u32,
                }),
        );
        self.indices.extend(
            INDICES
                .iter()
                .map(|index| index + first_index),
        );
    }

    /// returns true if the two faces can be merged
    fn can_merge_faces<Dir>(block_model_a: &BlockModel, block_model_b: &BlockModel) -> bool
    where
        Dir: FaceDir,
    {
        block_model_a.has_face(Dir::FACE_INDEX) == block_model_b.has_face(Dir::FACE_INDEX)
            && block_model_a.texture_index() == block_model_b.texture_index()
    }

    /// evaluate whether the original face can be merged with the face with coordinates
    /// `merge_candidate_u` and `merge_candidate_v` in the layer with position `layer_pos`
    /// returns two booleans: whether the face can be merged, and whether the block with the
    /// same U and V coordinates in the following layer is visible
    fn consider_merge_candidate<Dir>(
        blocks: &[BlockId; CHUNK_SIZE_CUBED],
        visible: &[bool; CHUNK_SIZE_SQUARED],
        layer_pos: u32,
        original_model: &BlockModel,
        merge_candidate_u: u32,
        merge_candidate_v: u32,
    ) -> (bool, bool)
    where
        Dir: FaceDir,
    {
        let merge_candidate_pos = UVec3::new(merge_candidate_u, merge_candidate_v, layer_pos);
        let merge_candidate_pos = Dir::rotate_uvec3(merge_candidate_pos);

        let merge_candidate_index_in_layer =
            (CHUNK_SIZE * merge_candidate_v + merge_candidate_u) as usize;

        let merge_candidate_id = blocks[uvec3_to_chunk_index(merge_candidate_pos) as usize];
        let merge_candidate_model = &BLOCKS[merge_candidate_id.0 as usize].model;
        let merge_candidate_visible = visible[merge_candidate_index_in_layer];

        let can_merge = Self::can_merge_faces::<Dir>(original_model, merge_candidate_model)
            && merge_candidate_visible;
        let next_visible = !merge_candidate_model.has_face(Dir::OPPOSITE_FACE_INDEX);

        (can_merge, next_visible)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ChunkVertex {
    pub position: [f32; 3],
    pub uv: [f32; 2],
    pub texture_index: u32,
}

impl Vertex for ChunkVertex {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 3] =
            wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Uint32];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

type ChunkIndex = u32;

/// face directions
pub trait FaceDir {
    /// index assigned to this face direction
    const FACE_INDEX: usize;

    /// index assigned to the opposite face direction
    const OPPOSITE_FACE_INDEX: usize;

    /// whether this face direction points away from its axis
    const NEGATIVE: bool;

    /// returns the 4 vertices for a face of this direction
    /// the size of the face on the two parallel directions is
    /// when looking at the face head on, the first vertex is at
    /// the bottom left and the following vertices proceed anticlockwise
    fn vertices(size: Vec2) -> [Vec3; 4];

    /// given a vector whose x and y components are specified parallel to the face and whose z
    /// component is specified perpendicular to the face, converts it to absolute coordinates by
    /// swizzling
    /// rotate_vec3(Vec3::new(0.0, 0.0, 1.0)) gives the axis of the face
    /// rotate_vec3(Vec3::new(1.0, 0.0, 0.0)) gives a tangent of the face
    /// rotate_vec3(Vec3::new(0.0, 1.0, 0.0)) gives another tangent of the face
    fn rotate_vec3(v: Vec3) -> Vec3;

    /// given a vector whose x and y components are specified parallel to the face and whose z
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
    const FACE_INDEX: usize = 0;
    const OPPOSITE_FACE_INDEX: usize = 3;
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
    const FACE_INDEX: usize = 1;
    const OPPOSITE_FACE_INDEX: usize = 4;
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
    const FACE_INDEX: usize = 2;
    const OPPOSITE_FACE_INDEX: usize = 5;
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
    const FACE_INDEX: usize = 3;
    const OPPOSITE_FACE_INDEX: usize = 0;
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
    const FACE_INDEX: usize = 4;
    const OPPOSITE_FACE_INDEX: usize = 1;
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
    const FACE_INDEX: usize = 5;
    const OPPOSITE_FACE_INDEX: usize = 2;
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
