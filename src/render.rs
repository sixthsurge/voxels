use std::sync::Arc;

use bytemuck::{Pod, Zeroable};
use pollster::FutureExt;
use wgpu::{util::DeviceExt, RenderPass, VertexBufferLayout};
use winit::{dpi::PhysicalSize, window::Window};

#[derive(Debug)]
pub struct WgpuState {
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
}

impl WgpuState {
    pub fn new(window: Arc<Window>) -> Self {
        Self::new_async(window).block_on()
    }

    pub fn on_resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface_config.width = new_size.width;
        self.surface_config.height = new_size.height;
        self.surface.configure(&self.device, &self.surface_config);
    }

    pub fn get_surface_texture(&mut self) -> Option<wgpu::SurfaceTexture> {
        match self.surface.get_current_texture() {
            Ok(tex) => Some(tex),
            // Reconfigure the surface if lost
            Err(wgpu::SurfaceError::Lost) => {
                self.surface.configure(&self.device, &self.surface_config);
                None
            }
            Err(wgpu::SurfaceError::OutOfMemory) => {
                log::error!("Surface error: out of memory");
                panic!()
            }
            Err(e) => {
                log::warn!("{:?}", e);
                None
            }
        }
    }

    async fn new_async(window: Arc<Window>) -> Self {
        let window_size = window.inner_size();

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..Default::default()
        });

        let surface = instance
            .create_surface(window.clone())
            .expect("failed to create surface");

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("failed to create adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    label: None,
                },
                None,
            )
            .await
            .expect("failed to create device");

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .filter(|format| format.is_srgb())
            .next()
            .unwrap_or_else(|| {
                log::warn!("non-sRGB surface format");
                surface_caps.formats[0]
            });

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: window_size.width,
            height: window_size.height,
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        Self {
            surface,
            surface_config,
            device,
            queue,
        }
    }
}

#[derive(Debug)]
pub struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_buffer_layout: wgpu::VertexBufferLayout<'static>,
    index_count: u32,
    index_format: wgpu::IndexFormat,
}

impl Mesh {
    pub fn new<V, I>(device: &wgpu::Device, vertices: &[V], indices: &[I]) -> Self
    where
        V: Vertex,
        I: Index,
    {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let vertex_buffer_layout = V::vertex_buffer_layout();
        let index_count = indices.len() as u32;
        let index_format = I::index_format();

        return Self {
            vertex_buffer,
            index_buffer,
            vertex_buffer_layout,
            index_count,
            index_format,
        };
    }

    pub fn draw<'mesh, 'render_pass>(&'mesh self, render_pass: &mut RenderPass<'render_pass>)
    where
        'mesh: 'render_pass,
    {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1)
    }

    pub fn vertex_buffer_layout(&self) -> VertexBufferLayout<'static> {
        self.vertex_buffer_layout.clone()
    }
}

pub trait Vertex: Pod + Zeroable {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static>;
}

pub trait Index: Pod + Zeroable {
    fn index_format() -> wgpu::IndexFormat;
}

impl Index for u16 {
    fn index_format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint16
    }
}

impl Index for u32 {
    fn index_format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint32
    }
}
