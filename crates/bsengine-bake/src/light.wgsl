// Lights and the sky (`scene.rs`), with the mesh shader's own formulas. Part
// of `gpu_scene::TRACE_WGSL`; reads two globals the consumer declares:
//
//   var<uniform> scene: Scene;
//   var sky: texture_2d<f32>;           (Rgba32Float, unfilterable)

// kind: 0 directional, 1 point, 2 spot, 3 rect. contribution: 0 indirect,
// 1 baked. a/b/c/d per kind -- see `gpu_scene.rs::GpuLight`.
struct Light {
    kind: u32,
    contribution: u32,
    range: f32,
    intensity: f32,
    a: vec4<f32>,
    b: vec4<f32>,
    c: vec4<f32>,
    d: vec4<f32>,
};
// What is traced against, beyond the geometry: how much tree there is, the
// lights, and the sky. One uniform, so a consumer binds the scene with one
// buffer and its own per-dispatch parameters with another.
struct Scene {
    light_count: u32,
    node_count: u32,
    // 0 uniform (`sky_uniform`), 1 the equirect texture.
    sky_kind: u32,
    sky_w: u32,
    sky_h: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    sky_uniform: vec4<f32>,
    lights: array<Light, 64>,
};

fn irradiance(li: Light, p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    if li.kind == 0u {
        let l = -normalize(li.a.xyz);
        let n_dot_l = dot(n, l);
        if n_dot_l <= 0.0 || occluded(p, l, 3.0e38) {
            return vec3<f32>(0.0);
        }
        return li.c.xyz * n_dot_l;
    }
    if li.kind == 1u || li.kind == 2u {
        let to = li.a.xyz - p;
        let dist = length(to);
        if dist >= li.range || dist <= 0.0 {
            return vec3<f32>(0.0);
        }
        let l = to / dist;
        var spot = 1.0;
        if li.kind == 2u {
            spot = smoothstep(li.d.y, li.d.x, dot(-l, normalize(li.b.xyz)));
        }
        let n_dot_l = dot(n, l);
        if spot <= 0.0 || n_dot_l <= 0.0 || occluded(p, l, dist - EPS) {
            return vec3<f32>(0.0);
        }
        let t = 1.0 - dist / li.range;
        return li.c.xyz * (li.intensity * t * t * spot) * n_dot_l;
    }
    // Rect: stratified points on the panel, E = L * integral(cos cos / r^2 dA).
    let dd = length(li.a.xyz - p) / max(li.range, 1e-4);
    let w = clamp(1.0 - dd * dd * dd * dd, 0.0, 1.0);
    let window = w * w;
    if window <= 0.0 {
        return vec3<f32>(0.0);
    }
    let cr = cross(li.b.xyz, li.c.xyz);
    let area = 4.0 * length(cr);
    if area <= 0.0 {
        return vec3<f32>(0.0);
    }
    let light_n = -normalize(cr);
    var sum = 0.0;
    for (var i = 0u; i < RECT_SAMPLES; i++) {
        for (var j = 0u; j < RECT_SAMPLES; j++) {
            let u = (f32(i) + next_f32()) / f32(RECT_SAMPLES) * 2.0 - 1.0;
            let v = (f32(j) + next_f32()) / f32(RECT_SAMPLES) * 2.0 - 1.0;
            let q = li.a.xyz + li.b.xyz * u + li.c.xyz * v;
            let to = q - p;
            let r2 = dot(to, to);
            if r2 <= 1e-12 {
                continue;
            }
            let r = sqrt(r2);
            let l = to / r;
            let cos_s = dot(n, l);
            let cos_l = dot(light_n, -l);
            if cos_s <= 0.0 || cos_l <= 0.0 || occluded(p, l, r - EPS) {
                continue;
            }
            sum += cos_s * cos_l / r2;
        }
    }
    return li.d.xyz * (sum * area / f32(RECT_SAMPLES * RECT_SAMPLES)) * window;
}

fn all_lights(p: vec3<f32>, n: vec3<f32>) -> vec3<f32> {
    var e = vec3<f32>(0.0);
    for (var i = 0u; i < scene.light_count; i++) {
        e += irradiance(scene.lights[i], p, n);
    }
    return e;
}

// ---- the sky (`scene.rs::Sky::radiance`) ----------------------------------
fn sky_radiance(d: vec3<f32>) -> vec3<f32> {
    if scene.sky_kind == 0u {
        return scene.sky_uniform.xyz;
    }
    let u = atan2(d.z, d.x) / (2.0 * PI) + 0.5;
    let v = 0.5 - asin(clamp(d.y, -1.0, 1.0)) / PI;
    let x = min(u32(u * f32(scene.sky_w)), scene.sky_w - 1u);
    let y = min(u32(v * f32(scene.sky_h)), scene.sky_h - 1u);
    return textureLoad(sky, vec2<u32>(x, y), 0).xyz;
}
