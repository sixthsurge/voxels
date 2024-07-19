use generational_arena::Index;

use self::{
    camera::{Camera, Projection},
    frustum_culling::FrustumCullingRegions,
    terrain::{ChunkCullingMode, TerrainRenderer},
};
use crate::{
    core::{
        tasks::Tasks,
        time::Time,
        wgpu_util::{
            bind_group_builder::BindGroupBuilder,
            texture::{DepthTexture, TextureHolder, WithViewAndSampler},
            wgpu_context::WgpuContext,
        },
    },
    terrain::{load_area::LoadArea, Terrain},
    util::{size::Size3, transform::Transform, DEGREE},
};

pub mod camera;
pub mod frustum_culling;
pub mod terrain;

pub struct Renderer {
    depth_texture: WithViewAndSampler<DepthTexture>,
    common_uniforms: CommonUniforms,
    common_uniforms_buffer: wgpu::Buffer,
    common_uniforms_bind_group: wgpu::BindGroup,
    terrain_renderer: TerrainRenderer,
    camera: Camera,
    frustum_culling_regions: FrustumCullingRegions,
}

impl Renderer {
    pub const DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Depth32Float;
    pub const DEPTH_COMPARE: wgpu::CompareFunction = wgpu::CompareFunction::Less;
    pub const FRUSTUM_CULLING_REGION_SIZE_CHUNKS: usize = 8;

    pub fn new(wgpu: &WgpuContext, load_area: &LoadArea) -> Self {
        let depth_texture = DepthTexture::new(
            &wgpu.device,
            wgpu.window_size,
            Self::DEPTH_FORMAT,
            Self::DEPTH_COMPARE,
            Some("Depth Texture"),
        )
        .with_view_and_sampler(
            &wgpu.device,
            wgpu::SamplerDescriptor {
                label: None,
                compare: Some(wgpu::CompareFunction::Less),
                ..Default::default()
            },
        );

        let common_uniforms = CommonUniforms::default();

        let common_uniforms_buffer = wgpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Global Uniform Buffer"),
            size: std::mem::size_of::<CommonUniforms>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let (common_uniforms_bind_group, common_uniforms_bind_group_layout) =
            BindGroupBuilder::new()
                .with_uniform_buffer(&common_uniforms_buffer, wgpu::ShaderStages::all())
                .build(&wgpu.device);

        let terrain_renderer = TerrainRenderer::new(
            wgpu,
            &common_uniforms_bind_group_layout,
            load_area,
            ChunkCullingMode::VisibilitySearch,
        );

        let camera = Camera::new(
            Transform::IDENTITY,
            Projection::Perspective {
                aspect_ratio: wgpu.window_size.width as f32 / wgpu.window_size.height as f32,
                fov_y_radians: 80.0 * DEGREE,
                z_near: 0.01,
                z_far: 1000.0,
            },
        );

        let frustum_culling_region_size = Size3::splat(Self::FRUSTUM_CULLING_REGION_SIZE_CHUNKS);
        let frustum_culling_grid_size = load_area.size() / frustum_culling_region_size + Size3::ONE;
        let frustum_culling_regions =
            FrustumCullingRegions::new(frustum_culling_grid_size, frustum_culling_region_size);

        Self {
            depth_texture,
            common_uniforms,
            common_uniforms_buffer,
            common_uniforms_bind_group,
            terrain_renderer,
            camera,
            frustum_culling_regions,
        }
    }

    pub fn render(
        &mut self,
        wgpu: &WgpuContext,
        output_view: &wgpu::TextureView,
        time: &Time,
        tasks: &mut Tasks,
        terrain: &Terrain,
        load_area_index: Index,
    ) {
        let view_matrix = self.camera.view_matrix();
        let proj_matrix = self.camera.projection_matrix();
        let view_proj_matrix = proj_matrix * view_matrix;

        // update frustum culling regions
        self.frustum_culling_regions
            .update(&view_proj_matrix, self.camera.pos());

        // update common uniforms
        self.common_uniforms.camera_view_matrix = view_matrix.to_cols_array();
        self.common_uniforms.camera_proj_matrix = proj_matrix.to_cols_array();

        wgpu.queue.write_buffer(
            &self.common_uniforms_buffer,
            0 as wgpu::BufferAddress,
            bytemuck::cast_slice(&[self.common_uniforms]),
        );

        let mut render_encoder =
            wgpu.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("Render Encoder"),
                });

        self.terrain_renderer.render(
            &mut render_encoder,
            output_view,
            &self.depth_texture.view(),
            &self.common_uniforms_bind_group,
            wgpu,
            time,
            tasks,
            terrain,
            load_area_index,
            &self.frustum_culling_regions,
            self.camera.pos(),
        );

        let command_buffer = render_encoder.finish();

        wgpu.queue.submit(std::iter::once(command_buffer));
    }

    pub fn resized(&mut self, wgpu: &WgpuContext) {
        // recreate depth texture
        let new_depth_texture = self
            .depth_texture
            .recreate(&wgpu.device, wgpu.window_size)
            .with_view_and_sampler(
                &wgpu.device,
                self.depth_texture.sampler_descriptor().clone(),
            );
        self.depth_texture = new_depth_texture;

        // update camera projection
        self.camera.resized(wgpu.window_size);
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
    pub camera_proj_matrix: [f32; 16],
}
