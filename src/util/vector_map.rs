/// Extension trait for vector types adding a `map` function that performs an operation
/// on each component
pub trait VectorMapExt {
    type Component;

    fn map<F>(&self, f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component;
}

impl VectorMapExt for glam::Vec2 {
    type Component = f32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y))
    }
}

impl VectorMapExt for glam::Vec3 {
    type Component = f32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y), f(self.z))
    }
}

impl VectorMapExt for glam::Vec4 {
    type Component = f32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y), f(self.z), f(self.w))
    }
}

impl VectorMapExt for glam::IVec2 {
    type Component = i32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y))
    }
}

impl VectorMapExt for glam::IVec3 {
    type Component = i32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y), f(self.z))
    }
}

impl VectorMapExt for glam::IVec4 {
    type Component = i32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y), f(self.z), f(self.w))
    }
}

impl VectorMapExt for glam::UVec2 {
    type Component = u32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y))
    }
}

impl VectorMapExt for glam::UVec3 {
    type Component = u32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y), f(self.z))
    }
}

impl VectorMapExt for glam::UVec4 {
    type Component = u32;

    fn map<F>(&self, mut f: F) -> Self
    where
        F: FnMut(Self::Component) -> Self::Component,
    {
        Self::new(f(self.x), f(self.y), f(self.z), f(self.w))
    }
}
