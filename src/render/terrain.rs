use std::time::Instant;

use generational_arena::Index;
use glam::Vec3;
use itertools::Itertools;

use self::{
    chunk_batching::ChunkBatches, vertex::TerrainVertex, visibility_search::visibility_search,
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
    tasks::{TaskId, Tasks},
    terrain::{
        chunk::Chunk,
        event::TerrainEvent,
        load_area::LoadArea,
        position_types::{ChunkPosition, LocalBlockPosition},
        Terrain,
    },
    time::Time,
    CHUNK_MESH_GENERATION_PRIORITY, CHUNK_MESH_OPTIMIZATION_PRIORITY, CHUNK_MESH_UPDATE_PRIORITY,
};

mod chunk_batching;
mod meshing;
mod vertex;
mod visibility_search;

/// Responsible for rendering the voxel terrain
#[derive(Debug)]
pub struct TerrainRenderer {
    /// Responsible for managing chunk batches
    chunk_batches: ChunkBatches,
    /// Frame index when each chunk batch was last rendered, to prevent them from being rendered
    /// multiple times per frame
    frame_last_drawn: Vec<usize>,
    /// Cull mode to use
    cull_mode: TerrainCullMode,
    /// Render pipeline for drawing chunk batches
    terrain_pipeline: wgpu::RenderPipeline,
    /// Bind group for the texture array
    texture_bind_group: wgpu::BindGroup,
}

impl TerrainRenderer {
    pub const MIP_LEVEL_COUNT: u32 = 4;

