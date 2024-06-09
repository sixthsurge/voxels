use std::sync::Arc;

use fly_camera::FlyCamera;
use input::Input;
use render::{context::RenderContext, renderer::Renderer};
use tasks::Tasks;
use terrain::{Anchor, Terrain};
use time::{TargetFrameRate, Time};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Window, WindowId},
};

mod block;
mod fly_camera;
mod input;
mod render;
mod tasks;
mod terrain;
mod time;
mod util;

const WINDOW_TITLE: &'static str = "\"minecraft\"";

/// Number of threads to use for executing tasks
const TASKS_WORKER_THREAD_COUNT: usize = 8;

/// Priority of chunk loading tasks (lower is higher)
const CHUNK_LOADING_PRIORITY: i32 = 0;

/// Priority of chunk mesh generation tasks (lower is higher)
const CHUNK_MESH_GENERATION_PRIORITY: i32 = 1;

/// Priority of chunk mesh generation tasks when a fine mesh already exists (lower is higher)
const CHUNK_MESH_OPTIMIZATION_PRIORITY: i32 = 2;

/// Size of one degree in radians
const DEGREE: f32 = 180.0 / std::f32::consts::PI;

struct State {
    window: Arc<Window>,
    render_cx: RenderContext,
    time: Time,
    input: Input,
    tasks: Tasks,
    terrain: Terrain,
    renderer: Renderer,
    fly_camera: FlyCamera,
    fly_camera_active: bool,
    close_requested: bool,
}

impl State {
    fn new(window: Arc<Window>) -> Self {
        let window_size = window.inner_size();
        let render_cx = RenderContext::new(window.clone());
        let input = Input::new();
        let time = Time::new(TargetFrameRate::UnlimitedOrVsync);
        let tasks = Tasks::new(TASKS_WORKER_THREAD_COUNT);
        let terrain = Terrain::new();
        let renderer = Renderer::new(&render_cx);
        let fly_camera = FlyCamera::default();

        Self {
            window,
            render_cx,
            input,
            time,
            tasks,
            terrain,
            renderer,
            fly_camera,
            fly_camera_active: true,
            close_requested: false,
        }
    }

    fn frame(&mut self) {
        self.time.begin_frame();
        self.update();
        self.render();
        self.time.update_frame_count();
        self.time.wait_for_next_frame();
    }

    fn resized(&mut self, new_size: PhysicalSize<u32>) {
        self.render_cx.resized(new_size);
        self.renderer.resized(&self.render_cx);
    }

    fn update(&mut self) {
        // display framerate in window title
        self.window.set_title(&format!(
            "{} ({} fps)",
            WINDOW_TITLE,
            self.time.get_frames_last_second()
        ));

        // update flycam
        if self.fly_camera_active {
            self.fly_camera
                .update(&self.input, &self.time);
        }
        self.renderer.camera_mut().transform = self.fly_camera.get_transform();

        self.terrain.update(
            &mut self.tasks,
            &[Anchor {
                position: self.fly_camera.position,
                load_radius: 5,
            }],
        );

        self.renderer
            .update(&mut self.tasks, &self.render_cx, &self.terrain);
        self.terrain.clear_events();

        self.input.reset();
    }

    fn render(&mut self) {
        let Some(surface_texture) = self.render_cx.get_surface_texture() else {
            log::warn!("couldn't acquire surface texture");
            return;
        };

        let surface_texture_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        self.renderer
            .render(&self.render_cx, &surface_texture_view);

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
            WindowEvent::Resized(new_size) => state.resized(new_size),
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
                state.frame();
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
