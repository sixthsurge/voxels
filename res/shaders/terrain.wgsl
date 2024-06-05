struct VertexInput {
    @location(0) position: vec3f,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4f,
}

struct FragmentOutput {
    @location(0) color: vec4f,
}

struct GlobalUniforms {
    camera_view_matrix: mat4x4f,
    camera_projection_matrix: mat4x4f,
}
@group(0) @binding(0)
var<uniform> global: GlobalUniforms;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = global.camera_projection_matrix * global.camera_view_matrix * vec4f(in.position, 1.0);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> FragmentOutput {
    var out: FragmentOutput;
    out.color = vec4f(1.0);
    return out;
}
