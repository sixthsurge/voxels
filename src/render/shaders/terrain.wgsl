struct VertexInput {
    @location(0) position: vec3f,
    @location(1) uv: vec2f,
    @location(2) texture_index: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
    @location(1) texture_index: u32,
}

struct FragmentOutput {
    @location(0) color: vec4f,
}

@group(0) @binding(0)
var texture_array: texture_2d_array<f32>;

@group(0) @binding(1)
var texture_array_sampler: sampler;

struct GlobalUniforms {
    camera_view_matrix: mat4x4f,
    camera_projection_matrix: mat4x4f,
}
@group(1) @binding(0)
var<uniform> global: GlobalUniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = global.camera_projection_matrix * global.camera_view_matrix * vec4f(in.position, 1.0);
    out.uv = in.uv;
    out.texture_index = in.texture_index;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = textureSample(texture_array, texture_array_sampler, in.uv, in.texture_index);
    return out;
}
