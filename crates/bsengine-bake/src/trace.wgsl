// The tree (`bvh.rs`) and its triangles, traversed. Part of
// `gpu_scene::TRACE_WGSL`; it reads three globals the consumer declares, under
// whatever group and binding suits it (WGSL resolves module-scope names in any
// order, so a declaration after this text is fine):
//
//   var<storage, read> nodes: array<Node>;
//   var<storage, read> tris: array<Tri>;
//   var<uniform> scene: Scene;          (light.wgsl; `node_count` is read here)

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
    if scene.node_count == 0u {
        return best;
    }
    let inv = safe_inv(d);
    // 32 entries: the tree is at most `bvh::MAX_DEPTH` (30) deep, and a
    // traversal never holds more than one entry per level plus one. Larger
    // costs speed -- at 64 the bake ran a third slower.
    var stack: array<u32, 32>;
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
