use std::{
    collections::HashMap,
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

use glam::{IVec3, UVec3};
use itertools::Itertools;
use wgpu::util::DeviceExt;

use self::{meshing::ChunkMeshInput, vertex::ChunkVertex};
use crate::{
    render::util::mesh::Mesh,
    terrain::{
        chunk::{Chunk, CHUNK_SIZE, CHUNK_SIZE_I32},
        position_types::ChunkPos,
    },
};

pub mod meshing;
pub mod vertex;

/// Number of threads used for chunk mesh generation
pub const MESHING_THREAD_COUNT: usize = 2;

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
    /// Uniform buffer for this render group
    uniform_buffer: wgpu::Buffer,
    /// Bind group for group-specific uniforms
    bind_group: wgpu::BindGroup,
    /// Vertices of each chunk in the render group
    vertices_for_chunks: [Vec<ChunkVertex>; RENDER_GROUP_SIZE_CUBED],
    /// Instants of when each chunk's vertices was queued for meshing
    instants_for_chunks: [Option<Instant>; RENDER_GROUP_SIZE_CUBED],
}

impl ChunkRenderGroup {
    pub fn new(pos: IVec3, cx: RenderGroupCreateContext) -> Self {
        let render_group_offset = pos.as_vec3() * (RENDER_GROUP_TOTAL_SIZE as f32);
        let render_group_offset = render_group_offset.to_array();

        let uniforms = RenderGroupUniforms {
            translation: render_group_offset,
            pad: 0.0,
        };

        let uniform_buffer = cx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render Group Uniform Buffer"),
                contents: bytemuck::cast_slice(&[uniforms]),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            });

        let uniforms_bind_group = cx
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Render Group Uniforms Bind Group"),
                layout: cx.bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                }],
            });

        let vertices_for_chunks = array_init::array_init(|_| Vec::new());
        let instants_for_chunks = [None; RENDER_GROUP_SIZE_CUBED];

        Self {
            pos,
            mesh: None,
            uniform_buffer,
            bind_group: uniforms_bind_group,
            vertices_for_chunks,
            instants_for_chunks,
        }
    }

    /// Update the stored vertices for the given chunk.
    /// If `instant_the_chunk_was_queued_for_meshing` is earlier than the stored instant for this
    /// chunk, the new mesh will be discarded as it is out of date
    /// latest mesh
    /// This function will not update the mesh
    /// Returns true if the vertices were updated
    pub fn set_vertices_for_chunk(
        &mut self,
        chunk_pos_in_group: UVec3,
        vertices: Vec<ChunkVertex>,
        instant_the_chunk_was_queued_for_meshing: Instant,
    ) -> bool {
        debug_assert!(chunk_pos_in_group.max_element() < RENDER_GROUP_SIZE as u32);

        let index = Self::get_index_for_chunk(chunk_pos_in_group);

        if let Some(stored_instant) = self.instants_for_chunks[index] {
            if instant_the_chunk_was_queued_for_meshing < stored_instant {
                return false;
            }
        }

        self.vertices_for_chunks[index] = vertices;
        self.instants_for_chunks[index] = Some(instant_the_chunk_was_queued_for_meshing);

        return true;
    }

    /// Clear the stored vertices for the given chunk position
    pub fn clear_vertices_for_chunk(&mut self, chunk_pos_in_group: UVec3) {
        let index = Self::get_index_for_chunk(chunk_pos_in_group);
        self.vertices_for_chunks[index].clear();
    }

    /// Update the mesh for this render group
    pub fn update_mesh(&mut self, device: &wgpu::Device) {
        // calculate the total number of vertices for the combined mesh
        let vertex_count = self
            .vertices_for_chunks
            .iter()
            .map(|vertices| vertices.len())
            .sum();

        // concatenate each chunk's vertices
        let vertices = {
            let mut vertices = Vec::with_capacity(vertex_count);
            self.vertices_for_chunks
                .iter()
                .for_each(|vertices_for_chunk| vertices.extend_from_slice(vertices_for_chunk));
            vertices
        };

        // generate the whole index buffer (the indices follow a repeating pattern so this
        // can be generated all at one)
        let indices = meshing::generate_indices(vertex_count);

        // create the new mesh
        self.mesh = Some(Mesh::new(device, &vertices, &indices));
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

    /// True if the group has no vertices for any chunk
    pub fn is_empty(&self) -> bool {
        self.vertices_for_chunks
            .iter()
            .all(|vertices_for_chunk| vertices_for_chunk.is_empty())
    }

    /// Returns the index in `self.vertices_for_chunk` for the chunk with the given position in the
    /// group
    fn get_index_for_chunk(pos: UVec3) -> usize {
        RENDER_GROUP_SIZE_SQUARED * pos.z as usize
            + RENDER_GROUP_SIZE * pos.y as usize
            + pos.x as usize
    }
}

