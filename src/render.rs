use glam::Vec4Swizzles;

use self::{
    camera::{Camera, Projection},
    context::RenderContext,
    terrain::TerrainRenderer,
    util::bind_group_builder::BindGroupBuilder,
    util::texture::{DepthTexture, TextureHolder, WithViewAndSampler},
};
use crate::{
    tasks::Tasks,
    terrain::Terrain,
    util::{transform::Transform, DEGREE},
};

pub mod camera;
pub mod context;
pub mod terrain;
pub mod util;

const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::Less;

pub struct RenderEngine {
    depth_texture: WithViewAndSampler<DepthTexture>,
    common_uniforms: CommonUniforms,
    common_uniforms_buffer: wgpu::Buffer,
    common_uniforms_bind_group: wgpu::BindGroup,
    terrain_renderer: TerrainRenderer,
    camera: Camera,
}

impl RenderEngine {
    pub fn new(cx: &RenderContext) -> Self {
        let depth_texture = DepthTexture::new(
            &cx.device,
            cx.window_size,
            DEPTH_FORMAT,
            DEPTH_COMPARE,
            Some("Depth Texture"),
        )
        .with_view_and_sampler(
            &cx.device,
            wgpu::SamplerDescriptor {
                label: None,
                compare: Some(wgpu::CompareFunction::Less),
                ..Default::default()
            },
        );

        let common_uniforms = CommonUniforms::default();

        let common_uniforms_buffer = cx
            .device
            .create_buffer(&wgpu::BufferDescriptor {
                label: Some("Global Uniform Buffer"),
                size: std::mem::size_of::<CommonUniforms>() as wgpu::BufferAddress,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });

        let (common_uniforms_bind_group, common_uniforms_bind_group_layout) =
            BindGroupBuilder::new()
                .with_uniform_buffer(&common_uniforms_buffer, wgpu::ShaderStages::all())
                .build(&cx.device);

        let terrain_renderer = TerrainRenderer::new(cx, &common_uniforms_bind_group_layout);

        let camera = Camera::new(
            Transform::IDENTITY,
            Projection::Perspective {
                aspect_ratio: cx.window_size.width as f32 / cx.window_size.height as f32,
                fov_y_radians: 70.0 * DEGREE,
                z_near: 0.01,
                z_far: 1000.0,
            },
        );

        Self {
            depth_texture,
            common_uniforms,
            common_uniforms_buffer,
            common_uniforms_bind_group,
            terrain_renderer,
            camera,
        }
    }

    pub fn render(
        &mut self,
        cx: &RenderContext,
        output_view: &wgpu::TextureView,
        tasks: &mut Tasks,
        terrain: &Terrain,
        use_cave_culling: bool,
    ) {
        // update common uniforms
        self.common_uniforms.camera_view_matrix = self
            .camera
            .view_matrix()
            .to_cols_array();
        self.common_uniforms
            .camera_projection_matrix = self
            .camera
            .projection_matrix()
            .to_cols_array();

        cx.queue.write_buffer(
            &self.common_uniforms_buffer,
            0 as wgpu::BufferAddress,
            bytemuck::cast_slice(&[self.common_uniforms]),
        );

        let mut render_encoder =
            cx.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.terrain_renderer.render(
            &mut render_encoder,
            output_view,
            &self.depth_texture.view(),
            &self.common_uniforms_bind_group,
            cx,
            tasks,
            terrain,
            self.camera.pos(),
            self.camera.look_dir(),
            use_cave_culling,
        );

        let command_buffer = render_encoder.finish();

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
pub struct CommonUniforms {
    pub camera_view_matrix: [f32; 16],
    pub camera_projection_matrix: [f32; 16],
}
