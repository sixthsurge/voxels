use glam::{DVec2, Vec2};
use rustc_hash::FxHashSet;
use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug)]
pub struct Input {
    keys_held: FxHashSet<KeyCode>,
    keys_held_last_frame: FxHashSet<KeyCode>,
    mouse_buttons_held: FxHashSet<MouseButton>,
    mouse_buttons_held_last_frame: FxHashSet<MouseButton>,
    mouse_delta: DVec2,
}

impl Input {
    pub fn new() -> Self {
        Self {
            keys_held: FxHashSet::default(),
            keys_held_last_frame: FxHashSet::default(),
            mouse_buttons_held: FxHashSet::default(),
            mouse_buttons_held_last_frame: FxHashSet::default(),
            mouse_delta: DVec2::ZERO,
        }
    }

    /// Called at the end of each frame
    pub fn reset(&mut self) {
        self.keys_held_last_frame = self.keys_held.clone();
        self.mouse_buttons_held_last_frame = self.mouse_buttons_held.clone();
        self.mouse_delta = DVec2::ZERO;
    }

    /// Returns true if the event was "consumed"
    pub fn handle_window_event(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        physical_key: PhysicalKey::Code(key_code),
                        state,
                        ..
                    },
                ..
            } => {
                match state {
                    ElementState::Pressed => {
                        self.keys_held.insert(*key_code);
                    }
                    ElementState::Released => {
                        self.keys_held.remove(key_code);
                    }
                }
                true
            }
            WindowEvent::MouseInput { button, state, .. } => {
                match state {
                    ElementState::Pressed => {
                        self.mouse_buttons_held.insert(*button);
                    }
                    ElementState::Released => {
                        self.mouse_buttons_held.remove(button);
                    }
                }
                true
            }
            _ => false,
        }
    }

    /// Returns true if the event was "consumed"
    pub fn handle_device_event(&mut self, event: &DeviceEvent) -> bool {
        match event {
            DeviceEvent::MouseMotion { delta } => {
                self.mouse_delta.x += delta.0;
                self.mouse_delta.y += delta.1;
                true
            }
            _ => false,
        }
    }

    pub fn is_key_down(&self, key_code: KeyCode) -> bool {
        self.keys_held.contains(&key_code)
    }

    pub fn is_key_just_pressed(&self, key_code: KeyCode) -> bool {
        self.keys_held.contains(&key_code) && !self.keys_held_last_frame.contains(&key_code)
    }

    pub fn is_key_just_released(&self, key_code: KeyCode) -> bool {
        self.keys_held_last_frame.contains(&key_code) && !self.keys_held.contains(&key_code)
    }

    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held.contains(&button)
    }

    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held.contains(&button)
            && !self.mouse_buttons_held_last_frame.contains(&button)
    }

    pub fn is_mouse_button_just_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held_last_frame.contains(&button)
            && !self.mouse_buttons_held.contains(&button)
    }

    pub fn mouse_delta(&self) -> DVec2 {
        self.mouse_delta
    }

    pub fn mouse_delta_f32(&self) -> Vec2 {
        self.mouse_delta.as_vec2()
    }
}
