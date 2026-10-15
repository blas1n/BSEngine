//! Automatic lightmap UV unwrap.
//!
//! A lightmap needs every surface point to own a distinct texel, so the
//! second UV set ([`Vertex::uv2`]) must be a parameterisation of the mesh
//! into `[0,1]²` that never overlaps itself -- which the first UV set, built
//! for tiling textures, almost never is. Unity ("Generate Lightmap UVs"),
//! Godot (xatlas at import) and Unreal all generate one when the model does
//! not carry its own; this is the same job, at the quality a first version
//! needs rather than xatlas's.
//!
//! The method, in the order [`unwrap`] runs it:
//!
//! 1. **Charts.** Triangles are grown into connected patches whose normals
//!    stay within [`UnwrapSettings::max_chart_angle_deg`] of the patch's
//!    first triangle; the angle cap is what keeps the flattening from
//!    stretching. A cap under 90 degrees also keeps every triangle facing the
//!    right way when flattened, but not the patch from landing on itself --
//!    a spiral ramp's normals all point up while its turns stack in the
//!    projection -- so a grown patch is rasterised and, if it overlaps, falls
//!    back to one chart per triangle (wasteful, rare, always valid).
//! 2. **Projection.** Each chart is flattened onto the plane of its seed
//!    triangle, in world units.
//! 3. **Packing.** One global scale (UV per world unit) is shared by every
//!    chart, so a texel covers the same world area everywhere -- the
//!    property that makes "lightmap resolution" mean something. Charts are
//!    shelf-packed with a gutter around each, and the scale is the largest
//!    one at which they all fit.
//!
//! A vertex on a chart border belongs to two charts with two different
//! `uv2`, so [`unwrap`] duplicates it. That is why it cannot run on a mesh
//! whose other arrays are parallel to the vertices (skin weights, morph
//! deltas): those keep `uv2 = [0, 0]`, and a lightmap is for static geometry.
//!
//! Everything here is deterministic -- no hashing order, no floating-point
//! reduction order that depends on a thread -- because the output is cached
//! by the importer and an unstable unwrap would invalidate the cache and
//! shift baked lightmaps between runs.

use crate::mesh::Vertex;
use glam::{Vec2, Vec3};
use std::collections::{HashMap, VecDeque};

/// Resolution [`uv2_is_valid`] rasterises at. Fine enough that two triangles
/// separated by even one gutter texel at the import resolution do not share
/// a texel centre, coarse enough to stay cheap in a test.
const VALIDATION_RESOLUTION: u32 = 1024;

/// Tuning for [`unwrap`]. `Default` is what imports use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UnwrapSettings {
    /// Triangles join a chart while their normal is within this angle of the
    /// chart's seed normal. Smaller means more, flatter charts (less
    /// distortion, more seams); 45 degrees keeps a sphere to a few dozen.
    pub max_chart_angle_deg: f32,
    /// Empty texels kept around every chart, measured against `atlas_size`.
    /// Bilinear filtering and the baker's dilation both read this far past a
    /// chart's edge; without it a neighbouring chart's light leaks in.
    pub gutter_texels: f32,
    /// The lightmap resolution the gutter is measured against.
    pub atlas_size: u32,
}

impl Default for UnwrapSettings {
    fn default() -> Self {
        Self {
            max_chart_angle_deg: 45.0,
            gutter_texels: 2.0,
            atlas_size: 1024,
        }
    }
}

/// One flattened chart: the triangles in it and the 2D bounds of their
/// projection, in world units.
struct Chart {
    triangles: Vec<usize>,
    tangent: Vec3,
    bitangent: Vec3,
    min: Vec2,
    size: Vec2,
}

