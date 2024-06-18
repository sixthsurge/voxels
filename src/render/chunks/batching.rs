use glam::{IVec3, UVec3};
use wgpu::util::DeviceExt;

use super::{meshing, vertex::ChunkVertex, ChunkMeshData, ChunkMeshStatus};
use crate::terrain::chunk::CHUNK_SIZE;

/// Size of one chunk batch on each axis, in chunks
/// Larger batches mean fewer draw calls, but more time spent recombining meshes and
/// less granularity for culling
pub const CHUNK_BATCH_SIZE: usize = 2;

/// Size of one chunk batch on each axis, squared
pub const CHUNK_BATCH_SIZE_SQUARED: usize = CHUNK_BATCH_SIZE * CHUNK_BATCH_SIZE;

/// Size of one chunk batch on each axis, cubed.
/// The number of chunks in one batch
pub const CHUNK_BATCH_SIZE_CUBED: usize = CHUNK_BATCH_SIZE * CHUNK_BATCH_SIZE * CHUNK_BATCH_SIZE;

/// The length of one chunk batch in the world
pub const CHUNK_BATCH_TOTAL_SIZE: usize = CHUNK_SIZE * CHUNK_BATCH_SIZE;

/// To reduce draw calls, neighbouring chunks are grouped into batches, where the mesh of
/// the batch is the concatenation of the meshes of the chunks it contains.
/// This is the struct that holds the terrain meshes that are actually sent to the GPU.
/// The disadvantage of this approach is that chunk vertices need to be kept in memory in order
/// to update the batch (normally they could just be discarded once sent to the GPU).
#[derive(Debug)]
pub struct ChunkBatch {
    /// Combined vertex buffer for all chunks in this batch
    vertex_buffer: Option<wgpu::Buffer>,
    /// Number of vertices in `vertex_buffer`
    vertex_count: usize,
    /// Mesh data for each chunk in the batch
    chunk_mesh_data: [Option<ChunkMeshData>; CHUNK_BATCH_SIZE_CUBED],
    /// Mesh status for each chunk in the batch
    chunk_mesh_status: [ChunkMeshStatus; CHUNK_BATCH_SIZE_CUBED],
    /// Bind group for render-group-specific uniforms
    bind_group: wgpu::BindGroup,
}

impl ChunkBatch {
    pub fn new(
        pos: IVec3,
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let batch_translation = pos.as_vec3() * (CHUNK_BATCH_TOTAL_SIZE as f32);
        let batch_translation = batch_translation.to_array();

        let uniforms = ChunkBatchUniforms {
            translation: batch_translation,
            pad: 0.0,
        };

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Batch Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Batch Uniforms Bind Group"),
            layout: bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let chunk_mesh_data = array_init::array_init(|_| None);
        let chunk_mesh_status = array_init::array_init(|_| ChunkMeshStatus::Missing);

        Self {
            vertex_buffer: None,
            vertex_count: 0,
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
        debug_assert!(chunk_pos_in_group.max_element() < CHUNK_BATCH_SIZE as u32);

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
        if !self.chunk_mesh_status[index].is_missing() {
            self.chunk_mesh_status[index] = ChunkMeshStatus::Outdated;
        }
    }

    /// Mark that the mesh data for the given chunk position can be optimized, if it exists
    pub fn mark_suboptimal(&mut self, chunk_pos_in_group: UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        if !self.chunk_mesh_status[index].is_missing()
            && !self.chunk_mesh_status[index].is_outdated()
        {
            self.chunk_mesh_status[index] = ChunkMeshStatus::Suboptimal;
        }
    }

    /// Update the vertex_buffer for this batch
    pub fn update_vertex_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        // calculate the total number of vertices for the combined mesh
        self.vertex_count = self
            .chunk_mesh_data
            .iter()
            .filter_map(|mesh_data_opt| mesh_data_opt.as_ref())
            .map(|mesh_data| mesh_data.vertices.len())
            .sum();

        // concatenate each chunk's vertices
        let mut vertices = Vec::with_capacity(self.vertex_count);
        self.chunk_mesh_data
            .iter()
            .filter_map(|mesh_data_opt| mesh_data_opt.as_ref())
            .for_each(|mesh_data| vertices.extend_from_slice(&mesh_data.vertices));

        // see if we can reuse the existing vertex buffer
        if let Some(old_vertex_buffer) = self
            .vertex_buffer
            .as_ref()
            .filter(|old_vertex_buffer| {
                self.vertex_count * std::mem::size_of::<ChunkVertex>()
                    <= old_vertex_buffer.size() as usize
            })
        {
            queue.write_buffer(&old_vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        } else {
            self.vertex_buffer = Some(device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: None,
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                },
            ));
        }
    }

    /// Returns the status of the given chunk in the batch
    pub fn get_status_for_chunk(&self, chunk_pos_in_group: UVec3) -> ChunkMeshStatus {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        self.chunk_mesh_status[index]
    }

    /// Returns the vertex buffer for this batch, if it has one
    pub fn vertex_buffer(&self) -> Option<&wgpu::Buffer> {
        self.vertex_buffer.as_ref()
    }

    /// Returns the number of vertices in this batch
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Returns the number of indices required to draw this batch
    pub fn index_count(&self) -> usize {
        self.vertex_count * 3 / 2
    }

    /// Returns the bind group for this batch's uniforms
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
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
        CHUNK_BATCH_SIZE_SQUARED * pos.z as usize
            + CHUNK_BATCH_SIZE * pos.y as usize
            + pos.x as usize
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
struct ChunkBatchUniforms {
    translation: [f32; 3],
    pad: f32,
}