/// Responsible for managing chunk render groups, including coordinating mesh generation
#[derive(Debug)]
pub struct ChunkRenderGroups {
    /// Thread pool for mesh generation
    mesh_generation_threads: rayon::ThreadPool,
    /// HashMap storing the render groups indexed by their position in the grid
    active_groups: HashMap<IVec3, ChunkRenderGroup>,
    /// Vec of positions of active render groups, for quick iteration
    active_group_positions: Vec<IVec3>,
    /// Vec of positions of render groups that require mesh updates
    dirty_group_positions: Vec<IVec3>,
    /// Sender for finished chunk meshes
    finished_mesh_tx: Sender<(Instant, ChunkPos, Vec<ChunkVertex>)>,
    /// Receiver for finished chunk meshes
    finished_mesh_rx: Receiver<(Instant, ChunkPos, Vec<ChunkVertex>)>,
}

impl ChunkRenderGroups {
    pub fn new() -> Self {
        let mesh_generation_threads = rayon::ThreadPoolBuilder::new()
            .num_threads(MESHING_THREAD_COUNT)
            .build()
            .expect("creating thread pool should not fail");

        let (finished_mesh_tx, finished_mesh_rx) = mpsc::channel();

        Self {
            mesh_generation_threads,
            active_groups: HashMap::new(),
            active_group_positions: Vec::new(),
            dirty_group_positions: Vec::new(),
            finished_mesh_tx,
            finished_mesh_rx,
        }
    }

    /// Called each frame after `chunk_modified` or `chunk_unloaded`
    pub fn update(&mut self, device: &wgpu::Device, cx: RenderGroupCreateContext) {
        // check for newly finished mesh
        while let Ok(newly_finished_mesh_info) = self.finished_mesh_rx.try_recv() {
            let (instant_the_chunk_was_queued_for_meshing, chunk_pos, vertices) =
                newly_finished_mesh_info;

            self.finished_meshing_chunk(
                instant_the_chunk_was_queued_for_meshing,
                chunk_pos,
                vertices,
                cx,
            );
        }

        // update the meshes of any dirty groups
        for group_pos in self.dirty_group_positions.iter() {
            if let Some(group) = self.active_groups.get_mut(&group_pos) {
                group.update_mesh(device);
            }
        }
        self.dirty_group_positions.clear();

        // remove any empty groups
        let groups_to_remove = self
            .active_group_positions
            .iter()
            .copied()
            .filter(|group_pos| self.active_groups[group_pos].is_empty())
            .collect_vec();
        groups_to_remove
            .iter()
            .copied()
            .for_each(|group_pos| self.remove_group(group_pos));
    }

    /// Called when a chunk has been modified or loaded and requires meshing
    pub fn chunk_modified(&mut self, chunk: &Chunk) {
        self.queue_chunk_for_meshing(chunk);
    }

    /// Called when a chunk has been unloaded and its mesh should be removed
    pub fn chunk_unloaded(&mut self, chunk_pos: &ChunkPos) {
        let (group_pos, chunk_pos_in_group) =
            Self::get_group_pos_and_chunk_pos_in_group(&chunk_pos);

        if let Some(group) = self.active_groups.get_mut(&group_pos) {
            group.clear_vertices_for_chunk(chunk_pos_in_group);
            self.dirty_group_positions
                .push(group_pos);
        }
    }

    /// Returns an iterator over all active chunk render groups
    pub fn iter(&self) -> impl Iterator<Item = &ChunkRenderGroup> {
        self.active_group_positions
            .iter()
            .map(|pos| self.active_groups.get(pos).expect("`active_groups` should contain `pos` if it is contained in `active_group_positions`"))
    }

