// The GPU lightmap baker: one invocation per texel, the CPU tracer
// (`trace.rs`, `scene.rs`, `bvh.rs`) moved to WGSL step for step. The CPU
// backend is the reference; where this one has to differ -- a 32-bit RNG, an
// explicit stack instead of recursion -- the result is the same estimate of
// the same integral, not the same bits.

const PI: f32 = 3.14159265358979323846;
const EPS: f32 = 1e-3;
const RECT_SAMPLES: u32 = 4u;
const NO_HIT: u32 = 0xffffffffu;

struct Node {
    lo: vec3<f32>,
    start: u32,
    hi: vec3<f32>,
    count: u32,
};
// Corners, the shading normal (already turned to face the side the vertex
// normals face, as `trace.rs` turns it), albedo and emissive.
struct Tri {
    p0: vec4<f32>,
    p1: vec4<f32>,
    p2: vec4<f32>,
    n: vec4<f32>,
    albedo: vec4<f32>,
    emissive: vec4<f32>,
};
struct Job {
    pos: vec3<f32>,
    layer: u32,
    normal: vec3<f32>,
    // x in the low 16 bits, y in the high.
    xy: u32,
};
// kind: 0 directional, 1 point, 2 spot, 3 rect. contribution: 0 indirect,
// 1 baked. a/b/c/d per kind -- see `gpu.rs::GpuLight`.
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
struct Params {
    samples: u32,
    bounces: u32,
    seed: u32,
    job_offset: u32,
    job_count: u32,
    light_count: u32,
    bake_direct: u32,
    node_count: u32,
    // 0 uniform (`sky_uniform`), 1 the equirect texture.
    sky_kind: u32,
    sky_w: u32,
    sky_h: u32,
    _pad: u32,
    sky_uniform: vec4<f32>,
};
struct Lights {
    items: array<Light, 64>,
};

@group(0) @binding(0) var<storage, read> nodes: array<Node>;
@group(0) @binding(1) var<storage, read> tris: array<Tri>;
@group(0) @binding(2) var<storage, read> jobs: array<Job>;
@group(0) @binding(3) var<storage, read_write> out: array<vec4<f32>>;
@group(0) @binding(4) var<uniform> params: Params;
@group(0) @binding(5) var<uniform> lights: Lights;
@group(0) @binding(6) var sky: texture_2d<f32>;

// ---- random numbers: PCG hash (Jarzynski & Olano), per texel -------------
var<private> rng_state: u32;

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn next_f32() -> f32 {
    rng_state = pcg(rng_state);
    return f32(rng_state >> 8u) / 16777216.0;
}

// ---- directions (`sample.rs`) --------------------------------------------
fn basis(n: vec3<f32>) -> mat2x3<f32> {
    let sign = select(-1.0, 1.0, n.z >= 0.0);
    let a = -1.0 / (sign + n.z);
    let b = n.x * n.y * a;
    return mat2x3<f32>(
        vec3<f32>(1.0 + sign * n.x * n.x * a, sign * b, -sign * n.x),
        vec3<f32>(b, sign + n.y * n.y * a, -n.y),
    );
}

fn cosine_hemisphere(n: vec3<f32>) -> vec3<f32> {
    let u1 = next_f32();
    let u2 = next_f32();
    let r = sqrt(u1);
    let phi = 2.0 * PI * u2;
    let tb = basis(n);
    let z = sqrt(max(1.0 - u1, 0.0));
    return normalize(tb[0] * (r * cos(phi)) + tb[1] * (r * sin(phi)) + n * z);
}

// ---- the tree (`bvh.rs`) --------------------------------------------------
// A component of exactly zero makes the slab test 0 * inf = NaN; a tiny
// stand-in keeps the test exact for every ray that can hit anything.
fn safe_inv(d: vec3<f32>) -> vec3<f32> {
    let tiny = vec3<f32>(1e-30);
    return 1.0 / select(d, tiny, abs(d) < tiny);
}

fn box_hit(lo: vec3<f32>, hi: vec3<f32>, o: vec3<f32>, inv: vec3<f32>, t_max: f32) -> bool {
    let t0 = (lo - o) * inv;
    let t1 = (hi - o) * inv;
    let tmin = min(t0, t1);
    let tmax = max(t0, t1);
    let near = max(max(max(tmin.x, tmin.y), tmin.z), 0.0);
    let far = min(min(min(tmax.x, tmax.y), tmax.z), t_max);
    return near <= far;
}

