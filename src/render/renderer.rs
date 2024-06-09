use super::{
    camera::Projection,
    chunk::{vertex::ChunkVertex, ChunkRenderGroups, RenderGroupCreateContext},
    util::texture::{ArrayTexture, DepthTexture, TextureConfig, TextureHolder, WithViewAndSampler},
};
use crate::{
    render::{
        camera::Camera,
        context::RenderContext,
        util::{
            bind_group_builder::BindGroupBuilder, mip_generator::MipGenerator,
            pipeline_builder::RenderPipelineBuilder,
        },
    },
    tasks::Tasks,
    terrain::{event::TerrainEvent, Terrain},
    util::transform::Transform,
    DEGREE,
};

pub struct Renderer {
    camera: Camera,
    chunk_render_groups: ChunkRenderGroups,
    depth_texture: WithViewAndSampler<DepthTexture>,
    terrain_textures: WithViewAndSampler<ArrayTexture>,
    terrain_pipeline: wgpu::RenderPipeline,
    texture_array_bind_group: wgpu::BindGroup,
    global_uniforms: GlobalUniforms,
    global_uniforms_buffer: wgpu::Buffer,
    global_uniforms_bind_group: wgpu::BindGroup,
    render_group_bind_group_layout: wgpu::BindGroupLayout,
}

impl Renderer {
    const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    const MIP_LEVEL_COUNT: u32 = 4;

    pub fn new(cx: &RenderContext) -> Self {
        let camera = Camera::new(
            Transform::IDENTITY,
            Projection::Perspective {
                aspect_ratio: cx.window_size.width as f32 / cx.window_size.height as f32,
                fov_y_radians: 70.0 * DEGREE,
                z_near: 0.01,
                z_far: 1000.0,
            },
        );

        let chunk_render_groups = ChunkRenderGroups::new();

        let depth_texture = DepthTexture::new(
            &cx.device,
            cx.window_size,
            Self::DEPTH_FORMAT,
            wgpu::CompareFunction::Less,
            Some("Depth Texture"),
        )
        .with_view_and_sampler(
            &cx.device,
            wgpu::SamplerDescriptor {
                label: Some("Depth Sampler"),
                compare: Some(wgpu::CompareFunction::Less),
                ..Default::default()
            },
        );

        let terrain_textures = ArrayTexture::from_files(
            &cx.device,
            &cx.queue,
            &[
                "assets/image/block/dirt.png",
                "assets/image/block/grass_side.png",
                "assets/image/block/grass_top.png",
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
        mip_generator.gen_mips(
            &mut mip_encoder,
            &cx.device,
            terrain_textures.texture(),
            terrain_textures.size().z,
            Self::MIP_LEVEL_COUNT,
        );
        cx.queue
            .submit(std::iter::once(mip_encoder.finish()));

        let terrain_shader = cx
            .device
            .create_shader_module(wgpu::include_wgsl!("shaders/terrain.wgsl"));

        let global_uniforms = GlobalUniforms::default();

        let global_uniforms_buffer = cx
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
                terrain_textures.view(),
                wgpu::TextureViewDimension::D2Array,
                wgpu::TextureSampleType::Float { filterable: true },
                wgpu::ShaderStages::FRAGMENT,
            )
            .with_sampler(
                terrain_textures.sampler(),
                wgpu::SamplerBindingType::Filtering,
                wgpu::ShaderStages::FRAGMENT,
            )
            .build(&cx.device);

        let (global_uniforms_bind_group, global_uniforms_bind_group_layout) =
            BindGroupBuilder::new()
                .with_uniform_buffer(&global_uniforms_buffer, wgpu::ShaderStages::all())
                .build(&cx.device);

        let render_group_bind_group_layout =
            cx.device
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
                cx.surface_config.format,
                Some(wgpu::BlendState::REPLACE),
                wgpu::ColorWrites::all(),
            )
            .with_depth(Self::DEPTH_FORMAT, wgpu::CompareFunction::Less)
            .build(&cx.device);

        Self {
            camera,
            chunk_render_groups,
            depth_texture,
            terrain_pipeline,
            terrain_textures,
            texture_array_bind_group,
            global_uniforms,
            global_uniforms_buffer,
            global_uniforms_bind_group,
            render_group_bind_group_layout,
        }
    }

    pub fn update(&mut self, tasks: &mut Tasks, cx: &RenderContext, terrain: &Terrain) {
        // update global uniforms
        self.global_uniforms.camera_view_matrix = self
            .camera
            .view_matrix()
            .to_cols_array();
        self.global_uniforms
            .camera_projection_matrix = self
            .camera
            .projection_matrix()
            .to_cols_array();

        for event in terrain.events() {
            match event {
                &TerrainEvent::ChunkLoaded(chunk_pos) => {
                    self.chunk_render_groups.chunk_loaded(
                        chunk_pos,
                        tasks,
                        terrain,
                        self.camera.transform.translation,
                    );
                }
                &TerrainEvent::ChunkUnloaded(chunk_pos) => {
                    self.chunk_render_groups
                        .chunk_unloaded(chunk_pos);
                }
            }
        }

        self.chunk_render_groups.update(
            &cx.device,
            terrain,
            RenderGroupCreateContext {
                device: &cx.device,
                bind_group_layout: &self.render_group_bind_group_layout,
            },
        );
    }

    pub fn render(&mut self, cx: &RenderContext, surface_texture_view: &wgpu::TextureView) {
        cx.queue.write_buffer(
            &self.global_uniforms_buffer,
            0 as wgpu::BufferAddress,
            bytemuck::cast_slice(&[self.global_uniforms]),
        );

        let mut encoder = cx
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

        for chunk_render_group in self.chunk_render_groups.iter() {
            if let Some(mesh) = chunk_render_group.mesh() {
                render_pass.set_bind_group(2, chunk_render_group.bind_group(), &[]);
                render_pass.set_vertex_buffer(0, mesh.vertex_buffer().slice(..));
                render_pass.set_index_buffer(mesh.index_buffer().slice(..), mesh.index_format());
                render_pass.draw_indexed(0..mesh.index_count(), 0, 0..1);
            }
        }

        drop(render_pass);

        let command_buffer = encoder.finish();

        cx.queue
            .submit(std::iter::once(command_buffer));
    }

    pub fn resized(&mut self, cx: &RenderContext) {
        // recreate depth texture
        let new_depth_texture = self
            .depth_texture
            .recreate(&cx.device, cx.window_size)
            .with_view_and_sampler(
                &cx.device,
                self.depth_texture
                    .sampler_descriptor()
                    .clone(),
            );
        self.depth_texture = new_depth_texture;

        // update camera projection
        self.camera.resized(cx.window_size);
    }

    /// Returns a shared reference to the camera used to render the world
    pub fn camera(&self) -> &Camera {
        &self.camera
    }

    /// Returns a mutable reference to the camera used to render the world
    pub fn camera_mut(&mut self) -> &mut Camera {
        &mut self.camera
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlobalUniforms {
    pub camera_view_matrix: [f32; 16],
    pub camera_projection_matrix: [f32; 16],
}
