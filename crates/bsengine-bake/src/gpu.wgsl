// The GPU lightmap baker: one invocation per texel, the CPU tracer
// (`trace.rs`, `scene.rs`, `bvh.rs`) moved to WGSL step for step. The CPU
// backend is the reference; where this one has to differ -- a 32-bit RNG, an
// explicit stack instead of recursion -- the result is the same estimate of
// the same integral, not the same bits.
//
// Compiled after `gpu_scene::TRACE_WGSL` (common.wgsl, trace.wgsl,
// light.wgsl), which this file declares the globals of and builds on.

struct Job {
    pos: vec3<f32>,
    layer: u32,
    normal: vec3<f32>,
    // x in the low 16 bits, y in the high.
    xy: u32,
};
struct Params {
    // All of a texel's samples, and the ones this pass traces: the samples
    // are split across passes (see `gpu.rs`), every texel in each.
    total_samples: u32,
    sample_offset: u32,
    sample_count: u32,
    bounces: u32,
    seed: u32,
    job_offset: u32,
    job_count: u32,
    bake_direct: u32,
};

@group(0) @binding(0) var<storage, read> nodes: array<Node>;
@group(0) @binding(1) var<storage, read> tris: array<Tri>;
@group(0) @binding(2) var<storage, read> jobs: array<Job>;
@group(0) @binding(3) var<storage, read_write> out: array<vec4<f32>>;
@group(0) @binding(4) var<uniform> params: Params;
@group(0) @binding(5) var<uniform> scene: Scene;
@group(0) @binding(6) var sky: texture_2d<f32>;

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
    // Per texel and per sample, from where the texel is and which sample this
    // is -- never from which pass or batch ran it: the same texel draws the
    // same numbers however the work is split.
    let texel_seed = pcg(params.seed ^ pcg(job.layer ^ pcg(x ^ pcg(y))));
    let n = job.normal;
    let origin = job.pos + n * EPS;
    // A running sum, divided by `total_samples` once every pass is in
    // (`gpu.rs`). The buffer starts zeroed.
    var sum = out[index].xyz;
    for (var s = params.sample_offset; s < params.sample_offset + params.sample_count; s++) {
        rng_state = pcg(texel_seed ^ pcg(s));
        sum += incoming(origin, cosine_hemisphere(n));
    }
    // The direct term once, in the first pass, weighted so the final divide
    // leaves it whole.
    if params.bake_direct != 0u && params.sample_offset == 0u {
        rng_state = pcg(texel_seed ^ 0x9e3779b9u);
        for (var i = 0u; i < scene.light_count; i++) {
            let li = scene.lights[i];
            if li.contribution == 1u {
                sum += irradiance(li, origin, n) / PI * f32(params.total_samples);
            }
        }
    }
    out[index] = vec4<f32>(sum, 1.0);
}