    pub fn new(
        cx: &RenderContext,
        common_uniforms_bind_group_layout: &wgpu::BindGroupLayout,
        load_area: &LoadArea,
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
                "assets/image/block/lamp_orange.png",
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
            .with_vertex::<TerrainVertex>()
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

        let chunk_batches = ChunkBatches::new(cx, load_area, batch_bind_group_layout);

        let frame_last_drawn = vec![0; chunk_batches.size().product()];

        Self {
            chunk_batches,
            frame_last_drawn,
            cull_mode,
            terrain_pipeline,
            texture_bind_group,
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
        time: &Time,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        frustum_culling_regions: &FrustumCullingRegions,
        camera_pos: Vec3,
    ) {
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

        // update chunk batches
        self.chunk_batches
            .update(cx, terrain, load_area_index);

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
                .filter(|chunk| frustum_culling_regions.is_chunk_within_frustum(&chunk.position()))
                .collect_vec(),
            TerrainCullMode::VisibilitySearch => visibility_search(
                terrain,
                load_area_index,
                frustum_culling_regions,
                camera_pos,
            ),
        };

        // request mesh updates for visible chunks
        for chunk in &render_queue {
            self.request_mesh_updates_for_chunk(
                cx,
                chunk,
                tasks,
                terrain,
                load_area_index,
                camera_pos,
            );
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
        render_pass.set_index_buffer(
            self.chunk_batches
                .shared_index_buffer()
                .slice(..),
            wgpu::IndexFormat::Uint32,
        );

        for chunk in &render_queue {
            let (batch_pos, _) =
                ChunkBatches::get_batch_pos_and_chunk_pos_in_batch(&chunk.position());
            let batch_index = self
                .chunk_batches
                .get_batch_index(&batch_pos);

            if self.frame_last_drawn[batch_index] == time.frame_index() {
                // don't draw the same chunk batch twice
                continue;
            }
            let Some(batch) = self.chunk_batches.get_batch(&batch_pos) else {
                continue;
            };
            if batch.vertex_count() == 0 {
                continue;
            }
            let Some(vertex_buffer) = batch.vertex_buffer() else {
                continue;
            };

            self.frame_last_drawn[batch_index] = time.frame_index();

            render_pass.set_bind_group(2, batch.uniform_bind_group(), &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw_indexed(0..(batch.index_count() as u32), 0, 0..1);
        }
    }

    /// Request any necessary mesh updates for the given chunk
    pub fn request_mesh_updates_for_chunk(
        &mut self,
        cx: &RenderContext,
        chunk: &Chunk,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
        camera_pos: Vec3,
    ) {
        let (batch_pos, chunk_pos_in_batch) =
            ChunkBatches::get_batch_pos_and_chunk_pos_in_batch(&chunk.position());

        let batch = self
            .chunk_batches
            .get_or_repurpose_batch(cx, tasks, &batch_pos);

        let remeshing_priority = match batch.get_chunk_mesh_status(&chunk_pos_in_batch) {
            ChunkMeshStatus::Good | ChunkMeshStatus::Generating(_) => None,
            ChunkMeshStatus::Missing => Some(CHUNK_MESH_GENERATION_PRIORITY),
            ChunkMeshStatus::Outdated => Some(CHUNK_MESH_UPDATE_PRIORITY),
            ChunkMeshStatus::Suboptimal => Some(CHUNK_MESH_OPTIMIZATION_PRIORITY),
        };

        if let Some(remeshing_priority) = remeshing_priority {
            self.chunk_batches
                .queue_chunk_for_meshing(
                    chunk,
                    tasks,
                    terrain,
                    load_area_index,
                    camera_pos,
                    remeshing_priority,
                )
        }
    }

    /// Called when a chunk has been loaded to note that its neighbours' meshes can be optimized
    /// NB: this does not queue the chunk for mesh generation: its mesh will only be generated
    /// after it is requested
    fn chunk_loaded(&mut self, chunk_pos: ChunkPosition) {
        let neighbour_positions = [
            chunk_pos + ChunkPosition::new(1, 0, 0),
            chunk_pos + ChunkPosition::new(0, 1, 0),
            chunk_pos + ChunkPosition::new(0, 0, 1),
            chunk_pos + ChunkPosition::new(-1, 0, 0),
            chunk_pos + ChunkPosition::new(0, -1, 0),
            chunk_pos + ChunkPosition::new(0, 0, -1),
        ];
        for neighbour_pos in neighbour_positions {
            let (batch_pos, chunk_pos_in_batch) =
                ChunkBatches::get_batch_pos_and_chunk_pos_in_batch(&neighbour_pos);

            if let Some(batch) = self
                .chunk_batches
                .get_batch_mut(&batch_pos)
            {
                batch.mark_suboptimal(&chunk_pos_in_batch);
            }
        }
    }

    /// Called when a chunk has been unloaded to remove its mesh from the batch containing
    /// it
    fn chunk_unloaded(&mut self, chunk_pos: ChunkPosition) {
        let (batch_pos, chunk_pos_in_batch) =
            ChunkBatches::get_batch_pos_and_chunk_pos_in_batch(&chunk_pos);

        if let Some(batch) = self
            .chunk_batches
            .get_batch_mut(&batch_pos)
        {
            batch.clear_mesh_data_for_chunk(&chunk_pos_in_batch);
        }
    }

    /// Called when a block in a chunk has been modified
    fn chunk_modified(&mut self, chunk_pos: &ChunkPosition, _block_pos: &LocalBlockPosition) {
        let (batch_pos, chunk_pos_in_batch) =
            ChunkBatches::get_batch_pos_and_chunk_pos_in_batch(chunk_pos);

        if let Some(batch) = self
            .chunk_batches
            .get_batch_mut(&batch_pos)
        {
            batch.mark_outdated(&chunk_pos_in_batch);
        }
    }
}

#[derive(Clone, Copy, Debug, derive_more::IsVariant)]
enum ChunkMeshStatus {
    Good,
    Generating(TaskId),
    Missing,
    Suboptimal,
    Outdated,
}

#[derive(Debug)]
struct ChunkMeshData {
    pub vertices: Vec<TerrainVertex>,
    pub queued_instant: Instant,
}

#[derive(Clone, Copy, Debug)]
pub enum TerrainCullMode {
    CullNone,
    Frustum,
    VisibilitySearch,
}
