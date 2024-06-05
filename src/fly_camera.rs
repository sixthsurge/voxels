use glam::{EulerRot, Quat, Vec3};
use winit::keyboard::KeyCode;

use crate::{input::Input, time::Time, util::transform::Transform};

#[derive(Clone, Debug)]
pub struct FlyCamera {
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub speed: f32,
    pub sensitivity: f32,
    pub key_forward: KeyCode,
    pub key_backward: KeyCode,
    pub key_right: KeyCode,
    pub key_left: KeyCode,
    pub key_up: KeyCode,
    pub key_down: KeyCode,
}

impl FlyCamera {
    pub fn get_transform(&self) -> Transform {
        Transform {
            translation: self.position,
            scale: Vec3::ONE,
            rotation: Quat::from_euler(EulerRot::ZYX, 0.0, self.yaw, self.pitch),
        }
    }

    pub fn update(&mut self, input: &Input, time: &Time) {
        // movement
        let input_forward = axis_input(input, self.key_forward, self.key_backward);
        let input_right = axis_input(input, self.key_right, self.key_left);
        let input_up = axis_input(input, self.key_up, self.key_down);

        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        let dir_forward = Vec3::new(-sin_yaw, 0.0, -cos_yaw);
        let dir_right = Vec3::new(cos_yaw, 0.0, -sin_yaw);
        const DIR_UP: Vec3 = Vec3::new(0.0, 1.0, 0.0);

        let speed = self.speed * time.delta_seconds();

        self.position += dir_forward * input_forward * speed;
        self.position += dir_right * input_right * speed;
        self.position += DIR_UP * input_up * speed;

        // rotation
        let (rotate_yaw, rotate_pitch) = input.mouse_delta_f32();
        self.yaw -= self.sensitivity * rotate_yaw;
        self.pitch -= self.sensitivity * rotate_pitch;

        self.pitch = self
            .pitch
            .clamp(-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2);
    }
}

impl Default for FlyCamera {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            speed: 5.0,
            sensitivity: 0.05,
            key_forward: KeyCode::KeyW,
            key_backward: KeyCode::KeyS,
            key_right: KeyCode::KeyD,
            key_left: KeyCode::KeyA,
            key_up: KeyCode::Space,
            key_down: KeyCode::ShiftLeft,
        }
    }
}

fn axis_input(input: &Input, key_pos: KeyCode, key_neg: KeyCode) -> f32 {
    (input.is_key_down(key_pos) as i32 - input.is_key_down(key_neg) as i32) as f32
}
