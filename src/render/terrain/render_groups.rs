use glam::{IVec3, UVec3};
use wgpu::util::DeviceExt;

use super::{meshing, ChunkMeshData, ChunkMeshStatus};
use crate::{render::util::mesh::Mesh, terrain::chunk::CHUNK_SIZE};

/// Size of one chunk render group on each axis, in chunks
/// Larger render groups mean fewer draw calls, but more time spent recombining meshes and
/// less granularity for culling
pub const RENDER_GROUP_SIZE: usize = 2;

/// Size of one chunk render group on each axis, squared
pub const RENDER_GROUP_SIZE_SQUARED: usize = RENDER_GROUP_SIZE * RENDER_GROUP_SIZE;

/// Size of one chunk render group on each axis, cubed.
/// The number of chunks in one render group
pub const RENDER_GROUP_SIZE_CUBED: usize =
    RENDER_GROUP_SIZE * RENDER_GROUP_SIZE * RENDER_GROUP_SIZE;

/// The length of one chunk render group in the world
pub const RENDER_GROUP_TOTAL_SIZE: usize = CHUNK_SIZE * RENDER_GROUP_SIZE;

/// To reduce draw calls, neighbouring chunks are grouped into "render groups", where the mesh of
/// the render group is the concatenation of the meshes of the chunks it contains.
/// This is the struct that holds the terrain meshes that are actually sent to the GPU.
/// The disadvantage of this approach is that chunk vertices need to be kept in memory in order
/// to update the render group (normally they could just be discarded once sent to the GPU).
#[derive(Debug)]
pub struct ChunkRenderGroup {
    /// Position of this render group in the grid of render groups
    pos: IVec3,
    /// Combined mesh for all chunks in this render group
    mesh: Option<Mesh>,
    /// Mesh data for each chunk in the group
    chunk_mesh_data: [Option<ChunkMeshData>; RENDER_GROUP_SIZE_CUBED],
    /// Mesh status for each chunk in the group
    chunk_mesh_status: [ChunkMeshStatus; RENDER_GROUP_SIZE_CUBED],
    /// Bind group for render-group-specific uniforms
    bind_group: wgpu::BindGroup,
}

impl ChunkRenderGroup {
    pub fn new(
        pos: IVec3,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let render_group_offset = pos.as_vec3() * (RENDER_GROUP_TOTAL_SIZE as f32);
        let render_group_offset = render_group_offset.to_array();

        let uniforms = RenderGroupUniforms {
            translation: render_group_offset,
            pad: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Render Group Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render Group Uniforms Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let chunk_mesh_data = array_init::array_init(|_| None);
        let chunk_mesh_status = array_init::array_init(|_| ChunkMeshStatus::NoneOrOutdated);

        Self {
            pos,
            mesh: None,
            chunk_mesh_data,
            chunk_mesh_status,
            bind_group,
        }
    }

    /// Update the stored vertices for the given chunk.
    /// If `queued_instant` is earlier than the stored instant for this
    /// chunk, the new mesh will be discarded as it is out of date
    /// This function will not update the group's combined mesh
    /// Returns true if the vertices were updated
    pub fn set_mesh_data_for_chunk(
        &mut self,
        chunk_pos_in_group: UVec3,
        mesh_data: ChunkMeshData,
    ) -> bool {
        debug_assert!(chunk_pos_in_group.max_element() < RENDER_GROUP_SIZE as u32);

        let index = Self::get_index_for_chunk(chunk_pos_in_group);

        if let Some(existing_mesh_data) = &self.chunk_mesh_data[index] {
            if mesh_data.queued_instant < existing_mesh_data.queued_instant {
                return false;
            }
        }

        self.chunk_mesh_data[index] = Some(mesh_data);
        self.chunk_mesh_status[index] = ChunkMeshStatus::Good;
        return true;
    }

    /// Clear the stored vertices for the given chunk position
    pub fn clear_mesh_data_for_chunk(&mut self, chunk_pos_in_group: UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        self.chunk_mesh_data[index] = None;
    }

    /// Mark that the mesh data for the given chunk position is currently being generated
    pub fn mark_generating(&mut self, chunk_pos_in_group: UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        self.chunk_mesh_status[index] = ChunkMeshStatus::Generating;
    }

    /// Mark that the mesh data for the given chunk position is outdated, if it exists
    pub fn mark_outdated(&mut self, chunk_pos_in_group: UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        if !self.chunk_mesh_status[index].is_none_or_outdated() {
            self.chunk_mesh_status[index] = ChunkMeshStatus::NoneOrOutdated;
        }
    }

    /// Mark that the mesh data for the given chunk position can be optimized, if it exists
    pub fn mark_suboptimal(&mut self, chunk_pos_in_group: UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        if !self.chunk_mesh_status[index].is_none_or_outdated() {
            self.chunk_mesh_status[index] = ChunkMeshStatus::Suboptimal;
        }
    }

    /// Update the mesh for this render group
    pub fn update_mesh(&mut self, device: &wgpu::Device) {
        // calculate the total number of vertices for the combined mesh
        let vertex_count = self
            .chunk_mesh_data
            .iter()
            .filter_map(|mesh_data_opt| mesh_data_opt.as_ref())
            .map(|mesh_data| mesh_data.vertices.len())
            .sum();

        // concatenate each chunk's vertices
        let mut vertices = Vec::with_capacity(vertex_count);
        self.chunk_mesh_data
            .iter()
            .filter_map(|mesh_data_opt| mesh_data_opt.as_ref())
            .for_each(|mesh_data| vertices.extend_from_slice(&mesh_data.vertices));

        // generate the whole index buffer (the indices follow a repeating pattern so this
        // can be generated all at one)
        let indices = meshing::generate_indices(vertex_count);

        // create the new mesh
        self.mesh = Some(Mesh::new(device, &vertices, &indices));
    }

    /// Returns the status of the given chunk in the render group
    pub fn get_status_for_chunk(&self, chunk_pos_in_group: UVec3) -> ChunkMeshStatus {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        self.chunk_mesh_status[index]
    }

    /// Returns the mesh for this render group, if it has one
    pub fn mesh(&self) -> Option<&Mesh> {
        self.mesh.as_ref()
    }

    /// Returns the bind group for this render group's uniforms
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    /// Returns the position of this render group in the grid of groups
    pub fn pos(&self) -> IVec3 {
        self.pos
    }

    /// True if the group has no mesh data for any chunk
    pub fn is_empty(&self) -> bool {
        self.chunk_mesh_data
            .iter()
            .all(|mesh_data| mesh_data.is_none())
    }

    /// Returns the index in `self.vertices_for_chunk` for the chunk with the given position in the
    /// group
    fn get_index_for_chunk(pos: UVec3) -> usize {
        RENDER_GROUP_SIZE_SQUARED * pos.z as usize
            + RENDER_GROUP_SIZE * pos.y as usize
            + pos.x as usize
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct RenderGroupUniforms {
    translation: [f32; 3],
    pad: f32,
}
