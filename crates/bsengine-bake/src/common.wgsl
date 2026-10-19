// Shared by every shader that traces the GI scene (`gpu_scene::TRACE_WGSL`):
// the lightmap bake (`gpu.wgsl`) and the renderer's realtime probe update.
// Constants, the random numbers, and directions -- `sample.rs` moved to WGSL.

const PI: f32 = 3.14159265358979323846;
const EPS: f32 = 1e-3;
const RECT_SAMPLES: u32 = 4u;
const NO_HIT: u32 = 0xffffffffu;

// ---- random numbers: PCG hash (Jarzynski & Olano) -------------------------
// The consumer seeds `rng_state` per invocation (per texel, per probe ray).
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
