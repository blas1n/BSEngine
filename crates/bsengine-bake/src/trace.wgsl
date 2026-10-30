// The tree (`bvh.rs`) and its triangles, traversed. Part of
// `gpu_scene::TRACE_WGSL`; it reads three globals the consumer declares, under
// whatever group and binding suits it (WGSL resolves module-scope names in any
// order, so a declaration after this text is fine):
//
//   var<storage, read> nodes: array<Node>;
//   var<storage, read> tris: array<Tri>;
//   var<uniform> scene: Scene;          (light.wgsl; `node_count` is read here)
//
// and two functions, `instance_count` and `instance_ray` (see `traverse`).

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

// `instance` is NO_HIT for the scene's own tree, and otherwise the consumer's
// instance the triangle was found through (`trace_instances`).
struct Hit {
    t: f32,
    tri: u32,
    instance: u32,
};

// One tree laid out at `root` in `nodes`, its triangles at `tri_base` in
// `tris`: the scene's own (both 0), or one of a consumer's per-mesh trees
// appended after it. A tree's node and triangle indices are its own, so the
// same flat layout serves at any offset. `best` is the nearest hit so far,
// which the tree must beat.
fn traverse_tree(
    root: u32,
    tri_base: u32,
    instance: u32,
    o: vec3<f32>,
    d: vec3<f32>,
    best_so_far: Hit,
    stop_at_first: bool,
) -> Hit {
    var best = best_so_far;
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
        let node = nodes[root + index];
        if !box_hit(node.lo, node.hi, o, inv, best.t) {
            continue;
        }
        if node.count > 0u {
            for (var k = node.start; k < node.start + node.count; k++) {
                let t = tri_hit(tri_base + k, o, d);
                if t > 0.0 && t < best.t {
                    best = Hit(t, tri_base + k, instance);
                    if stop_at_first {
                        return best;
                    }
                }
            }
        } else if node.start > index && index < 0xffffffffu && top < 31u {
            // A child always comes after its parent in the depth-first
            // layout, and the stack never needs more than its 32 entries
            // (above): true of every tree the builder makes, and what makes
            // this loop end on any data -- each entry pushes only larger
            // indices, below u32's top, into a bounded stack. That is what
            // the realtime GI relies on to compile without naga's loop
            // bounding (`realtime_gi::build_pipelines`), whose counter would
            // never have stopped a bad tree in practice anyway (it allows
            // 2^64 iterations) and only made the loop well-defined.
            stack[top] = index + 1u;
            stack[top + 1u] = node.start;
            top += 2u;
        }
    }
    return best;
}

// A consumer's instance as one ray meets it: which tree, the ray in that
// tree's space, and whether the ray reaches the instance's bounds at all.
struct InstanceRay {
    root: u32,
    tri_base: u32,
    o: vec3<f32>,
    d: vec3<f32>,
    enters: bool,
};

// The scene's own tree, then the consumer's instances: every consumer
// defines `instance_count() -> u32` and `instance_ray(i, o, d, t_max) ->
// InstanceRay`. The bake has none (`gpu_scene::NO_INSTANCES_WGSL`); the
// realtime GI's moving meshes are its instances, so a shadow ray in
// `light.wgsl` is blocked by them as the probe rays are. One loop over the
// trees, so `traverse_tree` has one call site: FXC (wgpu's D3D12 compiler)
// inlines every call, and this stack loop is what it is slow at -- two call
// sites would compile it twice.
fn traverse(o: vec3<f32>, d: vec3<f32>, t_max: f32, stop_at_first: bool) -> Hit {
    var best = Hit(t_max, NO_HIT, NO_HIT);
    let trees = instance_count() + 1u;
    for (var i = 0u; i < trees; i++) {
        var ray = InstanceRay(0u, 0u, o, d, scene.node_count > 0u);
        var instance = NO_HIT;
        if i > 0u {
            instance = i - 1u;
            ray = instance_ray(instance, o, d, best.t);
        }
        if !ray.enters {
            continue;
        }
        best = traverse_tree(ray.root, ray.tri_base, instance, ray.o, ray.d, best, stop_at_first);
        if stop_at_first && best.tri != NO_HIT {
            return best;
        }
    }
    return best;
}

fn occluded(o: vec3<f32>, d: vec3<f32>, t_max: f32) -> bool {
    return traverse(o, d, t_max, true).tri != NO_HIT;
}
