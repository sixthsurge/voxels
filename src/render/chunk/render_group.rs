use glam::{IVec3, UVec3};
use wgpu::util::DeviceExt;

use crate::{
    render::{context::RenderContext, util::mesh::Mesh},
    terrain::chunk::CHUNK_SIZE,
};

use super::{
    meshing,
    vertex::{self, ChunkVertex},
};

/// size of one chunk render group on each axis
pub const CHUNK_RENDER_GROUP_SIZE: usize = 2;

/// size of one chunk render group on each axis, squared
pub const CHUNK_RENDER_GROUP_SIZE_SQUARED: usize =
    CHUNK_RENDER_GROUP_SIZE * CHUNK_RENDER_GROUP_SIZE;

/// number of chunks in one render group
pub const CHUNK_RENDER_GROUP_SIZE_CUBED: usize =
    CHUNK_RENDER_GROUP_SIZE * CHUNK_RENDER_GROUP_SIZE * CHUNK_RENDER_GROUP_SIZE;

/// length of one chunk render group in the world
pub const CHUNK_RENDER_GROUP_LENGTH: usize = CHUNK_SIZE * CHUNK_RENDER_GROUP_SIZE;

/// to reduce draw calls, neighbouring chunks are grouped into "render groups", where the mesh of
/// the render group is the concatenation of the meshes of the chunks it contains
/// this is the struct that holds the terrain meshes that are actually sent to the GPU
/// the disadvantage of this approach is that chunk vertices need to be kept in memory in order
/// to update the render group (normally they could just be discarded once sent to the GPU)
#[derive(Debug)]
pub struct ChunkRenderGroup {
    /// position of this render group in the grid of render groups
    pos: IVec3,
    /// combined mesh for all chunks in this render group
    mesh: Option<Mesh>,
    /// uniform buffer for this render group
    uniform_buffer: wgpu::Buffer,
    /// bind group for the render group uniforms
    uniforms_bind_group: wgpu::BindGroup,
    /// vertices of each chunk in the render group
    chunk_vertices: [Vec<ChunkVertex>; CHUNK_RENDER_GROUP_SIZE_CUBED],
}

impl ChunkRenderGroup {
    pub fn new(
        position: IVec3,
        device: &wgpu::Device,
        uniforms_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let render_group_offset = position.as_vec3() * (CHUNK_RENDER_GROUP_LENGTH as f32);
        let render_group_offset = render_group_offset.to_array();

        let uniforms = RenderGroupUniforms {
            render_group_offset,
            pad: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Render Group Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniforms_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Group Uniforms Bind Group"),
            layout: uniforms_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let chunk_vertices = array_init::array_init(|_| Vec::new());

        Self {
            pos: position,
            mesh: None,
            uniform_buffer,
            uniforms_bind_group,
            chunk_vertices,
        }
    }

    /// update the stored vertices for the given chunk
    /// this function will not update the mesh
    pub fn set_vertices_for_chunk(
        &mut self,
        chunk_pos_in_group: UVec3,
        vertices: Vec<ChunkVertex>,
    ) {
        debug_assert!(chunk_pos_in_group.max_element() < CHUNK_RENDER_GROUP_SIZE as u32);

        let index = Self::get_index_for_chunk(chunk_pos_in_group);

        self.chunk_vertices[index] = vertices;
    }

    /// update the mesh for this render group
    pub fn update_mesh(&mut self, device: &wgpu::Device) {
        // calculate the total number of vertices for the combined mesh
        let vertex_count = self
            .chunk_vertices
            .iter()
            .map(|vertices| vertices.len())
            .sum();

        // add each chunk's vertices to the list of vertices
        let vertices = {
            let mut v = Vec::with_capacity(vertex_count);
            self.chunk_vertices
                .iter()
                .for_each(|vertices| v.extend_from_slice(vertices));
            v
        };

        // generate the whole index buffer (the indices follow a repeating pattern so this
        // can be generated all at one)
        let indices = meshing::generate_indices(vertex_count);

        // create the new mesh
        self.mesh = Some(Mesh::new(device, &vertices, &indices));
    }

    /// returns the mesh for this render group, if it has one
    pub fn mesh(&self) -> Option<&Mesh> {
        self.mesh.as_ref()
    }

    /// returns the bind group for this render group's uniforms
    pub fn uniforms_bind_group(&self) -> &wgpu::BindGroup {
        &self.uniforms_bind_group
    }

    /// returns the position of this render group in the grid of chunk render groups
    pub fn pos(&self) -> IVec3 {
        self.pos
    }

    /// returns the index in `self.chunk_vertices` for the chunk with the given position in the
    /// group
    fn get_index_for_chunk(pos: UVec3) -> usize {
        CHUNK_RENDER_GROUP_SIZE_SQUARED * pos.z as usize
            + CHUNK_RENDER_GROUP_SIZE * pos.y as usize
            + pos.x as usize
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RenderGroupUniforms {
    render_group_offset: [f32; 3],
    pad: f32,
}
