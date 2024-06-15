// based on https://github.com/gfx-rs/wgpu/blob/trunk/examples/src/mipmap/mod.rs

/// Generates mips for 2D textures and texture arrays
pub struct MipGenerator {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    texture_format: wgpu::TextureFormat,
}

impl MipGenerator {
    pub fn new(device: &wgpu::Device, texture_format: wgpu::TextureFormat) -> Self {
        // TODO get shader from proper asset system
        let shader =
            device.create_shader_module(wgpu::include_wgsl!("../../../assets/shader/blit.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                compilation_options: Default::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                compilation_options: Default::default(),
                targets: &[Some(texture_format.into())],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let bind_group_layout = pipeline.get_bind_group_layout(0);

        Self {
            pipeline,
            bind_group_layout,
            texture_format,
        }
    }

    /// generate mipmaps for a 2D texture or texture array
    pub fn generate_mips(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        device: &wgpu::Device,
        texture: &wgpu::Texture,
        array_layer_count: u32,
        mip_count: u32,
    ) {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mip"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        for array_layer_index in 0..array_layer_count {
            let views = (0..mip_count)
                .map(|mip| {
                    texture.create_view(&wgpu::TextureViewDescriptor {
                        label: Some("mip"),
                        format: None,
                        dimension: Some(wgpu::TextureViewDimension::D2),
                        aspect: wgpu::TextureAspect::All,
                        base_mip_level: mip,
                        mip_level_count: Some(1),
                        base_array_layer: array_layer_index,
                        array_layer_count: Some(1),
                    })
                })
                .collect::<Vec<_>>();

            for target_mip in 1..mip_count as usize {
                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    layout: &self.bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&views[target_mip - 1]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                    label: None,
                });

                let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: None,
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &views[target_mip],
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });

                render_pass.set_pipeline(&self.pipeline);
                render_pass.set_bind_group(0, &bind_group, &[]);
                render_pass.draw(0..3, 0..1);
            }
        }
    }
}
