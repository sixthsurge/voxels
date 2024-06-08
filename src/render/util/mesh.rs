use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

/// Stores vertices and indices to be uploaded to the GPU as a `Mesh`
#[derive(Debug)]
pub struct MeshData<V, I>
where
    V: Vertex,
    I: Index,
{
    pub vertices: Vec<V>,
    pub indices: Vec<I>,
}

impl<V, I> MeshData<V, I>
where
    V: Vertex,
    I: Index,
{
    /// Empty mesh data with no vertices or indices
    pub fn empty() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Creates a new mesh on the GPU with the vertices and indices
    pub fn create_mesh(&self, device: &wgpu::Device) -> Mesh {
        Mesh::new(device, &self.vertices, &self.indices)
    }
}

/// Holds handles to a vertex and index buffer.
/// once created, `Mesh` does not hold the vertices/indices in memory - these are stored on the GPU
#[derive(Debug)]
pub struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_format: wgpu::IndexFormat,
    index_count: u32,
}

impl Mesh {
    pub fn new<V, I>(device: &wgpu::Device, vertices: &[V], indices: &[I]) -> Self
    where
        V: Vertex,
        I: Index,
    {
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        let index_count = indices.len() as u32;
        let index_format = I::index_format();

        return Self {
            vertex_buffer,
            index_buffer,
            index_count,
            index_format,
        };
    }

    pub fn vertex_buffer(&self) -> &wgpu::Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &wgpu::Buffer {
        &self.index_buffer
    }

    pub fn index_format(&self) -> wgpu::IndexFormat {
        self.index_format
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }
}

pub trait Vertex: Pod + Zeroable {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static>;
}

pub trait Index: Pod + Zeroable {
    fn index_format() -> wgpu::IndexFormat;
}

impl Index for u16 {
    fn index_format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint16
    }
}

impl Index for u32 {
    fn index_format() -> wgpu::IndexFormat {
        wgpu::IndexFormat::Uint32
    }
}
