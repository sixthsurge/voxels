use std::{
    sync::mpsc::{self, Receiver, Sender},
    time::Instant,
};

use generational_arena::Index;
use glam::{IVec3, UVec3, Vec3};
use itertools::Itertools;
use rustc_hash::{FxHashMap, FxHashSet};
use wgpu::{util::DeviceExt, BufferAddress};

use self::{
    batching::{ChunkBatch, CHUNK_BATCH_SIZE},
    meshing::ChunkMeshInput,
    vertex::ChunkVertex,
    visibility_search::visibility_search,
};
use super::{
    frustum_culling::FrustumCullingRegions,
    render_context::RenderContext,
    render_engine::RenderEngine,
    util::{
        bind_group_builder::BindGroupBuilder,
        mip_generator::MipGenerator,
        pipeline_builder::RenderPipelineBuilder,
        texture::{ArrayTexture, TextureConfig, TextureHolder},
    },
};
use crate::{
    tasks::{TaskId, TaskPriority, Tasks},
    terrain::{
        chunk::{side::ChunkSide, Chunk, CHUNK_SIZE_I32},
        event::TerrainEvent,
        load_area::LoadArea,
        position_types::{ChunkPos, LocalBlockPos},
        Terrain,
    },
    CHUNK_MESH_GENERATION_PRIORITY, CHUNK_MESH_OPTIMIZATION_PRIORITY, CHUNK_MESH_UPDATE_PRIORITY,
};

mod batching;
mod meshing;
mod vertex;
mod visibility_search;

/// Responsible for rendering the voxel terrain
pub struct ChunkRenderer {
    /// Cull mode to use
    cull_mode: TerrainCullMode,
    /// Render pipeline for drawing chunk batches
    terrain_pipeline: wgpu::RenderPipeline,
    /// Bind group for the texture array
    texture_bind_group: wgpu::BindGroup,
    /// Bind group layout for uniforms specific to each chunk batch
    batch_bind_group_layout: wgpu::BindGroupLayout,
    /// As the indices for drawing chunk batches follow the same pattern for all batches, one index
    /// buffer is shared between all batches
    shared_index_buffer: wgpu::Buffer,
    /// Number of elements in `shared_index_buffer`
    shared_index_buffer_vertex_count: usize,
    /// HashMap storing active chunk batches indexed by their position in the grid
    batches: FxHashMap<IVec3, ChunkBatch>,
    /// Positions of all batches, for quick iteration
    batch_positions: Vec<IVec3>,
    /// Positions of batches that require mesh updates
    batches_requiring_mesh_updates: Vec<IVec3>,
    /// Task IDs for active mesh generation tasks
    meshing_tasks: FxHashMap<ChunkPos, TaskId>,
    /// Sender for finished chunk meshes
    finished_mesh_tx: Sender<(ChunkPos, ChunkMeshData)>,
    /// Receiver for finished chunk meshes
    finished_mesh_rx: Receiver<(ChunkPos, ChunkMeshData)>,
}

impl ChunkRenderer {
    pub const MIP_LEVEL_COUNT: u32 = 4;
    pub const SHARED_INDEX_BUFFER_INITIAL_VERTEX_COUNT: usize = 50000;

    pub fn new(
        cx: &RenderContext,
        common_uniforms_bind_group_layout: &wgpu::BindGroupLayout,
        cull_mode: TerrainCullMode,
    ) -> Self {
        // TODO load texture and shader using proper asset system rather than doing it here
        let texture_array = ArrayTexture::from_files(
            &cx.device,
            &cx.queue,
            &[
                "assets/image/block/dirt.png",
                "assets/image/block/grass_side.png",
                "assets/image/block/grass_top.png",
                "assets/image/block/wood.png",
            ],
            image::ImageFormat::Png,
            &TextureConfig {
                mip_level_count: Self::MIP_LEVEL_COUNT,
                ..Default::default()
            },
        )
        .expect("failed to load terrain textures")
        .with_view_and_sampler(
            &cx.device,
            wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                ..Default::default()
            },
        );

        // generate mipmaps
        let mut mip_encoder = cx
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        let mip_generator = MipGenerator::new(&cx.device, wgpu::TextureFormat::Rgba8UnormSrgb);
        mip_generator.generate_mips(
            &mut mip_encoder,
            &cx.device,
            texture_array.texture(),
            texture_array.size().z,
            Self::MIP_LEVEL_COUNT,
        );
        cx.queue
            .submit(std::iter::once(mip_encoder.finish()));

