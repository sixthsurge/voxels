use std::sync::Arc;

use fly_camera::FlyCamera;
use generational_arena::Index;
use glam::IVec3;
use input::Input;
use render::{context::RenderContext, RenderEngine};
use tasks::Tasks;
use terrain::{area::Area, chunk::CHUNK_SIZE, position_types::ChunkPos, Terrain};
use time::{TargetFrameRate, Time};
use util::size::Size3;
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    error::EventLoopError,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::KeyCode,
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

/// Number of threads to use for task processing
const TASKS_WORKER_THREAD_COUNT: usize = 4;

/// Priority value for chunk mesh generation tasks
const CHUNK_MESH_GENERATION_PRIORITY: i32 = 0;

/// Priority value for chunk loading tasks
const CHUNK_LOADING_PRIORITY: i32 = 1;

/// Priority value for chunk mesh generation tasks when an up-to-date mesh already exists
const CHUNK_MESH_OPTIMIZATION_PRIORITY: i32 = 2;

struct State {
    window: Arc<Window>,
    render_context: RenderContext,
    time: Time,
    input: Input,
    tasks: Tasks,
    render_engine: RenderEngine,
    terrain: Terrain,
    area_index: Index,
    fly_camera: FlyCamera,
    fly_camera_active: bool,
    close_requested: bool,
    use_cave_culling: bool,
}

impl State {
    fn new(window: Arc<Window>) -> Self {
        let render_context = RenderContext::new(window.clone());
        let input = Input::new();
        let time = Time::new(TargetFrameRate::UnlimitedOrVsync);
        let tasks = Tasks::new(TASKS_WORKER_THREAD_COUNT);
        let render_engine = RenderEngine::new(&render_context);
        let mut terrain = Terrain::new();
        let fly_camera = FlyCamera::default();

        let area_index = terrain.areas_mut().insert(Area::new(
            ChunkPos::ZERO,
            Size3::new(40, 8, 40),
            terrain::area::AreaShape::Cubic,
        ));

        Self {
            window,
            render_context,
            input,
            time,
            tasks,
            terrain,
            area_index,
            render_engine,
            fly_camera,
            fly_camera_active: true,
            close_requested: false,
            use_cave_culling: false,
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
        self.render_context.resized(new_size);
        self.render_engine
            .resized(&self.render_context);
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
        self.render_engine
            .camera_mut()
            .transform = self.fly_camera.get_transform();
        self.terrain.areas_mut()[self.area_index]
            .set_center(self.fly_camera.position / (CHUNK_SIZE as f32));

        self.terrain.clear_events();
        self.terrain.update(&mut self.tasks);

        if self
            .input
            .is_key_just_pressed(KeyCode::KeyC)
        {
            self.use_cave_culling = !self.use_cave_culling;
        }

        self.input.reset();
    }

    fn render(&mut self) {
        let Some(surface_texture) = self
            .render_context
            .get_surface_texture()
        else {
            log::warn!("couldn't acquire surface texture");
            return;
        };

        let output_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let loaded_area = self
            .terrain
            .areas()
            .get(self.area_index)
            .unwrap();

        self.render_engine.render(
            &self.render_context,
            &output_view,
            &mut self.tasks,
            &self.terrain,
            loaded_area,
            self.use_cave_culling,
        );

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
