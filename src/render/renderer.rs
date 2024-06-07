use std::collections::HashMap;

use glam::{IVec3, UVec2};

use crate::{
    render::{
        camera::Camera,
        chunk::vertex::ChunkVertex,
        context::RenderContext,
        util::{
            bind_group_builder::BindGroupBuilder, mip_generator::MipGenerator,
            pipeline_builder::RenderPipelineBuilder, texture::Texture,
        },
    },
    terrain::{
        self,
        chunk::{CHUNK_SIZE, CHUNK_SIZE_I32},
        event::TerrainEvent,
        position_types::ChunkPos,
        Terrain,
    },
};

use super::chunk::{
    meshing::{mesh_greedy, ChunkMeshInput},
    render_group::{self, ChunkRenderGroup, CHUNK_RENDER_GROUP_SIZE},
};

pub struct Renderer {
    depth_texture: Texture,
    terrain_pipeline: wgpu::RenderPipeline,
    texture_array: Texture,
    texture_array_bind_group: wgpu::BindGroup,
    global_uniforms: GlobalUniforms,
    global_uniforms_buffer: wgpu::Buffer,
    global_uniforms_bind_group: wgpu::BindGroup,
    render_group_bind_group_layout: wgpu::BindGroupLayout,
    chunk_render_groups: Vec<ChunkRenderGroup>,
}

impl Renderer {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    const MIP_LEVEL_COUNT: u32 = 4;

    pub fn new(render_context: &RenderContext, window_size: UVec2) -> Self {
        let depth_texture = Texture::new_depth_texture(
            &render_context.device,
            window_size,
            wgpu::TextureFormat::Depth32Float,
            Some("Depth Texture"),
        );

        let texture_array = Texture::load_array(
            &render_context.device,
            &render_context.queue,
            &["assets/test.png", "assets/sad.png"],
            UVec2::splat(16),
            Self::MIP_LEVEL_COUNT,
            wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            wgpu::AddressMode::Repeat,
            wgpu::FilterMode::Nearest,
            wgpu::FilterMode::Nearest,
            wgpu::FilterMode::Linear,
            Some("Terrain Texture Array"),
        )
        .unwrap();

        // generate mipmaps
        let mut mipmap_encoder = render_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        let mipmap_generator =
            MipGenerator::new(&render_context.device, wgpu::TextureFormat::Rgba8UnormSrgb);
        mipmap_generator.gen_mips(
            &mut mipmap_encoder,
            &render_context.device,
            texture_array.texture(),
            texture_array.size().z,
            Self::MIP_LEVEL_COUNT,
        );
        render_context
            .queue
            .submit(std::iter::once(mipmap_encoder.finish()));

        let terrain_shader = render_context
            .device
            .create_shader_module(wgpu::include_wgsl!("shaders/terrain.wgsl"));

        let global_uniforms = GlobalUniforms::default();

        let global_uniforms_buffer = render_context
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("Global Uniform Buffer"),
                size: std::mem::size_of::<GlobalUniforms>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        let (texture_array_bind_group, texture_array_bind_group_layout) = BindGroupBuilder::new()
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
            .build(&render_context.device);

        let (global_uniforms_bind_group, global_uniforms_bind_group_layout) =
            BindGroupBuilder::new()
                .with_uniform_buffer(&global_uniforms_buffer, wgpu::ShaderStages::all())
                .build(&render_context.device);

        let render_group_bind_group_layout = render_context
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: None,
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::all(),
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let (terrain_pipeline, _) = RenderPipelineBuilder::new()
            .with_label("Terrain Pipeline")
            .with_bind_group_layout(&texture_array_bind_group_layout)
            .with_bind_group_layout(&global_uniforms_bind_group_layout)
            .with_bind_group_layout(&render_group_bind_group_layout)
            .with_vertex::<ChunkVertex>()
            .with_vertex_shader(&terrain_shader, "vs_main")
            .with_fragment_shader(&terrain_shader, "fs_main")
            .with_color_target(
                render_context.surface_config.format,
                Some(wgpu::BlendState::REPLACE),
                wgpu::ColorWrites::all(),
            )
            .with_depth(Self::DEPTH_FORMAT, wgpu::CompareFunction::Less)
            .build(&render_context.device);

        let chunk_render_groups = Vec::new();

        Self {
            depth_texture,
            terrain_pipeline,
            texture_array,
            texture_array_bind_group,
            global_uniforms,
            global_uniforms_buffer,
            global_uniforms_bind_group,
            render_group_bind_group_layout,
            chunk_render_groups,
        }
    }

