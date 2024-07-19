use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

use generational_arena::Index;
use glam::{IVec3, UVec3, Vec3};
use itertools::Itertools;
use wgpu::util::DeviceExt;

use super::{
    meshing::{self, ChunkMeshInput},
    vertex::TerrainVertex,
    ChunkMeshData, ChunkMeshStatus,
};
use crate::{
    core::{
        tasks::{TaskId, TaskPriority, Tasks},
        wgpu_util::wgpu_context::WgpuContext,
    },
    terrain::{
        block::BLOCK_AIR,
        chunk::{block_store::ChunkBlockStore, side::ChunkSide, Chunk, CHUNK_SIZE, CHUNK_SIZE_I32},
        load_area::LoadArea,
        position_types::ChunkPosition,
        Terrain,
    },
    util::size::Size3,
};

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
/// Chunk batches live for a long time - they are initialized with the terrain renderer and
/// reused for new chunks. This means that the vertex buffers are reused
#[derive(Debug)]
pub struct ChunkBatch {
    /// Whether this batch has received new chunk vertices since the last time `update_vertex_buffer`
    /// was called
    vertex_buffer_needs_updating: bool,
    /// Current position of this batch in the grid of chunk batches
    position: IVec3,
    /// Combined vertex buffer for all chunks in this batch
    vertex_buffer: Option<wgpu::Buffer>,
    /// Number of vertices in `vertex_buffer`
    vertex_count: usize,
    /// Mesh data for each chunk in the batch
    chunk_mesh_data: [Option<ChunkMeshData>; CHUNK_BATCH_SIZE_CUBED],
    /// Mesh status for each chunk in the batch
    chunk_mesh_status: [ChunkMeshStatus; CHUNK_BATCH_SIZE_CUBED],
    /// Uniform buffer for batch-specific uniforms
    uniform_buffer: wgpu::Buffer,
    /// Bind group for the uniform buffer
    uniform_bind_group: wgpu::BindGroup,
}

impl ChunkBatch {
    pub fn new(
        pos: IVec3,
        wgpu: &WgpuContext,
        uniform_bind_group_layout: &wgpu::BindGroupLayout,
    ) -> Self {
        let batch_translation = pos.as_vec3() * (CHUNK_BATCH_TOTAL_SIZE as f32);
        let batch_translation = batch_translation.to_array();

        let uniforms = ChunkBatchUniforms {
            translation: batch_translation,
            pad: 0.0,
        };

        let uniform_buffer = wgpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Chunk Batch Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let uniform_bind_group = wgpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Chunk Batch Uniforms Bind Group"),
            layout: uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let chunk_mesh_data = array_init::array_init(|_| None);
        let chunk_mesh_status = array_init::array_init(|_| ChunkMeshStatus::Missing);

        Self {
            vertex_buffer_needs_updating: false,
            position: pos,
            vertex_buffer: None,
            vertex_count: 0,
            chunk_mesh_data,
            chunk_mesh_status,
            uniform_buffer,
            uniform_bind_group,
        }
    }