/// `(vertices, indices)` with [`Vertex::uv2`] written. Vertices on a chart
/// border are duplicated, so the output has at least as many vertices as the
/// input; every other field is copied unchanged and the triangles keep their
/// order and winding, so triangle `t` of the output is triangle `t` of the
/// input. Deterministic.
pub fn unwrap(
    vertices: &[Vertex],
    indices: &[u32],
    settings: &UnwrapSettings,
) -> (Vec<Vertex>, Vec<u32>) {
    let triangle_count = indices.len() / 3;
    if triangle_count == 0 {
        return (vertices.to_vec(), indices.to_vec());
    }
    let position = |i: u32| Vec3::from(vertices[i as usize].position);
    let triangles: Vec<[Vec3; 3]> = (0..triangle_count)
        .map(|t| {
            [
                position(indices[3 * t]),
                position(indices[3 * t + 1]),
                position(indices[3 * t + 2]),
            ]
        })
        .collect();

    // Face normals. A zero-area triangle has no normal; +Y is as good as any
    // and keeps it from poisoning a chart with NaN.
    let normals: Vec<Vec3> = triangles
        .iter()
        .map(|&[a, b, c]| {
            let n = (b - a).cross(c - a);
            if n.length_squared() > 1e-20 {
                n.normalize()
            } else {
                Vec3::Y
            }
        })
        .collect();

    let neighbours = triangle_neighbours(vertices, indices);
    let charts = grow_charts(&triangles, &normals, &neighbours, settings);
    let mut chart_of = vec![0usize; triangle_count];
    for (c, chart) in charts.iter().enumerate() {
        for &t in &chart.triangles {
            chart_of[t] = c;
        }
    }

    let (scale, origins, gutter) = pack(&charts, settings);

    // Emit vertices in triangle order, one per (chart, source vertex).
    let mut out_vertices: Vec<Vertex> = Vec::with_capacity(vertices.len());
    let mut out_indices: Vec<u32> = Vec::with_capacity(indices.len());
    let mut remap: HashMap<(usize, u32), u32> = HashMap::new();
    for t in 0..triangle_count {
        let c = chart_of[t];
        let chart = &charts[c];
        for k in 0..3 {
            let source = indices[3 * t + k];
            let index = *remap.entry((c, source)).or_insert_with(|| {
                let p = position(source);
                let flat = Vec2::new(p.dot(chart.tangent), p.dot(chart.bitangent));
                let uv2 = origins[c] + Vec2::splat(gutter) + (flat - chart.min) * scale;
                let mut v = vertices[source as usize];
                v.uv2 = uv2.to_array();
                out_vertices.push(v);
                (out_vertices.len() - 1) as u32
            });
            out_indices.push(index);
        }
    }
    (out_vertices, out_indices)
}

/// For each triangle, the triangles sharing an edge with it, in ascending
/// order. Vertices are welded by position first: a cube's faces carry their
/// own copies of its corner vertices (for per-face normals), and without
/// welding no two faces would ever count as neighbours.
fn triangle_neighbours(vertices: &[Vertex], indices: &[u32]) -> Vec<Vec<usize>> {
    let mut weld: HashMap<[i64; 3], u32> = HashMap::new();
    let weld_id: Vec<u32> = vertices
        .iter()
        .map(|v| {
            let key = v.position.map(|x| (x as f64 * 1e5).round() as i64);
            let next = weld.len() as u32;
            *weld.entry(key).or_insert(next)
        })
        .collect();

    let triangle_count = indices.len() / 3;
    let mut edges: HashMap<(u32, u32), Vec<usize>> = HashMap::new();
    for t in 0..triangle_count {
        for k in 0..3 {
            let a = weld_id[indices[3 * t + k] as usize];
            let b = weld_id[indices[3 * t + (k + 1) % 3] as usize];
            if a != b {
                edges.entry((a.min(b), a.max(b))).or_default().push(t);
            }
        }
    }
    let mut neighbours = vec![Vec::new(); triangle_count];
    for tris in edges.values() {
        for &a in tris {
            for &b in tris {
                if a != b {
                    neighbours[a].push(b);
                }
            }
        }
    }
    // The map's iteration order is arbitrary; sorting is what makes the
    // chart growth below -- and so the whole unwrap -- reproducible.
    for list in &mut neighbours {
        list.sort_unstable();
        list.dedup();
    }
    neighbours
}

/// Grow charts by breadth-first search from each unassigned triangle in
/// index order.
fn grow_charts(
    triangles: &[[Vec3; 3]],
    normals: &[Vec3],
    neighbours: &[Vec<usize>],
    settings: &UnwrapSettings,
) -> Vec<Chart> {
    let cos_limit = settings.max_chart_angle_deg.to_radians().cos();
    let mut assigned = vec![false; triangles.len()];
    let mut charts = Vec::new();
    for seed in 0..triangles.len() {
        if assigned[seed] {
            continue;
        }
        let n = normals[seed];
        assigned[seed] = true;
        let mut members = vec![seed];
        let mut queue = VecDeque::from([seed]);
        while let Some(t) = queue.pop_front() {
            for &u in &neighbours[t] {
                if assigned[u] || normals[u].dot(n) < cos_limit {
                    continue;
                }
                assigned[u] = true;
                members.push(u);
                queue.push_back(u);
            }
        }
        members.sort_unstable();

        let chart = make_chart(&members, n, triangles);
        if members.len() > 1 && chart_overlaps(&chart, triangles) {
            for &t in &members {
                charts.push(make_chart(&[t], normals[t], triangles));
            }
        } else {
            charts.push(chart);
        }
    }
    charts
}

