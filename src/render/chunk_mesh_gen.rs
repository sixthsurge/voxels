use glam::Vec3;

use crate::{
    block::{BlockId, BLOCK_WHITE},
    chunk::{CHUNK_SIZE_FLAT, CHUNK_SIZE_X, CHUNK_SIZE_Y, CHUNK_SIZE_Z},
    render::mesh::Vertex,
};

pub struct ChunkMesh {
    pub vertices: Vec<ChunkVertex>,
    pub indices: Vec<ChunkIndex>,
}

impl ChunkMesh {
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    pub fn build(blocks: [BlockId; CHUNK_SIZE_FLAT]) -> Self {
        let mut result = Self::empty();

        let mut block_index: usize = 0;
        for z in 0..CHUNK_SIZE_Z {
            for y in 0..CHUNK_SIZE_Y {
                for x in 0..CHUNK_SIZE_X {
                    let block_id = blocks[block_index];

                    if block_id == BLOCK_WHITE {}

                    result.add_face::<PosX>(x, y, z);
                    result.add_face::<PosY>(x, y, z);
                    result.add_face::<PosZ>(x, y, z);
                    result.add_face::<NegX>(x, y, z);
                    result.add_face::<NegY>(x, y, z);
                    result.add_face::<NegZ>(x, y, z);

                    block_index += 1;
                }
            }
        }

        result
    }

    pub fn add_face<Dir>(&mut self, x: u32, y: u32, z: u32)
    where
        Dir: FaceDir,
    {
        let first_index = self.vertices.len() as ChunkIndex;
        let block_pos = Vec3::new(x as f32, y as f32, z as f32);

        self.vertices.extend(
            Dir::VERTICES
                .iter()
                .copied()
                .map(|vertex_pos| ChunkVertex {
                    position: (block_pos + vertex_pos).to_array(),
                }),
        );
        self.indices.extend(
            Dir::INDICES
                .iter()
                .copied()
                .map(|index| index + first_index),
        );
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ChunkVertex {
    pub position: [f32; 3],
}

impl Vertex for ChunkVertex {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static> {
        const ATTRIBUTES: [wgpu::VertexAttribute; 1] = wgpu::vertex_attr_array![0 => Float32x3];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &ATTRIBUTES,
        }
    }
}

type ChunkIndex = u32;

pub trait FaceDir {
    /// Index assigned to this face direction
    const INDEX: usize;

    /// Vertices of this face on the unit cube
    const VERTICES: [Vec3; 4];

    /// Indices of the vertices in VERTICES of the two triangles making up this face of the unit cube
    const INDICES: [ChunkIndex; 6];

    /// Indices of the vertices in VERTICES of the two triangles making up this face of the unit cube
    /// (flipped orientation)
    const INDICES_FLIPPED: [ChunkIndex; 6];
}

pub struct PosX;
pub struct PosY;
pub struct PosZ;
pub struct NegX;
pub struct NegY;
pub struct NegZ;

impl FaceDir for PosX {
    const INDEX: usize = 0;
    const VERTICES: [Vec3; 4] = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];
    const INDICES: [ChunkIndex; 6] = [1, 3, 2, 2, 0, 1];
    const INDICES_FLIPPED: [ChunkIndex; 6] = [0, 1, 3, 3, 2, 0];
}
impl FaceDir for PosY {
    const INDEX: usize = 1;
    const VERTICES: [Vec3; 4] = [
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
        Vec3::new(0.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];
    const INDICES: [ChunkIndex; 6] = [1, 0, 2, 2, 3, 1];
    const INDICES_FLIPPED: [ChunkIndex; 6] = [0, 2, 3, 3, 1, 0];
}
impl FaceDir for PosZ {
    const INDEX: usize = 0;
    const VERTICES: [Vec3; 4] = [
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 1.0),
        Vec3::new(0.0, 1.0, 1.0),
        Vec3::new(1.0, 1.0, 1.0),
    ];
    const INDICES: [ChunkIndex; 6] = [0, 1, 3, 3, 2, 0];
    const INDICES_FLIPPED: [ChunkIndex; 6] = [1, 3, 2, 2, 0, 1];
}
impl FaceDir for NegX {
    const INDEX: usize = 0;
    const VERTICES: [Vec3; 4] = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(0.0, 1.0, 1.0),
    ];
    const INDICES: [ChunkIndex; 6] = [0, 2, 3, 3, 1, 0];
    const INDICES_FLIPPED: [ChunkIndex; 6] = [1, 0, 2, 2, 3, 1];
}
impl FaceDir for NegY {
    const INDEX: usize = 1;
    const VERTICES: [Vec3; 4] = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(1.0, 0.0, 1.0),
    ];
    const INDICES: [ChunkIndex; 6] = [3, 2, 1, 1, 3, 2];
    const INDICES_FLIPPED: [ChunkIndex; 6] = [0, 1, 3, 3, 2, 0];
}
impl FaceDir for NegZ {
    const INDEX: usize = 0;
    const VERTICES: [Vec3; 4] = [
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(1.0, 1.0, 0.0),
    ];
    const INDICES: [ChunkIndex; 6] = [1, 0, 2, 2, 3, 1];
    const INDICES_FLIPPED: [ChunkIndex; 6] = [0, 2, 3, 3, 1, 0];
}
