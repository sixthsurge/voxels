use glam::{Mat4, Vec3};
use winit::dpi::PhysicalSize;

use crate::util::transform::Transform;

#[derive(Clone, Copy, Debug)]
pub enum Projection {
    Orthographic {
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
    Perspective {
        aspect_ratio: f32,
        fov_y_radians: f32,
        z_near: f32,
        z_far: f32,
    },
}

impl Projection {
    pub fn as_matrix(&self) -> Mat4 {
        match self {
            &Projection::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => Mat4::orthographic_rh(left, right, bottom, top, near, far),
            &Projection::Perspective {
                aspect_ratio,
                fov_y_radians,
                z_near,
                z_far,
            } => Mat4::perspective_rh(fov_y_radians, aspect_ratio, z_near, z_far),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Camera {
    pub transform: Transform,
    pub projection: Projection,
}

impl Camera {
    pub fn new(transform: Transform, projection: Projection) -> Self {
        Self {
            transform,
            projection,
        }
    }

    pub fn view_matrix(&self) -> Mat4 {
        self.transform.as_matrix().inverse()
    }

    pub fn projection_matrix(&self) -> Mat4 {
        self.projection.as_matrix()
    }

    pub fn resized(&mut self, new_size: PhysicalSize<u32>) {
        if let Projection::Perspective { aspect_ratio, .. } = &mut self.projection {
            *aspect_ratio = new_size.width as f32 / new_size.height as f32;
        }
    }

    /// Returns the position of the camera in the world
    pub fn pos(&self) -> Vec3 {
        self.transform.translation
    }

    /// Returns the direction the camera is looking in
    pub fn look_dir(&self) -> Vec3 {
        self.transform.rotation.mul_vec3(Vec3::NEG_Z)
    }
}