/// Flatten `members` onto the plane with normal `n` and measure the result.
fn make_chart(members: &[usize], n: Vec3, triangles: &[[Vec3; 3]]) -> Chart {
    let helper = if n.x.abs() < 0.9 { Vec3::X } else { Vec3::Y };
    let tangent = (helper - n * helper.dot(n)).normalize();
    let bitangent = n.cross(tangent);
    let mut lo = Vec2::splat(f32::MAX);
    let mut hi = Vec2::splat(f32::MIN);
    for &t in members {
        for p in triangles[t] {
            let q = Vec2::new(p.dot(tangent), p.dot(bitangent));
            lo = lo.min(q);
            hi = hi.max(q);
        }
    }
    Chart {
        triangles: members.to_vec(),
        tangent,
        bitangent,
        min: lo,
        size: hi - lo,
    }
}

/// Resolution a grown chart is rasterised at to look for self-overlap.
const OVERLAP_CHECK_RESOLUTION: usize = 256;

/// True when two of the chart's triangles cover one texel centre once
/// flattened. Coarse by design: it only has to catch a patch stacking on
/// itself (a ramp's turns), not a sliver.
fn chart_overlaps(chart: &Chart, triangles: &[[Vec3; 3]]) -> bool {
    let n = OVERLAP_CHECK_RESOLUTION;
    let extent = chart.size.x.max(chart.size.y);
    if extent <= 0.0 {
        return false;
    }
    let scale = (n as f32 - 1.0) / extent;
    let mut owner = vec![u32::MAX; n * n];
    for &t in &chart.triangles {
        let p = triangles[t]
            .map(|v| (Vec2::new(v.dot(chart.tangent), v.dot(chart.bitangent)) - chart.min) * scale);
        let area = (p[1] - p[0]).perp_dot(p[2] - p[0]);
        if area.abs() < 1e-9 {
            continue;
        }
        let lo = p[0].min(p[1]).min(p[2]);
        let hi = p[0].max(p[1]).max(p[2]);
        let x0 = (lo.x - 0.5).floor().max(0.0) as usize;
        let y0 = (lo.y - 0.5).floor().max(0.0) as usize;
        let x1 = ((hi.x + 0.5).ceil().max(0.0) as usize).min(n);
        let y1 = ((hi.y + 0.5).ceil().max(0.0) as usize).min(n);
        for y in y0..y1 {
            for x in x0..x1 {
                let c = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (p[1] - c).perp_dot(p[2] - c) / area;
                let w1 = (p[2] - c).perp_dot(p[0] - c) / area;
                if w0 > 1e-5 && w1 > 1e-5 && 1.0 - w0 - w1 > 1e-5 {
                    let cell = &mut owner[y * n + x];
                    if *cell != u32::MAX && *cell != t as u32 {
                        return true;
                    }
                    *cell = t as u32;
                }
            }
        }
    }
    false
}

/// Shelf-pack rectangles (already padded) into the unit square, in the given
/// order. `None` when they do not fit.
fn shelf_pack(sizes: &[Vec2]) -> Option<Vec<Vec2>> {
    let mut placed = Vec::with_capacity(sizes.len());
    let (mut x, mut y, mut row) = (0.0f32, 0.0f32, 0.0f32);
    for &s in sizes {
        if s.x > 1.0 {
            return None;
        }
        if x + s.x > 1.0 {
            x = 0.0;
            y += row;
            row = 0.0;
        }
        placed.push(Vec2::new(x, y));
        x += s.x;
        row = row.max(s.y);
    }
    (y + row <= 1.0).then_some(placed)
}