    /// Spawn a new task to generate a chunk's mesh
    /// If this function is multiple times for the same chunk before the mesh generation finishes,
    /// the mesh from the latest call is used
    fn queue_chunk_for_meshing(&mut self, chunk: &Chunk) {
        // instant that `call_chunk_for_meshing` was called
        let instant = Instant::now();

        // clone the sender for the worker thread to use
        let finished_mesh_tx = self.finished_mesh_tx.clone();

        // prepare a snapshot of data about the chunk for the meshing thread to use
        let chunk_pos = chunk.pos();
        let blocks = chunk.as_block_array();

        self.mesh_generation_threads
            .spawn(move || {
                // move `blocks` to the new thread
                let blocks = blocks;

                let translation = chunk_pos
                    .as_ivec3()
                    .rem_euclid(IVec3::splat(RENDER_GROUP_SIZE as i32))
                    * CHUNK_SIZE_I32;

                let vertices = meshing::mesh_greedy(ChunkMeshInput {
                    blocks: &blocks,
                    translation: translation.as_vec3(), // eventually this will be an IVec3
                });

                if let Err(e) = finished_mesh_tx.send((instant, chunk_pos, vertices)) {
                    log::trace!(
                        "sending chunk vertices from meshing thread to main thread returned error: {}",
                        e
                    );
                }
            });
    }

    /// Called whenever a finished chunk mesh arrives
    fn finished_meshing_chunk(
        &mut self,
        instant_the_chunk_was_queued_for_meshing: Instant,
        chunk_pos: ChunkPos,
        vertices: Vec<ChunkVertex>,
        cx: RenderGroupCreateContext,
    ) {
        let (group_pos, chunk_pos_in_group) =
            Self::get_group_pos_and_chunk_pos_in_group(&chunk_pos);

        let group = self.get_or_add_group(group_pos, cx);

        if group.set_vertices_for_chunk(
            chunk_pos_in_group,
            vertices,
            instant_the_chunk_was_queued_for_meshing,
        ) {
            self.dirty_group_positions
                .push(group_pos);
        }
    }

    /// Create a new render group and add it to the active groups
    /// Returns the new group
    fn add_new_group(
        &mut self,
        group_pos: IVec3,
        cx: RenderGroupCreateContext,
    ) -> &mut ChunkRenderGroup {
        debug_assert!(!self
            .active_groups
            .contains_key(&group_pos));
        debug_assert!(!self
            .active_group_positions
            .contains(&group_pos));

        let group = ChunkRenderGroup::new(group_pos, cx);

        self.active_group_positions
            .push(group_pos);
        self.active_groups
            .insert(group_pos, group);

        self.active_groups
            .get_mut(&group_pos)
            .expect("group should exist as it was just created")
    }

    /// Remove a render group from the active groups
    /// Panics if the group does not exist
    fn remove_group(&mut self, group_pos: IVec3) {
        debug_assert!(self
            .active_groups
            .contains_key(&group_pos));
        debug_assert!(self
            .active_group_positions
            .contains(&group_pos));

        self.active_groups.remove(&group_pos);
        self.active_group_positions.remove(
            self.active_group_positions
                .iter()
                .position(|x| *x == group_pos)
                .expect("position of chunk render group being removed should be in `active_group_positions`"),
        );
    }

    /// Returns a mutable reference to the render group at the given position, creating it if it
    /// does not exist
    fn get_or_add_group(
        &mut self,
        group_pos: IVec3,
        cx: RenderGroupCreateContext,
    ) -> &mut ChunkRenderGroup {
        if self
            .active_groups
            .contains_key(&group_pos)
        {
            self.active_groups
                .get_mut(&group_pos)
                .unwrap()
        } else {
            self.add_new_group(group_pos, cx)
        }
    }

    /// Given a chunk position, returns the position of its render group in the grid and the
    /// position of the chunk in the render group
    fn get_group_pos_and_chunk_pos_in_group(chunk_pos: &ChunkPos) -> (IVec3, UVec3) {
        let chunk_pos = chunk_pos.as_ivec3();
        let group_pos = chunk_pos.div_euclid(IVec3::splat(RENDER_GROUP_SIZE as i32));
        let chunk_pos_in_group = chunk_pos - group_pos * (RENDER_GROUP_SIZE as i32);
        (group_pos, chunk_pos_in_group.as_uvec3())
    }
}

/// Context required to create a chunk render group
#[derive(Clone, Copy, Debug)]
pub struct RenderGroupCreateContext<'a> {
    pub device: &'a wgpu::Device,
    pub bind_group_layout: &'a wgpu::BindGroupLayout,
}

/// Uniforms specific to each render group
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct RenderGroupUniforms {
    translation: [f32; 3],
    pad: f32,
}
