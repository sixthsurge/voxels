use bytemuck::{Pod, Zeroable};
use wgpu::util::DeviceExt;

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

#[derive(Debug)]
pub struct Mesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_buffer_layout: wgpu::VertexBufferLayout<'static>,
    index_count: u32,
    index_format: wgpu::IndexFormat,
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
        let vertex_buffer_layout = V::vertex_buffer_layout();
        let index_count = indices.len() as u32;
        let index_format = I::index_format();

        return Self {
            vertex_buffer,
            index_buffer,
            vertex_buffer_layout,
            index_count,
            index_format,
        };
    }

    pub fn draw<'mesh, 'render_pass>(&'mesh self, render_pass: &mut wgpu::RenderPass<'render_pass>)
    where
        'mesh: 'render_pass,
    {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), self.index_format);
        render_pass.draw_indexed(0..self.index_count, 0, 0..1)
    }

    pub fn vertex_buffer_layout(&self) -> wgpu::VertexBufferLayout<'static> {
        self.vertex_buffer_layout.clone()
    }
}