/// Choose the global scale and each chart's origin (the corner of its padded
/// rectangle). Returns `(scale, origins, gutter)` in UV units.
fn pack(charts: &[Chart], settings: &UnwrapSettings) -> (f32, Vec<Vec2>, f32) {
    // Tallest first, then widest, then by index: the usual shelf order, with
    // a tie-break that does not depend on anything unordered.
    let mut order: Vec<usize> = (0..charts.len()).collect();
    order.sort_by(|&a, &b| {
        charts[b]
            .size
            .y
            .total_cmp(&charts[a].size.y)
            .then(charts[b].size.x.total_cmp(&charts[a].size.x))
            .then(a.cmp(&b))
    });

    let try_pack = |scale: f32, gutter: f32| {
        let sizes: Vec<Vec2> = order
            .iter()
            .map(|&c| charts[c].size * scale + Vec2::splat(2.0 * gutter))
            .collect();
        shelf_pack(&sizes)
    };

    // A mesh with thousands of tiny charts can fail to fit even at scale 0,
    // where every chart is only its padding. Narrow the gutter until the
    // padding alone fits, rather than collapsing every chart to a point.
    let mut gutter = settings.gutter_texels / settings.atlas_size as f32;
    for _ in 0..8 {
        if try_pack(0.0, gutter).is_some() {
            break;
        }
        gutter *= 0.5;
    }

    let largest = charts
        .iter()
        .map(|c| c.size.x.max(c.size.y))
        .fold(0.0f32, f32::max);
    let (mut lo, mut hi) = (
        0.0f32,
        if largest > 0.0 {
            (1.0 - 2.0 * gutter) / largest
        } else {
            1.0
        },
    );
    if largest > 0.0 {
        for _ in 0..40 {
            let mid = 0.5 * (lo + hi);
            if try_pack(mid, gutter).is_some() {
                lo = mid;
            } else {
                hi = mid;
            }
        }
    } else {
        lo = hi;
    }

    let placed = try_pack(lo, gutter).unwrap_or_else(|| vec![Vec2::ZERO; charts.len()]);
    let mut origins = vec![Vec2::ZERO; charts.len()];
    for (slot, &c) in order.iter().enumerate() {
        origins[c] = placed[slot];
    }
    (lo, origins, gutter)
}

/// Texel-owner buffer for `uv2` at `size`² (`u32::MAX` = empty), or `None`
/// when two different triangles claim one texel centre. A centre exactly on a
/// shared edge is claimed by neither, so neighbouring triangles of one chart
/// do not count as overlapping.
pub(crate) fn rasterise_owners(
    vertices: &[Vertex],
    indices: &[u32],
    size: u32,
) -> Option<Vec<u32>> {
    let n = size as usize;
    let mut owner = vec![u32::MAX; n * n];
    for (t, tri) in indices.chunks_exact(3).enumerate() {
        let p: Vec<Vec2> = tri
            .iter()
            .map(|&i| Vec2::from(vertices[i as usize].uv2) * size as f32)
            .collect();
        let area = (p[1] - p[0]).perp_dot(p[2] - p[0]);
        if area.abs() < 1e-9 {
            continue;
        }
        let lo = p[0].min(p[1]).min(p[2]);
        let hi = p[0].max(p[1]).max(p[2]);
        let x0 = (lo.x - 0.5).floor().max(0.0) as usize;
        let y0 = (lo.y - 0.5).floor().max(0.0) as usize;
        let x1 = ((hi.x + 0.5).ceil().max(0.0) as usize).min(n);
        let y1 = ((hi.y + 0.5).ceil().max(0.0) as usize).min(n);
        for y in y0..y1 {
            for x in x0..x1 {
                let c = Vec2::new(x as f32 + 0.5, y as f32 + 0.5);
                let w0 = (p[1] - c).perp_dot(p[2] - c) / area;
                let w1 = (p[2] - c).perp_dot(p[0] - c) / area;
                let w2 = 1.0 - w0 - w1;
                if w0 > 1e-5 && w1 > 1e-5 && w2 > 1e-5 {
                    let cell = &mut owner[y * n + x];
                    if *cell != u32::MAX && *cell != t as u32 {
                        return None;
                    }
                    *cell = t as u32;
                }
            }
        }
    }
    Some(owner)
}

