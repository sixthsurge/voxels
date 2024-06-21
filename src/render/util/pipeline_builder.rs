use super::mesh::Vertex;

/// Helper struct to create render pipelines and their layouts using the builder pattern
pub struct RenderPipelineBuilder<'a> {
    label: Option<&'static str>,
    bind_group_layouts: Vec<&'a wgpu::BindGroupLayout>,
    vertex_buffer_layouts: Vec<wgpu::VertexBufferLayout<'static>>,
    vertex_module: Option<&'a wgpu::ShaderModule>,
    fragment_module: Option<&'a wgpu::ShaderModule>,
    vertex_entry_point: Option<&'a str>,
    fragment_entry_point: Option<&'a str>,
    vertex_compilation_options: wgpu::PipelineCompilationOptions<'a>,
    fragment_compilation_options: wgpu::PipelineCompilationOptions<'a>,
    targets: Vec<Option<wgpu::ColorTargetState>>,
    depth: Option<(wgpu::TextureFormat, wgpu::CompareFunction)>,
    topology: wgpu::PrimitiveTopology,
    front_face: wgpu::FrontFace,
    cull_mode: Option<wgpu::Face>,
    polygon_mode: wgpu::PolygonMode,
}

impl<'a> RenderPipelineBuilder<'a> {
    pub fn new() -> Self {
        Self {
            label: None,
            bind_group_layouts: Vec::new(),
            vertex_buffer_layouts: Vec::new(),
            vertex_module: None,
            fragment_module: None,
            vertex_entry_point: None,
            fragment_entry_point: None,
            vertex_compilation_options: wgpu::PipelineCompilationOptions::default(),
            fragment_compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: Vec::new(),
            depth: None,
            topology: wgpu::PrimitiveTopology::TriangleList,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            polygon_mode: wgpu::PolygonMode::Fill,
        }
    }

    pub fn build(self, device: &wgpu::Device) -> (wgpu::RenderPipeline, wgpu::PipelineLayout) {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &self.bind_group_layouts,
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: self.label,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: self
                    .vertex_module
                    .expect("missing vertex shader module"),
                entry_point: self
                    .vertex_entry_point
                    .expect("missing vertex entry point"),
                buffers: &self.vertex_buffer_layouts,
                compilation_options: self.vertex_compilation_options,
            },
            fragment: self
                .fragment_module
                .map(|module| wgpu::FragmentState {
                    module,
                    entry_point: self
                        .fragment_entry_point
                        .expect("missing fragment entry point"),
                    compilation_options: self.fragment_compilation_options,
                    targets: &self.targets,
                }),
            primitive: wgpu::PrimitiveState {
                topology: self.topology,
                strip_index_format: None,
                front_face: self.front_face,
                cull_mode: self.cull_mode,
                unclipped_depth: false,
                polygon_mode: self.polygon_mode,
                conservative: false,
            },
            depth_stencil: self
                .depth
                .map(|(format, depth_compare)| wgpu::DepthStencilState {
                    format,
                    depth_compare,
                    depth_write_enabled: true,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        (pipeline, pipeline_layout)
    }

    pub fn with_label(mut self, label: &'static str) -> Self {
        self.label = Some(label);
        self
    }

    pub fn with_bind_group_layout(mut self, bind_group_layout: &'a wgpu::BindGroupLayout) -> Self {
        self.bind_group_layouts
            .push(bind_group_layout);
        self
    }

    pub fn with_vertex_buffer_layout(
        mut self,
        vertex_buffer_layout: wgpu::VertexBufferLayout<'static>,
    ) -> Self {
        self.vertex_buffer_layouts
            .push(vertex_buffer_layout);
        self
    }

    pub fn with_vertex<V>(self) -> Self
    where
        V: Vertex,
    {
        self.with_vertex_buffer_layout(V::vertex_buffer_layout())
    }

    pub fn with_vertex_shader(
        mut self,
        module: &'a wgpu::ShaderModule,
        entry_point: &'static str,
    ) -> Self {
        self.vertex_module = Some(module);
        self.vertex_entry_point = Some(entry_point);
        self
    }

    pub fn with_fragment_shader(
        mut self,
        module: &'a wgpu::ShaderModule,
        entry_point: &'static str,
    ) -> Self {
        self.fragment_module = Some(module);
        self.fragment_entry_point = Some(entry_point);
        self
    }

    pub fn with_vertex_compilation_options(
        mut self,
        options: wgpu::PipelineCompilationOptions<'a>,
    ) -> Self {
        self.vertex_compilation_options = options;
        self
    }

    pub fn with_fragment_compilation_options(
        mut self,
        options: wgpu::PipelineCompilationOptions<'a>,
    ) -> Self {
        self.fragment_compilation_options = options;
        self
    }

    pub fn with_color_target(
        mut self,
        format: wgpu::TextureFormat,
        blend: Option<wgpu::BlendState>,
        write_mask: wgpu::ColorWrites,
    ) -> Self {
        self.targets
            .push(Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask,
            }));
        self
    }

    pub fn with_depth(
        mut self,
        format: wgpu::TextureFormat,
        depth_compare: wgpu::CompareFunction,
    ) -> Self {
        self.depth = Some((format, depth_compare));
        self
    }

    pub fn with_topology(mut self, topology: wgpu::PrimitiveTopology) -> Self {
        self.topology = topology;
        self
    }

    pub fn with_front_face(mut self, front_face: wgpu::FrontFace) -> Self {
        self.front_face = front_face;
        self
    }

    pub fn with_cull_mode(mut self, cull_mode: Option<wgpu::Face>) -> Self {
        self.cull_mode = cull_mode;
        self
    }

    pub fn with_polygon_mode(mut self, polygon_mode: wgpu::PolygonMode) -> Self {
        self.polygon_mode = polygon_mode;
        self
    }
}
