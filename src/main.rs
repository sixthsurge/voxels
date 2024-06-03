mod block;
mod camera;
mod fly_camera;
mod input;
mod render;
mod time;
mod transform;
mod world;
mod world_renderer;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use block::BlockRegistry;
use camera::{Camera, Projection};
use fly_camera::FlyCamera;
use input::InputState;
use render::WgpuState;
use time::{TargetFrameRate, TimeState};
use transform::Transform;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};
use world::World;
use world_renderer::WorldRenderer;

const WINDOW_TITLE: &'static str = "\"minecraft\"";

/// Size of one degree in radians
const DEGREE: f32 = 180.0 / std::f32::consts::PI;

struct State {
    window: Arc<Window>,
    input: InputState,
    wgpu: WgpuState,
    time: TimeState,
    camera: Camera,
    block_registry: BlockRegistry,
    world: World,
    world_renderer: WorldRenderer,
    fly_camera: FlyCamera,
    fly_camera_active: bool,
    close_requested: bool,
}

impl State {
    fn new(window: Arc<Window>) -> Self {
        let input = InputState::new();
        let wgpu = WgpuState::new(window.clone());
        let time = TimeState::new(TargetFrameRate::UnlimitedOrVsync);
        let camera = Camera::new(
            Transform::IDENTITY,
            Projection::Perspective {
                aspect_ratio: window.inner_size().width as f32 / window.inner_size().height as f32,
                fov_y_radians: 70.0 * DEGREE,
                z_near: 0.01,
                z_far: 1000.0,
            },
        );
        let mut block_registry = BlockRegistry::new();
        block_registry.add_all_in_dir(Path::new("res/data/blocks"));

        let world = World {};
        let world_renderer = WorldRenderer::new(&wgpu.device, wgpu.surface_config.format);

        Self {
            window,
            input,
            wgpu,
            time,
            camera,
            block_registry,
            world,
            world_renderer,
            fly_camera: FlyCamera::default(),
            fly_camera_active: true,
            close_requested: false,
        }
    }

    fn on_frame(&mut self) {
        self.time.begin_frame();
        self.update();
        self.render();
        self.time.update_frame_count();
        self.window.set_title(&format!(
            "{} ({} fps)",
            WINDOW_TITLE,
            self.time.get_frames_last_second()
        ));
        self.time.wait_for_next_frame();
    }

    fn on_resize(&mut self, new_size: PhysicalSize<u32>) {
        self.wgpu.on_resize(new_size);
        self.camera.on_resize(new_size);
    }

    fn update(&mut self) {
        if self.fly_camera_active {
            self.fly_camera.update(&self.input, &self.time);
        }

        self.input.reset();
    }

    fn render(&mut self) {
        self.camera.transform = self.fly_camera.get_transform();
        self.world_renderer.set_camera(&self.camera);

        let Some(surface_texture) = self.wgpu.get_surface_texture() else {
            log::warn!("Couldn't acquire surface texture");
            return;
        };

        let surface_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.world_renderer
            .render(&self.wgpu, &surface_texture_view);

        surface_texture.present();
    }
}

struct WinitApplicationHandler {
    state: Option<State>,
}

impl WinitApplicationHandler {
    fn new() -> Self {
        Self { state: None }
    }
}

impl ApplicationHandler<()> for WinitApplicationHandler {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            let window_attributes = Window::default_attributes().with_title(WINDOW_TITLE);
            let window = Arc::new(
                event_loop
                    .create_window(window_attributes)
                    .expect("failed to create window"),
            );

            self.state = Some(State::new(window));
        }
    }

    fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let state = self.state.as_mut().unwrap();

        match event {
            WindowEvent::CloseRequested => state.close_requested = true,
            WindowEvent::Resized(new_size) => state.on_resize(new_size),
            _ => {
                state.input.handle_window_event(&event);
            }
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        let state = self.state.as_mut().unwrap();
        state.input.handle_device_event(&event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        match self.state.as_mut() {
            Some(state) => {
                if state.close_requested {
                    event_loop.exit();
                }
                state.on_frame();
                event_loop.set_control_flow(ControlFlow::Poll);
            }
            None => (),
        }
    }
}

fn main() -> Result<(), EventLoopError> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info,wgpu=warn"))
        .init();
    EventLoop::new()?.run_app(&mut WinitApplicationHandler::new())
}
