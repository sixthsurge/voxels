use std::sync::Arc;

use pollster::FutureExt;
use winit::{dpi::PhysicalSize, window::Window};

#[derive(Debug)]
pub struct WgpuContext {
    pub window_size: PhysicalSize<u32>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
}

impl WgpuContext {
    pub fn new(window: Arc<Window>) -> Self {
        let window_size = window.inner_size();

        let (device, queue, surface, surface_config) = init_wgpu(window);

        Self {
            window_size,
            device,
            queue,
            surface,
            surface_config,
        }
    }

    pub fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.window_size = new_size;
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
}

/// Create the core wgpu resources: device, queue, surface and surface configuration
fn init_wgpu(
    window: Arc<Window>,
) -> (
    wgpu::Device,
    wgpu::Queue,
    wgpu::Surface<'static>,
    wgpu::SurfaceConfiguration,
) {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..Default::default()
    });

    let surface = instance
        .create_surface(window.clone())
        .expect("failed to create surface");

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::LowPower,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        })
        .block_on()
        .expect("failed to create adapter");

    let (device, queue) = adapter
        .request_device(
            &wgpu::DeviceDescriptor {
                required_features: wgpu::Features::POLYGON_MODE_LINE,
                required_limits: wgpu::Limits::default(),
                memory_hints: wgpu::MemoryHints::Performance,
                label: None,
            },
            None,
        )
        .block_on()
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
        width: window.inner_size().width,
        height: window.inner_size().height,
        present_mode: wgpu::PresentMode::AutoNoVsync,
        alpha_mode: wgpu::CompositeAlphaMode::Auto,
        view_formats: vec![],
        desired_maximum_frame_latency: 2,
    };

    surface.configure(&device, &surface_config);
    surface.configure(&device, &surface_config);

    (device, queue, surface, surface_config)
}