    pub fn update(&mut self, render_context: &RenderContext, terrain: &Terrain) {
        let terrain_events = terrain.events();
        for event in terrain_events {
            match event {
                TerrainEvent::ChunkLoaded(chunk_pos) => {
                    let chunk = terrain.get_chunk(chunk_pos).unwrap();
                    let blocks = chunk.as_block_array();
                    let chunk_pos_in_group = chunk_pos
                        .as_ivec3()
                        .rem_euclid(IVec3::splat(CHUNK_RENDER_GROUP_SIZE as i32));
                    let mesh_input = ChunkMeshInput {
                        blocks: &blocks,
                        translation: (chunk_pos_in_group * CHUNK_SIZE_I32).as_vec3(),
                    };
                    let vertices = mesh_greedy(mesh_input);
                    let chunk_render_group =
                        self.get_render_group_for_chunk(*chunk_pos, &render_context.device);
                    chunk_render_group
                        .set_vertices_for_chunk(chunk_pos_in_group.as_uvec3(), vertices);
                    chunk_render_group.update_mesh(&render_context.device);
                }
                _ => (),
            }
        }
    }

    pub fn render(
        &mut self,
        render_context: &RenderContext,
        surface_texture_view: &wgpu::TextureView,
    ) {
        render_context.queue.write_buffer(
            &self.global_uniforms_buffer,
            0 as wgpu::BufferAddress,
            bytemuck::cast_slice(&[self.global_uniforms]),
        );

        let mut encoder = render_context
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("World Render Encoder"),
            });

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Terrain Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &surface_texture_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &self.depth_texture.view(),
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        // draw chunk render groups

        render_pass.set_pipeline(&self.terrain_pipeline);
        render_pass.set_bind_group(0, &self.texture_array_bind_group, &[]);
        render_pass.set_bind_group(1, &self.global_uniforms_bind_group, &[]);

        for chunk_render_group in &self.chunk_render_groups {
            if let Some(mesh) = chunk_render_group.mesh() {
                render_pass.set_bind_group(2, chunk_render_group.uniforms_bind_group(), &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass.set_index_buffer(mesh.index_buffer().slice(..), mesh.index_format());
                render_pass.draw_indexed(0..mesh.index_count(), 0, 0..1);
            }
        }

        drop(render_pass);

        let command_buffer = encoder.finish();

        render_context
            .queue
            .submit(std::iter::once(command_buffer));
    }

    pub fn resized(&mut self, render_context: &RenderContext, window_size: UVec2) {
        let depth_texture = Texture::new_depth_texture(
            &render_context.device,
            window_size,
            wgpu::TextureFormat::Depth32Float,
            Some("Depth Texture"),
        );
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.global_uniforms.camera_view_matrix = camera.view_matrix().to_cols_array();
        self.global_uniforms
            .camera_projection_matrix = camera
            .projection_matrix()
            .to_cols_array();
    }

    /// given a chunk position, returns a mutating reference to render group for that chunk
    /// if the render group does not exist, it will be created
    fn get_render_group_for_chunk(
        &mut self,
        chunk_pos: ChunkPos,
        device: &wgpu::Device,
    ) -> &mut ChunkRenderGroup {
        let render_group_pos = chunk_pos
            .as_ivec3()
            .div_euclid(IVec3::splat(CHUNK_RENDER_GROUP_SIZE as i32));

        let group_index = self
            .chunk_render_groups
            .iter_mut()
            .position(|g| g.pos() == render_group_pos);

        if let Some(group_index) = group_index {
            &mut self.chunk_render_groups[group_index]
        } else {
            self.chunk_render_groups
                .push(ChunkRenderGroup::new(
                    render_group_pos,
                    &device,
                    &self.render_group_bind_group_layout,
                ));
            self.chunk_render_groups
                .last_mut()
                .unwrap()
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub camera_view_matrix: [f32; 16],
    pub camera_projection_matrix: [f32; 16],
}
