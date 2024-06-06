use glam::UVec2;

use crate::render::{
    camera::Camera,
    chunk_mesh_gen::ChunkVertex,
    context::RenderContext,
    util::{bind_group_builder::BindGroupBuilder, mesh::Mesh, mesh::Vertex, texture::Texture},
};

use super::util::pipeline_builder::RenderPipelineBuilder;

pub struct RenderEngine {
    chunk_meshes: Vec<Mesh>,
    depth_texture: Texture,
    terrain_pipeline: wgpu::RenderPipeline,
    texture_array: Texture,
    texture_array_bind_group: wgpu::BindGroup,
    global_uniforms: GlobalUniforms,
    global_uniforms_buffer: wgpu::Buffer,
    global_uniforms_bind_group: wgpu::BindGroup,
}

impl RenderEngine {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;

    pub fn new(render_context: &RenderContext, window_size: UVec2) -> Self {
        let chunk_meshes = Vec::new();

        let depth_texture = Texture::new_depth_texture(
            &render_context.device,
            window_size,
            wgpu::TextureFormat::Depth32Float,
            Some("Depth Texture"),
        );

        let texture_array = Texture::load_array(
            &render_context.device,
            &render_context.queue,
            &["res/assets/test.png"],
            UVec2::splat(16),
            1,
            wgpu::AddressMode::Repeat,
            wgpu::FilterMode::Nearest,
            wgpu::FilterMode::Nearest,
            wgpu::FilterMode::Linear,
            Some("Terrain Texture Array"),
        )
        .unwrap();

        let terrain_shader = render_context
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("terrain.wgsl"),
                source: wgpu::ShaderSource::Wgsl(
                    std::fs::read_to_string("res/shaders/terrain.wgsl")
                        .unwrap()
                        .into(),
                ),
            });

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

        let (terrain_pipeline, _) = RenderPipelineBuilder::new()
            .with_label("Terrain Pipeline")
            .with_bind_group_layout(&texture_array_bind_group_layout)
            .with_bind_group_layout(&global_uniforms_bind_group_layout)
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

        Self {
            chunk_meshes,
            depth_texture,
            terrain_pipeline,
            texture_array,
            texture_array_bind_group,
            global_uniforms,
            global_uniforms_buffer,
            global_uniforms_bind_group,
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

        render_pass.set_pipeline(&self.terrain_pipeline);
        render_pass.set_bind_group(0, &self.texture_array_bind_group, &[]);
        render_pass.set_bind_group(1, &self.global_uniforms_bind_group, &[]);

        for mesh in &self.chunk_meshes {
            mesh.draw(&mut render_pass);
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

    pub fn add_chunk_mesh(&mut self, mesh: Mesh) {
        self.chunk_meshes.push(mesh);
    }

    pub fn set_camera(&mut self, camera: &Camera) {
        self.global_uniforms.camera_view_matrix = camera.view_matrix().to_cols_array_2d();
        self.global_uniforms
            .camera_projection_matrix = camera
            .projection_matrix()
            .to_cols_array_2d();
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub camera_view_matrix: [[f32; 4]; 4],
    pub camera_projection_matrix: [[f32; 4]; 4],
}