    /// Reset this chunk batch so that it can be reused
    pub fn reset(&mut self, wgpu: &WgpuContext, pos: IVec3) {
        self.vertex_buffer_needs_updating = false;
        self.position = pos;
        self.vertex_count = 0;
        self.chunk_mesh_data = array_init::array_init(|_| None);
        self.chunk_mesh_status = array_init::array_init(|_| ChunkMeshStatus::Missing);

        // update the uniform buffer
        let batch_translation = pos.as_vec3() * (CHUNK_BATCH_TOTAL_SIZE as f32);
        let batch_translation = batch_translation.to_array();

        let uniforms = ChunkBatchUniforms {
            translation: batch_translation,
            pad: 0.0,
        };

        wgpu.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));
    }

    /// Update the stored vertices for the given chunk.
    /// If `queued_instant` is earlier than the stored instant for this
    /// chunk, the new mesh will be discarded as it is out of date
    /// This function will not update the group's combined mesh
    /// Returns true if the vertices were updated
    pub fn set_mesh_data_for_chunk(
        &mut self,
        chunk_pos_in_batch: UVec3,
        mesh_data: ChunkMeshData,
    ) -> bool {
        debug_assert!(chunk_pos_in_batch.max_element() < CHUNK_BATCH_SIZE as u32);

        let index = Self::get_index_for_chunk(&chunk_pos_in_batch);

        if let Some(existing_mesh_data) = &self.chunk_mesh_data[index] {
            if mesh_data.queued_instant < existing_mesh_data.queued_instant {
                return false;
            }
        }

        self.chunk_mesh_data[index] = Some(mesh_data);
        self.chunk_mesh_status[index] = ChunkMeshStatus::Good;
        self.vertex_buffer_needs_updating = true;
        return true;
    }

    /// Clear the stored vertices for the given chunk position
    pub fn clear_mesh_data_for_chunk(&mut self, chunk_pos_in_batch: &UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_batch);
        self.chunk_mesh_data[index] = None;
    }

    /// Mark that the mesh data for the given chunk position is currently being generated
    pub fn mark_generating(&mut self, chunk_pos_in_batch: &UVec3, task_id: TaskId) {
        let index = Self::get_index_for_chunk(chunk_pos_in_batch);
        self.chunk_mesh_status[index] = ChunkMeshStatus::Generating(task_id);
    }

    /// Mark that the mesh data for the given chunk position is outdated, if it exists
    pub fn mark_outdated(&mut self, chunk_pos_in_batch: &UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_batch);
        if !self.chunk_mesh_status[index].is_missing() {
            self.chunk_mesh_status[index] = ChunkMeshStatus::Outdated;
        }
    }

    /// Mark that the mesh data for the given chunk position can be optimized, if it exists
    pub fn mark_suboptimal(&mut self, chunk_pos_in_batch: &UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_batch);
        if !self.chunk_mesh_status[index].is_missing()
            && !self.chunk_mesh_status[index].is_outdated()
        {
            self.chunk_mesh_status[index] = ChunkMeshStatus::Suboptimal;
        }
    }

    /// Update the vertex_buffer for this batch
    pub fn update_vertex_buffer(&mut self, device: &wgpu::Device, queue: &wgpu::Queue) {
        self.vertex_buffer_needs_updating = false;

        // calculate the total number of vertices for the combined mesh
        self.vertex_count = self
            .chunk_mesh_data
            .iter()
            .filter_map(|mesh_data_opt| mesh_data_opt.as_ref())
            .map(|mesh_data| mesh_data.vertices.len())
            .sum();

        if self.vertex_count == 0 {
            self.vertex_buffer = None;
            return;
        }

        // concatenate each chunk's vertices
        let mut vertices = Vec::with_capacity(self.vertex_count);
        self.chunk_mesh_data
            .iter()
            .filter_map(|mesh_data_opt| mesh_data_opt.as_ref())
            .for_each(|mesh_data| vertices.extend_from_slice(&mesh_data.vertices));

        // see if we can reuse the existing vertex buffer
        if let Some(old_vertex_buffer) = self.vertex_buffer.as_ref().filter(|old_vertex_buffer| {
            self.vertex_count * std::mem::size_of::<TerrainVertex>()
                <= old_vertex_buffer.size() as usize
        }) {
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
    pub fn get_chunk_mesh_status(&self, chunk_pos_in_batch: &UVec3) -> ChunkMeshStatus {
        let index = Self::get_index_for_chunk(chunk_pos_in_batch);
        self.chunk_mesh_status[index]
    }

    /// Returns the vertex buffer for this batch, if it has one
    pub fn vertex_buffer(&self) -> Option<&wgpu::Buffer> {
        self.vertex_buffer.as_ref()
    }

    /// Returns the bind group for this batch's uniforms
    pub fn uniform_bind_group(&self) -> &wgpu::BindGroup {
        &self.uniform_bind_group
    }

    /// Returns the number of vertices in this batch
    pub fn vertex_count(&self) -> usize {
        self.vertex_count
    }

    /// Returns the number of indices required to draw this batch
    pub fn index_count(&self) -> usize {
        self.vertex_count * 3 / 2
    }

    /// Returns the index in `self.vertices_for_chunk` for the chunk with the given position in the
    /// group
    fn get_index_for_chunk(pos: &UVec3) -> usize {
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

/// Responsible for managing chunk batches, including issuing mesh generation tasks
#[derive(Debug)]
pub struct ChunkBatches {
    /// 3D array of chunk batches indexed by flatten(`batch_pos % batch_grid_size`)
    batches: Vec<ChunkBatch>,
    /// Size of the grid of chunk batches
    batch_grid_size: Size3,
    /// Sender for finished chunk meshes
    finished_mesh_tx: Sender<(ChunkPosition, ChunkMeshData)>,
    /// Receiver for finished chunk meshes
    finished_mesh_rx: Receiver<(ChunkPosition, ChunkMeshData)>,
    /// Bind group layout for uniforms specific to each chunk batch
    uniform_bind_group_layout: wgpu::BindGroupLayout,
    /// Shared index buffer for rendering chunk batches
    shared_index_buffer: SharedIndexBuffer,
}

impl ChunkBatches {
    pub fn new(
        wgpu: &WgpuContext,
        load_area: &LoadArea,
        uniform_bind_group_layout: wgpu::BindGroupLayout,
    ) -> Self {
        let batch_grid_size = Self::compute_batch_grid_size(load_area);

        let batches = itertools::iproduct!(
            (0..batch_grid_size.x),
            (0..batch_grid_size.y),
            (0..batch_grid_size.z),
        )
        .map(|(x, y, z)| {
            let pos = Size3::new(x, y, z).as_ivec3();
            ChunkBatch::new(pos, wgpu, &uniform_bind_group_layout)
        })
        .collect_vec();

        let (finished_mesh_tx, finished_mesh_rx) = mpsc::channel();

        let shared_index_buffer =
            SharedIndexBuffer::new(&wgpu.device, SharedIndexBuffer::INITIAL_VERTEX_COUNT);

        Self {
            batches,
            batch_grid_size,
            finished_mesh_tx,
            finished_mesh_rx,
            uniform_bind_group_layout,
            shared_index_buffer,
        }
    }

    /// Returns the position of the batch in the grid of batches containing the chunk and the
    /// position of the chunk in the batch
    pub fn get_batch_pos_and_chunk_pos_in_batch(chunk_pos: &ChunkPosition) -> (IVec3, UVec3) {
        let chunk_pos = chunk_pos.as_ivec3();
        let batch_pos = chunk_pos.div_euclid(IVec3::splat(CHUNK_BATCH_SIZE as i32));
        let chunk_pos_in_batch = chunk_pos - batch_pos * (CHUNK_BATCH_SIZE as i32);

        (batch_pos, chunk_pos_in_batch.as_uvec3())
    }

    /// Returns the index in `batches` for the given batch position
    pub fn get_batch_index(&self, batch_pos: &IVec3) -> usize {
        let grid_pos = batch_pos
            .rem_euclid(self.batch_grid_size.as_ivec3())
            .as_uvec3();

        self.batch_grid_size.flatten(grid_pos)
    }

    /// Called each frame before rendering terrain to update the chunk batches
    pub fn update(&mut self, wgpu: &WgpuContext, terrain: &Terrain, load_area_index: Index) {
        // check for newly finished meshes
        while let Ok(received) = self.finished_mesh_rx.try_recv() {
            let load_area = terrain
                .load_areas()
                .get(load_area_index)
                .expect("load area should exist");

            self.finished_mesh_received(load_area, received.0, received.1);
        }

        // update the vertex buffers of any batches requiring it
        let mut highest_vertex_count = self.shared_index_buffer.vertex_count;
        for batch in &mut self.batches {
            if batch.vertex_buffer_needs_updating {
                batch.update_vertex_buffer(&wgpu.device, &wgpu.queue);
                highest_vertex_count = highest_vertex_count.max(batch.vertex_count());
            }
        }

        // grow the shared index buffer if necessary
        if highest_vertex_count > self.shared_index_buffer.vertex_count {
            self.shared_index_buffer = SharedIndexBuffer::new(&wgpu.device, highest_vertex_count);
        }
    }

    /// Returns a shared reference to the batch at the given position, or None if there is no batch
    /// assigned to this position
    pub fn get_batch(&self, batch_pos: &IVec3) -> Option<&ChunkBatch> {
        let index = self.get_batch_index(batch_pos);
        let batch = &self.batches[index];

        if batch.position == *batch_pos {
            Some(batch)
        } else {
            // the batch at this index is assigned to another position
            None
        }
    }

    /// Returns a shared reference to the batch at the given position, or None if there is no batch
    /// assigned to this position
    pub fn get_batch_mut(&mut self, batch_pos: &IVec3) -> Option<&mut ChunkBatch> {
        let index = self.get_batch_index(batch_pos);
        let batch = &mut self.batches[index];

        if batch.position == *batch_pos {
            Some(batch)
        } else {
            // the batch at this index is assigned to another position
            None
        }
    }

    /// Repurposes the chunk batch with the same index in `batches` to the new position
    /// No-op if the batch position already matches
    /// Returns a mutable reference to the repurposed chunk batch
    pub fn get_or_repurpose_batch(
        &mut self,
        wgpu: &WgpuContext,
        tasks: &mut Tasks,
        batch_pos: &IVec3,
    ) -> &mut ChunkBatch {
        let index = self.get_batch_index(batch_pos);
        let batch = &mut self.batches[index];

        if batch.position != *batch_pos {
            // cancel any mesh generation tasks queued for this chunk batch
            for chunk_mesh_status in batch.chunk_mesh_status {
                match chunk_mesh_status {
                    ChunkMeshStatus::Generating(task_id) => {
                        tasks.cancel_if_pending(task_id);
                    }
                    _ => (),
                }
            }

            batch.reset(wgpu, *batch_pos);
        }

        batch
    }

    /// Spawn a new task to generate a chunk's mesh
    /// If this function is called multiple times for the same chunk before the mesh generation
    /// finishes, the mesh from the latest call is used
    pub fn queue_chunk_for_meshing(
        &mut self,
        chunk: &Chunk,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        camera_pos: Vec3,
        priority: i32,
    ) {
        let queued_instant = Instant::now();

        // skip meshing air chunks
        if let ChunkBlockStore::Uniform(block_id) = chunk.get_block_store() {
            if *block_id == BLOCK_AIR {
                let _ = self.finished_mesh_tx.send((
                    chunk.position(),
                    ChunkMeshData {
                        vertices: Vec::new(),
                        queued_instant,
                    },
                ));
            }
        }

        let finished_mesh_tx = self.finished_mesh_tx.clone();

        let (batch_pos, chunk_pos_in_batch) =
            Self::get_batch_pos_and_chunk_pos_in_batch(&chunk.position());

        let batch = self.get_batch_mut(&batch_pos).expect("batch should exist");

        // if we already issued a task to generate this mesh, cancel it
        match batch.get_chunk_mesh_status(&chunk_pos_in_batch) {
            ChunkMeshStatus::Generating(task_id) => {
                tasks.cancel_if_pending(task_id);
            }
            _ => (),
        }

        // prepare a snapshot of data about the chunk to be passed to the meshing thread
        let chunk_pos = chunk.position();
        let block_store = chunk.get_block_store().clone();
        let surrounding_sides =
            ChunkSide::get_surrounding_sides(chunk_pos, terrain, load_area_index);

        // assign a higher priority to chunks closer to the camera
        let priority_within_class = (chunk_pos.as_vec3() - camera_pos).length_squared() as i32;

        let task_id = tasks.submit(
            TaskPriority {
                class_priority: priority,
                priority_within_class,
            },
            move || {
                // move `blocks` and `surrounding sides` to the new thread
                let (blocks, surrounding_sides) = (block_store, surrounding_sides);
                let blocks = blocks.as_block_array();

                let translation = chunk_pos
                    .as_ivec3()
                    .rem_euclid(IVec3::splat(CHUNK_BATCH_SIZE as i32))
                    * CHUNK_SIZE_I32;

                let vertices = meshing::mesh_greedy(ChunkMeshInput {
                    blocks: &blocks,
                    translation: translation.as_vec3(), // eventually this will be an IVec3
                    surrounding_sides: &surrounding_sides,
                });

                if let Err(e) = finished_mesh_tx.send((chunk_pos, ChunkMeshData {
                    vertices,
                    queued_instant,
                })) {
                    log::trace!(
                        "sending chunk vertices from meshing thread to main thread returned error: {}",
                        e
                    );
                }
            });

        batch.mark_generating(&chunk_pos_in_batch, task_id);
    }

    /// Size of the grid of chunk batches
    pub fn size(&self) -> Size3 {
        self.batch_grid_size
    }

    /// Index buffer used to draw all chunk batches
    pub fn shared_index_buffer(&self) -> &wgpu::Buffer {
        &self.shared_index_buffer.index_buffer
    }

    /// Called whenever a finished chunk mesh arrives
    fn finished_mesh_received(
        &mut self,
        loaded_area: &LoadArea,
        chunk_pos: ChunkPosition,
        mesh_data: ChunkMeshData,
    ) {
        // make sure that the chunk is still loaded
        if !loaded_area.is_loaded(&chunk_pos) {
            return;
        }

        let (batch_pos, chunk_pos_in_batch) =
            Self::get_batch_pos_and_chunk_pos_in_batch(&chunk_pos);

        let Some(batch) = self.get_batch_mut(&batch_pos) else {
            return;
        };

        batch.set_mesh_data_for_chunk(chunk_pos_in_batch, mesh_data);
    }

    fn compute_batch_grid_size(load_area: &LoadArea) -> Size3 {
        load_area.size() / Size3::splat(CHUNK_BATCH_SIZE) + Size3::ONE
    }
}

/// As the indices for drawing chunk batches follow the same pattern for all batches, one index
/// buffer is shared between all batches
#[derive(Debug)]
pub struct SharedIndexBuffer {
    index_buffer: wgpu::Buffer,
    vertex_count: usize,
}

impl SharedIndexBuffer {
    pub const INITIAL_VERTEX_COUNT: usize = 50000;

    /// Create a shared index buffer for `vertex` vertices
    pub fn new(device: &wgpu::Device, vertex_count: usize) -> Self {
        let indices = meshing::generate_indices(vertex_count);
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Chunk Renderer Shared Index Buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            index_buffer,
            vertex_count,
        }
    }
}
