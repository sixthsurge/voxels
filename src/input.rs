use std::collections::HashSet;

use winit::{
    event::{DeviceEvent, ElementState, KeyEvent, MouseButton, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
};

#[derive(Debug)]
pub struct Input {
    keys_held: HashSet<KeyCode>,
    keys_held_last_frame: HashSet<KeyCode>,
    mouse_buttons_held: HashSet<MouseButton>,
    mouse_buttons_held_last_frame: HashSet<MouseButton>,
    mouse_delta: (f64, f64),
}

impl Input {
    pub fn new() -> Self {
        Self {
            keys_held: HashSet::new(),
            keys_held_last_frame: HashSet::new(),
            mouse_buttons_held: HashSet::new(),
            mouse_buttons_held_last_frame: HashSet::new(),
            mouse_delta: (0.0, 0.0),
        }
    }

    /// Called at the end of each frame
    pub fn reset(&mut self) {
        self.keys_held_last_frame = self.keys_held.clone();
        self.mouse_buttons_held_last_frame = self.mouse_buttons_held.clone();
        self.mouse_delta = (0.0, 0.0);
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
                self.mouse_delta.0 += delta.0;
                self.mouse_delta.1 += delta.1;
                true
            }
            _ => false,
        }
    }

    pub fn is_key_down(&self, key_code: KeyCode) -> bool {
        self.keys_held.contains(&key_code)
    }

    pub fn is_key_just_pressed(&self, key_code: KeyCode) -> bool {
        self.keys_held.contains(&key_code)
            && !self
                .keys_held_last_frame
                .contains(&key_code)
    }

    pub fn is_key_just_released(&self, key_code: KeyCode) -> bool {
        self.keys_held_last_frame
            .contains(&key_code)
            && !self.keys_held.contains(&key_code)
    }

    pub fn is_mouse_button_down(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held
            .contains(&button)
    }

    pub fn is_mouse_button_just_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held
            .contains(&button)
            && !self
                .mouse_buttons_held_last_frame
                .contains(&button)
    }

    pub fn is_mouse_button_just_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held_last_frame
            .contains(&button)
            && !self
                .mouse_buttons_held
                .contains(&button)
    }

    pub fn mouse_delta(&self) -> (f64, f64) {
        self.mouse_delta
    }

    pub fn mouse_delta_f32(&self) -> (f32, f32) {
        (self.mouse_delta.0 as f32, self.mouse_delta.1 as f32)
    }
}
