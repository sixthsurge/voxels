use bytemuck::{Pod, Zeroable};

pub trait Vertex: Pod + Zeroable {
    fn vertex_buffer_layout() -> wgpu::VertexBufferLayout<'static>;
}
