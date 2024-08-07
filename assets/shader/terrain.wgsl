struct ColorTargets {
    @location(0) color: vec4f,
}

struct Attributes {
    @location(0) position: vec3f,
    @location(1) texture_index: u32,
    @location(2) light: vec4f,
    @location(3) uv: vec2f,
};

struct Interpolated {
    @builtin(position) clip_position: vec4f,
    @location(0) uv: vec2f,
    @location(1) texture_index: u32,
    @location(2) light: vec4f,
}

struct GlobalUniforms {
    camera_view_matrix: mat4x4f,
    camera_projection_matrix: mat4x4f,
}

struct RenderGroupUniforms {
    offset: vec3f,
}

@group(0) @binding(0)
var texture_array: texture_2d_array<f32>;

@group(0) @binding(1)
var texture_array_sampler: sampler;

@group(1) @binding(0)
var<uniform> global: GlobalUniforms;

@group(2) @binding(0)
var<uniform> render_group: RenderGroupUniforms;

@vertex
fn vs_main(in: Attributes) -> Interpolated {
    var out: Interpolated;
    out.clip_position = global.camera_projection_matrix * global.camera_view_matrix * vec4f(in.position + render_group.offset, 1.0);
    out.uv = in.uv;
    out.texture_index = in.texture_index;
    out.light = in.light;
    return out;
}

@fragment
fn fs_main(in: Interpolated) -> ColorTargets {
    var out: ColorTargets;

    let light = min(in.light.xyz + vec3f(in.light.w), vec3f(1.0));
    out.color = textureSample(texture_array, texture_array_sampler, in.uv, in.texture_index) * vec4f(light, 1.0);
    // out.color = vec4f(light, 1.0); white world

    return out;
}
