// Custom shader for the Pickup entity: a Fresnel-style rim glow that pulses
// over time, driven entirely by the GPU via camera.time.
//
// Bind group layout below mirrors the standard mesh shader's CameraUniform
// (@group(0)) and ModelUniform (@group(1)) structs exactly -- both are
// verified against MESH_WGSL in crates/bsengine-rhi-wgpu/src/surface.rs.
// Custom shaders are compiled against the same `pipeline_layout` as the
// standard mesh pipeline (see `compile_and_store_shader`), so field name,
// order, and padding here must match precisely or pipeline creation panics
// with a wgpu validation error. Groups 2 (light) and 3 (texture) are part
// of that same pipeline layout but are left undeclared here since this
// shader doesn't sample them -- wgpu only validates bind groups the shader
// actually references.
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    time: f32,
};
struct ModelUniform {
    model: mat4x4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: vec3<f32>,
    _pad2: f32,
    base_color: vec3<f32>,
    opacity: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
}

@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    let world_pos4 = model_data.model * vec4<f32>(in.pos, 1.0);
    out.clip_pos = camera.view_proj * world_pos4;
    out.world_pos = world_pos4.xyz;
    let normal_matrix = mat3x3<f32>(
        model_data.model[0].xyz,
        model_data.model[1].xyz,
        model_data.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    // Fresnel-style rim glow: brightest at grazing angles relative to the
    // camera, pulsing over time via camera.time (matches pickup.js's old
    // PULSE_SPEED=3.0 / 0.6+0.4*sin(...) formula, now computed on the GPU
    // instead of being pushed from JS every frame).
    let n = normalize(in.world_normal);
    let v = normalize(camera.cam_pos - in.world_pos);
    let rim = pow(1.0 - clamp(dot(n, v), 0.0, 1.0), 2.5);
    let pulse = 0.6 + 0.4 * sin(camera.time * 3.0);
    let base = model_data.base_color * 0.3;
    let glow = model_data.emissive * pulse * (0.5 + rim);
    return vec4<f32>(base + glow, 1.0);
}