        let (texture_bind_group, texture_bind_group_layout) = BindGroupBuilder::new()
            .with_label("Texture Array Bind Group")
            .with_texture_view(
                texture_array.view(),
                wgpu::TextureViewDimension::D2Array,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::ShaderStages::FRAGMENT,
            )
            .with_sampler(
                texture_array.sampler(),
                wgpu::SamplerBindingType::Filtering,
                wgpu::ShaderStages::FRAGMENT,
            )
            .build(&cx.device);

        let batch_bind_group_layout =
            cx.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: None,
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                });

        let terrain_shader = cx
            .device
            .create_shader_module(wgpu::include_wgsl!("../../assets/shader/terrain.wgsl"));

        let (terrain_pipeline, _) = RenderPipelineBuilder::new()
            .with_label("Terrain Pipeline")
            .with_bind_group_layout(&texture_bind_group_layout)
            .with_bind_group_layout(&common_uniforms_bind_group_layout)
            .with_bind_group_layout(&batch_bind_group_layout)
            .with_vertex::<ChunkVertex>()
            .with_vertex_shader(&terrain_shader, "vs_main")
            .with_fragment_shader(&terrain_shader, "fs_main")
            .with_color_target(
                cx.surface_config.format,
                Some(wgpu::BlendState::REPLACE),
                wgpu::ColorWrites::all(),
            )
            .with_depth(RenderEngine::DEPTH_FORMAT, RenderEngine::DEPTH_COMPARE)
            //.with_polygon_mode(wgpu::PolygonMode::Line)
            .build(&cx.device);

        let (finished_mesh_tx, finished_mesh_rx) = mpsc::channel();

        let shared_indices =
            meshing::generate_indices(Self::SHARED_INDEX_BUFFER_INITIAL_VERTEX_COUNT);
        let shared_index_buffer = cx
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Chunk Renderer Shared Index Buffer"),
                contents: bytemuck::cast_slice(&shared_indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        Self {
            cull_mode,
            terrain_pipeline,
            texture_bind_group,
            batch_bind_group_layout,
            shared_index_buffer,
            shared_index_buffer_vertex_count: Self::SHARED_INDEX_BUFFER_INITIAL_VERTEX_COUNT,
            batches: FxHashMap::default(),
            batch_positions: Vec::new(),
            batches_requiring_mesh_updates: Vec::new(),
            meshing_tasks: FxHashMap::default(),
            finished_mesh_tx,
            finished_mesh_rx,
        }
    }

    /// Called once per frame to render the terrain
    pub fn render(
        &mut self,
        render_encoder: &mut wgpu::CommandEncoder,
        output_view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        common_uniforms_bind_group: &wgpu::BindGroup,
        cx: &RenderContext,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        frustum_culling_regions: &FrustumCullingRegions,
        camera_pos: Vec3,
    ) {
        // get the list of chunks to be rendered in order
        let render_queue = match self.cull_mode {
            TerrainCullMode::CullNone => terrain
                .chunks()
                .iter()
                .map(|(_, chunk)| chunk)
                .collect_vec(),
            TerrainCullMode::Frustum => terrain
                .chunks()
                .iter()
                .map(|(_, chunk)| chunk)
                .filter(|chunk| frustum_culling_regions.is_chunk_within_frustum(&chunk.pos()))
                .collect_vec(),
            TerrainCullMode::VisibilitySearch => visibility_search(
                terrain,
                load_area_index,
                frustum_culling_regions,
                camera_pos,
            ),
        };

        // prepare for rendering, updating meshes as necessary
        self.prepare(
            cx,
            &render_queue,
            tasks,
            terrain,
            load_area_index,
            camera_pos,
        );

        for chunk in &render_queue {
            self.request_mesh_updates_for_chunk(chunk, tasks, terrain, load_area_index, camera_pos);
        }

        // begin render pass
        let mut render_pass = render_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Terrain Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.25,
                        g: 0.45,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.terrain_pipeline);
        render_pass.set_bind_group(0, &self.texture_bind_group, &[]);
        render_pass.set_bind_group(1, &common_uniforms_bind_group, &[]);

        // will track which chunk batches have already been drawn, to avoid issuing multiple
        // draw calls for the same group
        let mut drawn_batches = FxHashSet::default();

        for chunk in &render_queue {
            let (group_pos, _) = Self::get_group_pos_and_chunk_pos_in_batch(chunk.pos());

            if drawn_batches.contains(&group_pos) {
                continue;
            }

            let Some(batch) = self.batches.get(&group_pos) else {
                continue;
            };
            let Some(vertex_buffer) = batch.vertex_buffer() else {
                continue;
            };

            render_pass.set_bind_group(2, batch.bind_group(), &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.set_index_buffer(
                self.shared_index_buffer
                    .slice(0..(4 * batch.index_count() as BufferAddress)),
                wgpu::IndexFormat::Uint32,
            );
            render_pass.draw_indexed(0..(batch.index_count() as u32), 0, 0..1);

            drawn_batches.insert(group_pos);
        }
    }

    /// Called once per frame to prepare for rendering
    pub fn prepare(
        &mut self,
        cx: &RenderContext,
        render_queue: &[&Chunk],
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        camera_pos: Vec3,
    ) {
        // request necessary mesh updates for all chunks in the queue
        for chunk in render_queue {
            self.request_mesh_updates_for_chunk(chunk, tasks, terrain, load_area_index, camera_pos);
        }

        // process terrain events
        for event in terrain.events() {
            match event {
                TerrainEvent::ChunkLoaded(chunk_pos) => self.chunk_loaded(*chunk_pos),
                TerrainEvent::ChunkUnloaded(chunk_pos) => self.chunk_unloaded(*chunk_pos),
                TerrainEvent::BlockModified(chunk_pos, local_block_pos) => {
                    self.chunk_modified(chunk_pos, local_block_pos)
                }
            }
        }

        // check for newly finished meshes
        while let Ok(received) = self.finished_mesh_rx.try_recv() {
            let (chunk_pos, mesh_data) = received;
            self.finished_meshing_chunk(
                cx,
                terrain
                    .load_areas()
                    .get(load_area_index)
                    .expect("the load area at `load_area_index` should exist"),
                chunk_pos,
                mesh_data,
            );
        }

        // remove any empty groups
        let batches_to_remove = self
            .batch_positions
            .iter()
            .copied()
            .filter(|group_pos| self.batches[group_pos].is_empty())
            .collect_vec();
        batches_to_remove
            .iter()
            .copied()
            .for_each(|group_pos| self.remove_batch(group_pos));

        // update the vertex buffers of any dirty groups, and grow the shared index buffer if needed
        let mut highest_vertex_count = self.shared_index_buffer_vertex_count;
        for group_pos in &self.batches_requiring_mesh_updates {
            if let Some(batch) = self.batches.get_mut(&group_pos) {
                batch.update_vertex_buffer(&cx.device, &cx.queue);
                highest_vertex_count = highest_vertex_count.max(batch.vertex_count());
            }
        }
        if highest_vertex_count > self.shared_index_buffer_vertex_count {
            // grow shared index buffer
            let shared_indices = meshing::generate_indices(highest_vertex_count);
            self.shared_index_buffer =
                cx.device
                    .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("Chunk Renderer Shared Index Buffer"),
                        contents: bytemuck::cast_slice(&shared_indices),
                        usage: wgpu::BufferUsages::INDEX,
                    });
            self.shared_index_buffer_vertex_count = highest_vertex_count;
        }

        self.batches_requiring_mesh_updates
            .clear();
    }

    /// Called when a chunk has been loaded to note that its neighbours' meshes can be optimized
    /// NB: this does not queue the chunk for mesh generation: its mesh will only be generated
    /// after it is requested
    fn chunk_loaded(&mut self, chunk_pos: ChunkPos) {
        let neighbour_positions = [
            chunk_pos + ChunkPos::new(1, 0, 0),
            chunk_pos + ChunkPos::new(0, 1, 0),
            chunk_pos + ChunkPos::new(0, 0, 1),
            chunk_pos + ChunkPos::new(-1, 0, 0),
            chunk_pos + ChunkPos::new(0, -1, 0),
            chunk_pos + ChunkPos::new(0, 0, -1),
        ];
        for neighbour_pos in neighbour_positions {
            let (group_pos, chunk_pos_in_group) =
                Self::get_group_pos_and_chunk_pos_in_batch(neighbour_pos);

            if let Some(group) = self.batches.get_mut(&group_pos) {
                group.mark_suboptimal(chunk_pos_in_group);
            }
        }
    }

    /// Called when a chunk has been unloaded to remove its mesh from the batch containing
    /// it
    fn chunk_unloaded(&mut self, chunk_pos: ChunkPos) {
        let (group_pos, chunk_pos_in_group) = Self::get_group_pos_and_chunk_pos_in_batch(chunk_pos);

        if let Some(group) = self.batches.get_mut(&group_pos) {
            group.clear_mesh_data_for_chunk(chunk_pos_in_group);
            self.batches_requiring_mesh_updates
                .push(group_pos);
        }
    }

    /// Called when a block in a chunk has been modified
    fn chunk_modified(&mut self, chunk_pos: &ChunkPos, block_pos: &LocalBlockPos) {
        let (group_pos, chunk_pos_in_group) =
            Self::get_group_pos_and_chunk_pos_in_batch(*chunk_pos);

        if let Some(group) = self.batches.get_mut(&group_pos) {
            group.mark_outdated(chunk_pos_in_group);
        }
    }

    /// Queue any necessary mesh updates for this chunk
    fn request_mesh_updates_for_chunk(
        &mut self,
        chunk: &Chunk,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        camera_pos: Vec3,
    ) {
        if chunk.is_empty() {
            return;
        }

        let (group_pos, chunk_pos_in_group) =
            Self::get_group_pos_and_chunk_pos_in_batch(chunk.pos());

        if let Some(group) = self.batches.get_mut(&group_pos) {
            match group.get_status_for_chunk(chunk_pos_in_group) {
                ChunkMeshStatus::Good | ChunkMeshStatus::Generating => (),
                ChunkMeshStatus::Missing => {
                    group.mark_generating(chunk_pos_in_group);
                    self.queue_chunk_for_meshing(
                        chunk,
                        tasks,
                        terrain,
                        load_area_index,
                        camera_pos,
                        CHUNK_MESH_GENERATION_PRIORITY,
                    );
                }
                ChunkMeshStatus::Outdated => {
                    group.mark_generating(chunk_pos_in_group);
                    self.queue_chunk_for_meshing(
                        chunk,
                        tasks,
                        terrain,
                        load_area_index,
                        camera_pos,
                        CHUNK_MESH_UPDATE_PRIORITY,
                    );
                }
                ChunkMeshStatus::Suboptimal => {
                    group.mark_generating(chunk_pos_in_group);
                    self.queue_chunk_for_meshing(
                        chunk,
                        tasks,
                        terrain,
                        load_area_index,
                        camera_pos,
                        CHUNK_MESH_OPTIMIZATION_PRIORITY,
                    );
                }
            }
        } else {
            // there isn't a batch at this chunk's position yet, but it will be created
            // once the chunk's mesh is finished
            if !self
                .meshing_tasks
                .contains_key(&chunk.pos())
            {
                self.queue_chunk_for_meshing(
                    chunk,
                    tasks,
                    terrain,
                    load_area_index,
                    camera_pos,
                    CHUNK_MESH_GENERATION_PRIORITY,
                );
            }
        }
    }

    /// Spawn a new task to generate a chunk's mesh
    /// If this function is called multiple times for the same chunk before the mesh generation
    /// finishes, the mesh from the latest call is used
    fn queue_chunk_for_meshing(
        &mut self,
        chunk: &Chunk,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        camera_pos: Vec3,
        priority: i32,
    ) {
        // if we already issued a task to generate this mesh, cancel it
        if let Some(existing_task_id) = self
            .meshing_tasks
            .get(&chunk.pos())
            .copied()
        {
            self.meshing_tasks.remove(&chunk.pos());
            tasks.cancel_if_pending(existing_task_id);
        }

        // instant that `call_chunk_for_meshing` was called
        let queued_instant = Instant::now();

        // clone the sender for the worker thread to use
        let finished_mesh_tx = self.finished_mesh_tx.clone();

        // prepare a snapshot of data about the chunk for the meshing thread to use
        let chunk_pos = chunk.pos();
        let blocks = chunk.as_block_array();
        let surrounding_sides =
            Self::get_surrounding_chunk_sides(chunk_pos, terrain, load_area_index);

        // assign a higher priority to chunks closer to the camera
        let priority_within_class = (chunk_pos.as_vec3() - camera_pos).length_squared() as i32;

        let task_id = tasks.submit(
            TaskPriority {
                class_priority: priority,
                priority_within_class,
            },
            move || {
                // move `blocks` and `surrounding sides` to the new thread
                let (blocks, surrounding_sides) = (blocks, surrounding_sides);

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

        self.meshing_tasks
            .insert(chunk_pos, task_id);
    }

    /// Called whenever a finished chunk mesh arrives
    fn finished_meshing_chunk(
        &mut self,
        cx: &RenderContext,
        loaded_area: &LoadArea,
        chunk_pos: ChunkPos,
        mesh_data: ChunkMeshData,
    ) {
        self.meshing_tasks.remove(&chunk_pos);

        // make sure that the chunk is still loaded
        if !loaded_area.is_loaded(&chunk_pos) {
            return;
        }

        let (group_pos, chunk_pos_in_group) = Self::get_group_pos_and_chunk_pos_in_batch(chunk_pos);

        let group = self.get_or_add_batch(group_pos, &cx.device);

        if group.set_mesh_data_for_chunk(chunk_pos_in_group, mesh_data) {
            self.batches_requiring_mesh_updates
                .push(group_pos);
        }
    }

    /// Create a new batch and add it to the active groups
    /// Returns the new group
    fn add_new_batch(&mut self, group_pos: IVec3, device: &wgpu::Device) -> &mut ChunkBatch {
        debug_assert!(!self.batches.contains_key(&group_pos));
        debug_assert!(!self
            .batch_positions
            .contains(&group_pos));

        let mut group = ChunkBatch::new(group_pos, device, &self.batch_bind_group_layout);

        // inform the group about which chunks are already generating
        for (x, y, z) in itertools::iproduct!(
            (0..CHUNK_BATCH_SIZE as u32),
            (0..CHUNK_BATCH_SIZE as u32),
            (0..CHUNK_BATCH_SIZE as u32)
        ) {
            let chunk_pos_in_group = UVec3::new(x, y, z);
            let chunk_pos = ChunkPos::from(
                group_pos * (CHUNK_BATCH_SIZE as i32) + chunk_pos_in_group.as_ivec3(),
            );
            if self
                .meshing_tasks
                .contains_key(&chunk_pos)
            {
                group.mark_generating(chunk_pos_in_group);
            }
        }

        self.batch_positions.push(group_pos);
        self.batches.insert(group_pos, group);

        self.batches
            .get_mut(&group_pos)
            .expect("group should exist as it was just created")
    }

    /// Remove a batch from the active groups
    /// Panics if the group does not exist
    fn remove_batch(&mut self, group_pos: IVec3) {
        debug_assert!(self.batches.contains_key(&group_pos));
        debug_assert!(self
            .batch_positions
            .contains(&group_pos));

        self.batches.remove(&group_pos);
        self.batch_positions.remove(
            self.batch_positions
                .iter()
                .position(|x| *x == group_pos)
                .expect(
                    "position of chunk batch being removed should be in `active_group_positions`",
                ),
        );
    }

    /// Returns a mutable reference to the batch at the given position, creating it if it
    /// does not exist
    fn get_or_add_batch(&mut self, group_pos: IVec3, device: &wgpu::Device) -> &mut ChunkBatch {
        if self.batches.contains_key(&group_pos) {
            self.batches
                .get_mut(&group_pos)
                .unwrap()
        } else {
            self.add_new_batch(group_pos, device)
        }
    }

    /// Given a chunk position, returns the position of its batch in the grid and the
    /// position of the chunk in the batch
    fn get_group_pos_and_chunk_pos_in_batch(chunk_pos: ChunkPos) -> (IVec3, UVec3) {
        let chunk_pos = chunk_pos.as_ivec3();
        let group_pos = chunk_pos.div_euclid(IVec3::splat(CHUNK_BATCH_SIZE as i32));
        let chunk_pos_in_group = chunk_pos - group_pos * (CHUNK_BATCH_SIZE as i32);

        (group_pos, chunk_pos_in_group.as_uvec3())
    }

    /// Returns the sides of all chunks surrounding `chunk_pos`
    fn get_surrounding_chunk_sides(
        center_pos: ChunkPos,
        terrain: &Terrain,
        load_area_index: Index,
    ) -> Vec<Option<ChunkSide>> {
        let side_px = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPos::new(1, 0, 0)))
            .map(ChunkSide::nx);
        let side_py = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPos::new(0, 1, 0)))
            .map(ChunkSide::ny);
        let side_pz = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPos::new(0, 0, 1)))
            .map(ChunkSide::nz);
        let side_nx = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPos::new(-1, 0, 0)))
            .map(ChunkSide::px);
        let side_ny = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPos::new(0, -1, 0)))
            .map(ChunkSide::py);
        let side_nz = terrain
            .get_chunk(load_area_index, &(center_pos + ChunkPos::new(0, 0, -1)))
            .map(ChunkSide::pz);

        vec![side_px, side_py, side_pz, side_nx, side_ny, side_nz]
    }
}

#[derive(Clone, Copy, Debug, derive_more::IsVariant)]
enum ChunkMeshStatus {
    Good,
    Generating,
    Missing,
    Suboptimal,
    Outdated,
}

#[derive(Debug)]
struct ChunkMeshData {
    pub vertices: Vec<ChunkVertex>,
    pub queued_instant: Instant,
}

pub enum TerrainCullMode {
    CullNone,
    Frustum,
    VisibilitySearch,
}