// Moller-Trumbore; t <= 0 for a miss.
fn tri_hit(i: u32, o: vec3<f32>, d: vec3<f32>) -> f32 {
    let t = tris[i];
    let e1 = t.p1.xyz - t.p0.xyz;
    let e2 = t.p2.xyz - t.p0.xyz;
    let p = cross(d, e2);
    let det = dot(e1, p);
    if abs(det) < 1e-12 {
        return -1.0;
    }
    let inv = 1.0 / det;
    let s = o - t.p0.xyz;
    let b1 = dot(s, p) * inv;
    if b1 < 0.0 || b1 > 1.0 {
        return -1.0;
    }
    let q = cross(s, e1);
    let b2 = dot(d, q) * inv;
    if b2 < 0.0 || b1 + b2 > 1.0 {
        return -1.0;
    }
    let dist = dot(e2, q) * inv;
    return select(-1.0, dist, dist > 1e-6);
}

struct Hit {
    t: f32,
    tri: u32,
};

fn traverse(o: vec3<f32>, d: vec3<f32>, t_max: f32, stop_at_first: bool) -> Hit {
    var best = Hit(t_max, NO_HIT);
    if params.node_count == 0u {
        return best;
    }
    let inv = safe_inv(d);
    var stack: array<u32, 64>;
    var top = 1u;
    stack[0] = 0u;
    while top > 0u {
        top -= 1u;
        let index = stack[top];
        let node = nodes[index];
        if !box_hit(node.lo, node.hi, o, inv, best.t) {
            continue;
        }
        if node.count > 0u {
            for (var k = node.start; k < node.start + node.count; k++) {
                let t = tri_hit(k, o, d);
                if t > 0.0 && t < best.t {
                    best = Hit(t, k);
                    if stop_at_first {
                        return best;
                    }
                }
            }
        } else {
            stack[top] = index + 1u;
            stack[top + 1u] = node.start;
            top += 2u;
        }
    }
    return best;
}

fn occluded(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> bool {
    return traverse(o, d, t_max, true).tri != NO_HIT;
}

// ---- lights (`scene.rs::irradiance`, the mesh shader's formulas) --------
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
    for (var i = 0u; i < params.light_count; i++) {
        e += irradiance(lights.items[i], p, n);
    }
    return e;
}

// ---- the sky (`scene.rs::Sky::radiance`) ----------------------------------
fn sky_radiance(d: vec3<f32>) -> vec3<f32> {
    if params.sky_kind == 0u {
        return params.sky_uniform.xyz;
    }
    let u = atan2(d.z, d.x) / (2.0 * PI) + 0.5;
    let v = 0.5 - asin(clamp(d.y, -1.0, 1.0)) / PI;
    let x = min(u32(u * f32(params.sky_w)), params.sky_w - 1u);
    let y = min(u32(v * f32(params.sky_h)), params.sky_h - 1u);
    return textureLoad(sky, vec2<u32>(x, y), 0).xyz;
}

// ---- the path (`trace.rs::World::incoming`, without recursion) -----------
// The CPU's `emissive + albedo * (E_direct / pi + incoming(next))`, unrolled:
// each hit adds `throughput * (emissive + albedo * E_direct / pi)`, and the
// next bounce is weighted by the albedos so far. The first hit is always
// shaded; further ones only while `depth < bounces`, as on the CPU.
fn incoming(o0: vec3<f32>, d0: vec3<f32>) -> vec3<f32> {
    var o = o0;
    var d = d0;
    var throughput = vec3<f32>(1.0);
    var radiance = vec3<f32>(0.0);
    var depth = 1u;
    loop {
        let hit = traverse(o, d, 3.0e38, false);
        if hit.tri == NO_HIT {
            radiance += throughput * sky_radiance(d);
            break;
        }
        let tri = tris[hit.tri];
        let ng = tri.n.xyz;
        if dot(ng, d) >= 0.0 {
            break;
        }
        let y = o + d * hit.t + ng * EPS;
        radiance += throughput * (tri.emissive.xyz + tri.albedo.xyz * all_lights(y, ng) / PI);
        if depth >= params.bounces {
            break;
        }
        throughput *= tri.albedo.xyz;
        d = cosine_hemisphere(ng);
        o = y;
        depth += 1u;
    }
    return radiance;
}

@compute @workgroup_size(64, 1, 1)
fn cs_bake(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= params.job_count {
        return;
    }
    let index = params.job_offset + gid.x;
    let job = jobs[index];
    let x = job.xy & 0xffffu;
    let y = job.xy >> 16u;
    // Per texel, from where it is, never from which batch ran it: the same
    // texel draws the same numbers however the work is split.
    rng_state = pcg(params.seed ^ pcg(job.layer ^ pcg(x ^ pcg(y))));
    let n = job.normal;
    let origin = job.pos + n * EPS;
    let samples = max(params.samples, 1u);
    var sum = vec3<f32>(0.0);
    for (var s = 0u; s < samples; s++) {
        sum += incoming(origin, cosine_hemisphere(n));
    }
    var result = sum / f32(samples);
    if params.bake_direct != 0u {
        for (var i = 0u; i < params.light_count; i++) {
            let li = lights.items[i];
            if li.contribution == 1u {
                result += irradiance(li, origin, n) / PI;
            }
        }
    }
    out[index] = vec4<f32>(result, 1.0);
}
