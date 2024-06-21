

/// Helper struct to construct bind groups and bind group layouts together using the builder
/// pattern
pub struct BindGroupBuilder<'a> {
    label: Option<&'static str>,
    layout_entries: Vec<wgpu::BindGroupLayoutEntry>,
    binding_resources: Vec<wgpu::BindingResource<'a>>,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            layout_entries: Vec::new(),
            binding_resources: Vec::new(),
        }
    }

    pub fn build(self, device: &wgpu::Device) -> (wgpu::BindGroup, wgpu::BindGroupLayout) {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &self.layout_entries,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: self.label,
            layout: &bind_group_layout,
            entries: &self
                .binding_resources
                .into_iter()
                .zip(0u32..)
                .map(|(resource, binding)| wgpu::BindGroupEntry { binding, resource })
                .collect::<Vec<_>>(),
        });

        (bind_group, bind_group_layout)
    }

    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_uniform_buffer(
        mut self,
        buffer: &'a wgpu::Buffer,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let binding = self.layout_entries.len() as u32;

        let layout_entry = wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };

        let binding_resource = buffer.as_entire_binding();

        self.layout_entries.push(layout_entry);
        self.binding_resources
            .push(binding_resource);
        self
    }

    pub fn with_texture_view(
        mut self,
        view: &'a wgpu::TextureView,
        view_dimension: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let binding = self.layout_entries.len() as u32;

        let layout_entry = wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Texture {
                multisampled: false,
                view_dimension,
                sample_type,
            },
            count: None,
        };

        let binding_resource = wgpu::BindingResource::TextureView(view);

        self.layout_entries.push(layout_entry);
        self.binding_resources
            .push(binding_resource);
        self
    }

    pub fn with_sampler(
        mut self,
        sampler: &'a wgpu::Sampler,
        sampler_binding_type: wgpu::SamplerBindingType,
        visibility: wgpu::ShaderStages,
    ) -> Self {
        let binding = self.layout_entries.len() as u32;

        let layout_entry = wgpu::BindGroupLayoutEntry {
            binding,
            visibility,
            ty: wgpu::BindingType::Sampler(sampler_binding_type),
            count: None,
        };

        let binding_resource = wgpu::BindingResource::Sampler(sampler);

        self.layout_entries.push(layout_entry);
        self.binding_resources
            .push(binding_resource);
        self
    }
}