/// True when every `uv2` lies in `[0,1]²` and no two triangles overlap. The
/// test oracle for [`unwrap`], public so an importer can assert it in debug
/// builds. Zero-area triangles are ignored (they cover no texel).
pub fn uv2_is_valid(vertices: &[Vertex], indices: &[u32]) -> bool {
    let in_range = |v: &Vertex| v.uv2.iter().all(|&c| (-1e-4..=1.0 + 1e-4).contains(&c));
    indices.iter().all(|&i| in_range(&vertices[i as usize]))
        && rasterise_owners(vertices, indices, VALIDATION_RESOLUTION).is_some()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::{
        capsule_vertices, cube_vertices, cylinder_vertices, plane_vertices, sphere_vertices,
    };

    fn primitives() -> Vec<(&'static str, (Vec<Vertex>, Vec<u32>))> {
        vec![
            ("cube", cube_vertices()),
            ("plane", plane_vertices()),
            ("sphere", sphere_vertices()),
            ("cylinder", cylinder_vertices()),
            ("capsule", capsule_vertices()),
        ]
    }

    fn triangle_positions(vertices: &[Vertex], indices: &[u32], t: usize) -> [[f32; 3]; 3] {
        [0, 1, 2].map(|k| vertices[indices[3 * t + k] as usize].position)
    }

    fn world_area(vertices: &[Vertex], indices: &[u32], t: usize) -> f32 {
        let [a, b, c] = triangle_positions(vertices, indices, t).map(Vec3::from);
        0.5 * (b - a).cross(c - a).length()
    }

    fn uv2_area(vertices: &[Vertex], indices: &[u32], t: usize) -> f32 {
        let [a, b, c] = [0, 1, 2].map(|k| Vec2::from(vertices[indices[3 * t + k] as usize].uv2));
        0.5 * (b - a).perp_dot(c - a).abs()
    }

    /// Every primitive unwraps to a valid, geometry-preserving result.
    #[test]
    fn every_primitive_unwraps_without_overlap_and_without_touching_geometry() {
        for (name, (vertices, indices)) in primitives() {
            let (out_v, out_i) = unwrap(&vertices, &indices, &UnwrapSettings::default());
            assert_eq!(out_i.len(), indices.len(), "{name}: triangle count");
            assert!(
                out_v.len() >= vertices.len(),
                "{name}: only ever splits vertices"
            );
            for t in 0..indices.len() / 3 {
                assert_eq!(
                    triangle_positions(&out_v, &out_i, t),
                    triangle_positions(&vertices, &indices, t),
                    "{name}: triangle {t} keeps its position and winding"
                );
            }
            for (v, source) in out_i.iter().zip(&indices) {
                let (o, s) = (&out_v[*v as usize], &vertices[*source as usize]);
                assert_eq!(
                    (o.color, o.normal, o.uv),
                    (s.color, s.normal, s.uv),
                    "{name}"
                );
            }
            assert!(
                uv2_is_valid(&out_v, &out_i),
                "{name}: uv2 overlaps or leaves [0,1]"
            );
        }
    }

    /// The oracle must reject the things the unwrap exists to avoid, or the
    /// test above passes for a do-nothing unwrap that leaves every uv2 at the
    /// origin.
    #[test]
    fn the_oracle_rejects_overlap_and_out_of_range() {
        let (vertices, indices) = cube_vertices();
        let (mut v, i) = unwrap(&vertices, &indices, &UnwrapSettings::default());
        assert!(uv2_is_valid(&v, &i), "premise: the unwrapped cube is valid");

        // Every triangle stacked on one spot: overlap.
        let stacked: Vec<Vertex> = v
            .iter()
            .map(|x| Vertex {
                uv2: [0.1, 0.1],
                ..*x
            })
            .collect();
        // All-equal uv2 makes zero-area triangles, which cover no texel, so
        // give the first triangle of two different faces (different charts, so
        // no shared vertices to overwrite) real, identical area.
        let mut overlapped = stacked;
        for k in 0..3 {
            overlapped[i[k] as usize].uv2 = [[0.1, 0.1], [0.3, 0.1], [0.1, 0.3]][k];
            overlapped[i[6 + k] as usize].uv2 = [[0.1, 0.1], [0.3, 0.1], [0.1, 0.3]][k];
        }
        assert!(
            !uv2_is_valid(&overlapped, &i),
            "two stacked triangles overlap"
        );

        v[i[0] as usize].uv2 = [1.5, 0.5];
        assert!(!uv2_is_valid(&v, &i), "a coordinate past 1 is out of range");
    }

    /// One global scale: a texel covers the same world area in every chart.
    /// Two planes in one mesh, one twice the size of the other.
    #[test]
    fn texel_density_is_the_same_in_every_chart() {
        let (mut vertices, mut indices) = plane_vertices();
        let offset = vertices.len() as u32;
        for mut v in plane_vertices().0 {
            v.position = [
                v.position[0] * 2.0 + 3.0,
                v.position[1],
                v.position[2] * 2.0,
            ];
            vertices.push(v);
        }
        indices.extend(plane_vertices().1.iter().map(|i| i + offset));

        let (out_v, out_i) = unwrap(&vertices, &indices, &UnwrapSettings::default());
        let triangles = out_i.len() / 3;
        let ratios: Vec<f32> = (0..triangles)
            .map(|t| uv2_area(&out_v, &out_i, t) / world_area(&out_v, &out_i, t))
            .collect();
        // Premise: the big plane really is bigger, so a per-chart normalised
        // unwrap (every chart filling its slot) would show different ratios.
        let big = world_area(&out_v, &out_i, triangles - 1);
        let small = world_area(&out_v, &out_i, 0);
        assert!(
            (big / small - 4.0).abs() < 1e-3,
            "premise: {big} vs {small}"
        );
        for r in &ratios {
            assert!(
                (r / ratios[0] - 1.0).abs() < 0.01,
                "densities differ: {ratios:?}"
            );
        }
    }

    /// Charts do not touch: no texel of one chart is next to a texel of
    /// another. On a cube each face is a chart of two triangles.
    #[test]
    fn charts_keep_a_gutter_between_them() {
        let (vertices, indices) = cube_vertices();
        let touching = |settings: &UnwrapSettings| {
            let (v, i) = unwrap(&vertices, &indices, settings);
            let n = 1024usize;
            let owner = rasterise_owners(&v, &i, n as u32).expect("valid");
            let mut count = 0;
            for y in 0..n {
                for x in 0..n - 1 {
                    for (a, b) in [(owner[y * n + x], owner[y * n + x + 1])] {
                        if a != u32::MAX && b != u32::MAX && a / 2 != b / 2 {
                            count += 1;
                        }
                    }
                }
            }
            for y in 0..n - 1 {
                for x in 0..n {
                    let (a, b) = (owner[y * n + x], owner[(y + 1) * n + x]);
                    if a != u32::MAX && b != u32::MAX && a / 2 != b / 2 {
                        count += 1;
                    }
                }
            }
            count
        };
        assert_eq!(
            touching(&UnwrapSettings::default()),
            0,
            "gutter keeps charts apart"
        );
        let none = UnwrapSettings {
            gutter_texels: 0.0,
            ..UnwrapSettings::default()
        };
        assert!(
            touching(&none) > 0,
            "premise: with no gutter the charts do touch"
        );
    }

    #[test]
    fn the_same_mesh_unwraps_to_the_same_bytes() {
        for (name, (vertices, indices)) in primitives() {
            let a = unwrap(&vertices, &indices, &UnwrapSettings::default());
            let b = unwrap(&vertices, &indices, &UnwrapSettings::default());
            assert_eq!(a, b, "{name}");
        }
    }

    /// A two-turn spiral ramp: every normal points within a few degrees of
    /// straight up, so the angle cap alone puts the whole ramp in one chart,
    /// and flattened from above its turns land on each other. The overlap
    /// fallback is what makes the result valid.
    #[test]
    fn a_ramp_whose_turns_stack_in_projection_still_unwraps_without_overlap() {
        let (steps, turns) = (96usize, 2.0f32);
        let vertex = |x: f32, y: f32, z: f32| Vertex {
            position: [x, y, z],
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [0.0; 2],
            uv2: [0.0; 2],
        };
        let mut vertices = Vec::new();
        for s in 0..=steps {
            let a = std::f32::consts::TAU * turns * s as f32 / steps as f32;
            let rise = 0.02 * a;
            vertices.push(vertex(1.0 * a.cos(), rise, 1.0 * a.sin()));
            vertices.push(vertex(2.0 * a.cos(), rise, 2.0 * a.sin()));
        }
        let mut indices = Vec::new();
        for s in 0..steps as u32 {
            let b = 2 * s;
            indices.extend([b, b + 1, b + 3, b, b + 3, b + 2]);
        }
        // Premise: both orientations are present or the ramp faces up; and
        // the normals really are all within the default cap of one another,
        // so a single chart is what the angle cap alone would build.
        let normal = |t: usize| {
            let p = |k: usize| Vec3::from(vertices[indices[3 * t + k] as usize].position);
            (p(1) - p(0)).cross(p(2) - p(0)).normalize()
        };
        let first = normal(0);
        for t in 0..indices.len() / 3 {
            assert!(
                normal(t).dot(first) > 0.9,
                "premise: triangle {t} within the angle cap of triangle 0"
            );
        }
        let (v, i) = unwrap(&vertices, &indices, &UnwrapSettings::default());
        assert!(uv2_is_valid(&v, &i), "the spiral unwraps without overlap");
    }
}
