use std::ops::{Add, Div, Mul, Sub};

use glam::{IVec2, IVec3, UVec2, UVec3, Vec2, Vec3};

/// Size of a 2D grid
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size2 {
    pub x: usize,
    pub y: usize,
}

impl Size2 {
    pub const ZERO: Self = Self::splat(0);
    pub const ONE: Self = Self::splat(1);

    pub const fn new(x: usize, y: usize) -> Self {
        Self { x, y }
    }

    pub const fn splat(all: usize) -> Self {
        Self { x: all, y: all }
    }

    pub const fn as_uvec2(&self) -> UVec2 {
        UVec2::new(self.x as u32, self.y as u32)
    }

    pub const fn as_ivec2(&self) -> IVec2 {
        IVec2::new(self.x as i32, self.y as i32)
    }

    pub const fn as_vec2(&self) -> Vec2 {
        Vec2::new(self.x as f32, self.y as f32)
    }

    /// Returns the product of the two components of the size
    pub const fn product(&self) -> usize {
        self.x * self.y
    }

    /// Flatten a 2D grid position into an index into a 1D array ordered by y then x
    pub const fn flatten(&self, pos: UVec3) -> usize {
        let x = pos.x as usize;
        let y = pos.y as usize;
        self.x * y + x
    }
}

impl Add for Size2 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub for Size2 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul for Size2 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
        }
    }
}

impl Div for Size2 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
        }
    }
}

pub trait AsSize2 {
    fn as_size2(&self) -> Size2;
}

impl AsSize2 for UVec2 {
    fn as_size2(&self) -> Size2 {
        Size2 {
            x: self.x as usize,
            y: self.y as usize,
        }
    }
}

impl AsSize2 for IVec2 {
    fn as_size2(&self) -> Size2 {
        Size2 {
            x: self.x as usize,
            y: self.y as usize,
        }
    }
}

impl AsSize2 for Vec2 {
    fn as_size2(&self) -> Size2 {
        Size2 {
            x: self.x as usize,
            y: self.y as usize,
        }
    }
}

/// Size of a 3D grid
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Size3 {
    pub x: usize,
    pub y: usize,
    pub z: usize,
}

impl Size3 {
    pub const ZERO: Self = Self::splat(0);
    pub const ONE: Self = Self::splat(1);

    pub const fn new(x: usize, y: usize, z: usize) -> Self {
        Self { x, y, z }
    }

    pub const fn splat(all: usize) -> Self {
        Self {
            x: all,
            y: all,
            z: all,
        }
    }

    pub const fn as_uvec3(&self) -> UVec3 {
        UVec3::new(self.x as u32, self.y as u32, self.z as u32)
    }

    pub const fn as_ivec3(&self) -> IVec3 {
        IVec3::new(self.x as i32, self.y as i32, self.z as i32)
    }

    pub const fn as_vec3(&self) -> Vec3 {
        Vec3::new(self.x as f32, self.y as f32, self.z as f32)
    }

    /// Returns the product of the three components of the size
    pub const fn product(&self) -> usize {
        self.x * self.y * self.z
    }

    /// Flatten a 3D grid position into an index into a 1D array ordered by z then y then x
    pub const fn flatten(&self, pos: UVec3) -> usize {
        let x = pos.x as usize;
        let y = pos.y as usize;
        let z = pos.z as usize;
        self.x * (self.y * z + y) + x
    }

    /// True if `v` is contained in a grid of this size
    pub const fn contains_uvec3(&self, v: UVec3) -> bool {
        v.x < self.x as u32 && v.y < self.y as u32 && v.z < self.z as u32
    }

    /// True if `v` is contained in a grid of this size
    pub const fn contains_ivec3(&self, v: IVec3) -> bool {
        v.x >= 0
            && v.y >= 0
            && v.z >= 0
            && v.x < self.x as i32
            && v.y < self.y as i32
            && v.z < self.z as i32
    }
}

impl Add for Size3 {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
            z: self.z + rhs.z,
        }
    }
}

impl Sub for Size3 {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
            z: self.z - rhs.z,
        }
    }
}

impl Mul for Size3 {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x * rhs.x,
            y: self.y * rhs.y,
            z: self.z * rhs.z,
        }
    }
}

impl Div for Size3 {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x / rhs.x,
            y: self.y / rhs.y,
            z: self.z / rhs.z,
        }
    }
}

pub trait AsSize3 {
    fn as_size3(&self) -> Size3;
}

impl AsSize3 for UVec3 {
    fn as_size3(&self) -> Size3 {
        Size3 {
            x: self.x as usize,
            y: self.y as usize,
            z: self.z as usize,
        }
    }
}

impl AsSize3 for IVec3 {
    fn as_size3(&self) -> Size3 {
        Size3 {
            x: self.x as usize,
            y: self.y as usize,
            z: self.z as usize,
        }
    }
}

impl AsSize3 for Vec3 {
    fn as_size3(&self) -> Size3 {
        Size3 {
            x: self.x as usize,
            y: self.y as usize,
            z: self.z as usize,
        }
    }
}
