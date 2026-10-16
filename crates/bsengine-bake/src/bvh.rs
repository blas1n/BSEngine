//! A bounding volume hierarchy over triangles, for the rays the baker casts.
//!
//! Binned SAH build (Wald 2007; what pbrt, Embree and Godot's lightmapper all
//! use in some form), flattened into an array, traversed with an explicit
//! stack. Nothing clever: the baker casts tens of millions of rays per bake,
//! so the tree has to be good, but it is built once per bake and never
//! refitted.

use glam::Vec3;

/// A triangle's corners, as the tree stores them.
pub type Tri = [Vec3; 3];

/// Where a ray hit: the distance along it, which triangle, and the
/// barycentric coordinates of the hit inside that triangle (weights of
/// corners 1 and 2; corner 0 gets the rest).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    pub t: f32,
    pub triangle: u32,
    pub b1: f32,
    pub b2: f32,
}

#[derive(Debug, Clone, Copy)]
struct Aabb {
    min: Vec3,
    max: Vec3,
}

impl Aabb {
    const EMPTY: Self = Self {
        min: Vec3::splat(f32::INFINITY),
        max: Vec3::splat(f32::NEG_INFINITY),
    };

    fn grow(&mut self, p: Vec3) {
        self.min = self.min.min(p);
        self.max = self.max.max(p);
    }

    fn merge(&mut self, o: &Aabb) {
        self.min = self.min.min(o.min);
        self.max = self.max.max(o.max);
    }

    fn area(&self) -> f32 {
        let d = (self.max - self.min).max(Vec3::ZERO);
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Entry distance of the ray into the box, or `None` if it misses it
    /// within `t_max`. The slab test, with `inv_dir` precomputed.
    fn hit(&self, origin: Vec3, inv_dir: Vec3, t_max: f32) -> Option<f32> {
        let t0 = (self.min - origin) * inv_dir;
        let t1 = (self.max - origin) * inv_dir;
        let near = t0.min(t1).max_element().max(0.0);
        let far = t0.max(t1).min_element().min(t_max);
        (near <= far).then_some(near)
    }
}

#[derive(Debug, Clone, Copy)]
struct Node {
    bounds: Aabb,
    /// For an interior node, the index of its second child (the first is the
    /// next node in the array); for a leaf, the first entry in `order`.
    start: u32,
    /// Number of triangles for a leaf; 0 for an interior node.
    count: u32,
}

/// The tree. Triangles are referenced, in `order`, by their index in the
/// slice the tree was built from.
pub struct Bvh {
    nodes: Vec<Node>,
    order: Vec<u32>,
    tris: Vec<Tri>,
}

/// Leaves stop splitting at this many triangles: below it a split costs more
/// to traverse than it saves.
const LEAF_SIZE: usize = 4;
/// Candidate split planes per axis for the SAH.
const BINS: usize = 12;

impl Bvh {
    pub fn build(tris: &[Tri]) -> Self {
        let mut bvh = Self {
            nodes: Vec::with_capacity(tris.len() * 2),
            order: (0..tris.len() as u32).collect(),
            tris: tris.to_vec(),
        };
        if tris.is_empty() {
            return bvh;
        }
        let centroids: Vec<Vec3> = tris.iter().map(|t| (t[0] + t[1] + t[2]) / 3.0).collect();
        bvh.build_node(0, tris.len(), &centroids);
        bvh
    }

    fn bounds_of(&self, start: usize, end: usize) -> Aabb {
        let mut b = Aabb::EMPTY;
        for &i in &self.order[start..end] {
            for p in self.tris[i as usize] {
                b.grow(p);
            }
        }
        b
    }

    /// Builds the node for `order[start..end]`, returning its index.
    fn build_node(&mut self, start: usize, end: usize, centroids: &[Vec3]) -> usize {
        let index = self.nodes.len();
        let bounds = self.bounds_of(start, end);
        self.nodes.push(Node {
            bounds,
            start: start as u32,
            count: (end - start) as u32,
        });
        let count = end - start;
        if count <= LEAF_SIZE {
            return index;
        }

        let mut cb = Aabb::EMPTY;
        for &i in &self.order[start..end] {
            cb.grow(centroids[i as usize]);
        }
        let extent = cb.max - cb.min;

        // The cheapest split over every axis and bin boundary, by surface
        // area times triangle count on each side.
        let mut best: Option<(usize, f32, f32)> = None; // (axis, plane, cost)
        for axis in 0..3 {
            if extent[axis] <= 1e-12 {
                continue;
            }
            let mut bins = [(Aabb::EMPTY, 0usize); BINS];
            let bin_of = |c: Vec3| {
                (((c[axis] - cb.min[axis]) / extent[axis]) * BINS as f32).min(BINS as f32 - 1.0)
                    as usize
            };
            for &i in &self.order[start..end] {
                let b = &mut bins[bin_of(centroids[i as usize])];
                for p in self.tris[i as usize] {
                    b.0.grow(p);
                }
                b.1 += 1;
            }
            for split in 1..BINS {
                let (mut l, mut r) = (Aabb::EMPTY, Aabb::EMPTY);
                let (mut nl, mut nr) = (0, 0);
                for (k, (b, n)) in bins.iter().enumerate() {
                    if *n == 0 {
                        continue;
                    }
                    if k < split {
                        l.merge(b);
                        nl += n;
                    } else {
                        r.merge(b);
                        nr += n;
                    }
                }
                if nl == 0 || nr == 0 {
                    continue;
                }
                let cost = l.area() * nl as f32 + r.area() * nr as f32;
                if best.is_none_or(|(_, _, c)| cost < c) {
                    let plane = cb.min[axis] + extent[axis] * split as f32 / BINS as f32;
                    best = Some((axis, plane, cost));
                }
            }
        }

        // Every centroid in one place (coincident triangles): no plane
        // separates them, so this is a leaf however many it holds.
        let Some((axis, plane, cost)) = best else {
            return index;
        };
        if cost >= bounds.area() * count as f32 {
            return index;
        }
        // Partition `order[start..end]` by the chosen plane.
        let mut mid = start;
        for k in start..end {
            if centroids[self.order[k] as usize][axis] < plane {
                self.order.swap(k, mid);
                mid += 1;
            }
        }
        if mid == start || mid == end {
            return index;
        }
        self.build_node(start, mid, centroids);
        let right = self.build_node(mid, end, centroids);
        self.nodes[index].start = right as u32;
        self.nodes[index].count = 0;
        index
    }

    /// The tree as the GPU backend uploads it: every node as `(min, start,
    /// max, count)` in array order -- an interior node (`count == 0`) has its
    /// first child next and its second at `start`; a leaf's triangles are
    /// `order[start..start + count]` -- and that order. The GPU traversal is
    /// this one, so the two trace the same tree.
    pub(crate) fn flat(&self) -> (Vec<([f32; 3], u32, [f32; 3], u32)>, &[u32]) {
        let nodes = self
            .nodes
            .iter()
            .map(|n| {
                (
                    n.bounds.min.to_array(),
                    n.start,
                    n.bounds.max.to_array(),
                    n.count,
                )
            })
            .collect();
        (nodes, &self.order)
    }

    /// The nearest hit along `origin + t * dir` for `t` in `(0, t_max)`.
    /// Both faces count: the caller decides what a back face means.
    pub fn closest(&self, origin: Vec3, dir: Vec3, t_max: f32) -> Option<Hit> {
        self.traverse(origin, dir, t_max, false)
    }

    /// Whether anything at all lies along the ray within `t_max` -- a shadow
    /// ray, which can stop at the first hit.
    pub fn occluded(&self, origin: Vec3, dir: Vec3, t_max: f32) -> bool {
        self.traverse(origin, dir, t_max, true).is_some()
    }

    fn traverse(&self, origin: Vec3, dir: Vec3, t_max: f32, any: bool) -> Option<Hit> {
        if self.nodes.is_empty() {
            return None;
        }
        let inv_dir = dir.recip();
        let mut best: Option<Hit> = None;
        let mut limit = t_max;
        let mut stack = [0u32; 64];
        let mut top = 1;
        while top > 0 {
            top -= 1;
            let index = stack[top];
            let node = &self.nodes[index as usize];
            if node.bounds.hit(origin, inv_dir, limit).is_none() {
                continue;
            }
            if node.count > 0 {
                for &i in &self.order[node.start as usize..(node.start + node.count) as usize] {
                    if let Some((t, b1, b2)) = intersect(&self.tris[i as usize], origin, dir) {
                        if t < limit {
                            limit = t;
                            best = Some(Hit {
                                t,
                                triangle: i,
                                b1,
                                b2,
                            });
                            if any {
                                return best;
                            }
                        }
                    }
                }
            } else {
                // Interior: push both children; the first child is the next
                // node, the second is `start`.
                stack[top] = index + 1;
                stack[top + 1] = node.start;
                top += 2;
            }
        }
        best
    }
}

/// Möller–Trumbore. `(t, b1, b2)` for a hit in front of the origin.
fn intersect(tri: &Tri, origin: Vec3, dir: Vec3) -> Option<(f32, f32, f32)> {
    let e1 = tri[1] - tri[0];
    let e2 = tri[2] - tri[0];
    let p = dir.cross(e2);
    let det = e1.dot(p);
    if det.abs() < 1e-12 {
        return None;
    }
    let inv = 1.0 / det;
    let s = origin - tri[0];
    let b1 = s.dot(p) * inv;
    if !(0.0..=1.0).contains(&b1) {
        return None;
    }
    let q = s.cross(e1);
    let b2 = dir.dot(q) * inv;
    if b2 < 0.0 || b1 + b2 > 1.0 {
        return None;
    }
    let t = e2.dot(q) * inv;
    (t > 1e-6).then_some((t, b1, b2))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sample::Rng;

    fn random_tris(n: usize, rng: &mut Rng) -> Vec<Tri> {
        (0..n)
            .map(|_| {
                let c = Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) * 20.0 - 10.0;
                let corner = |rng: &mut Rng| {
                    c + Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) * 2.0 - 1.0
                };
                [corner(rng), corner(rng), corner(rng)]
            })
            .collect()
    }

    /// The tree answers exactly what testing every triangle answers: the same
    /// nearest triangle at the same distance, for a thousand random rays
    /// through two thousand random triangles. The brute force is the oracle;
    /// a tree that skipped a subtree it should have entered fails here.
    #[test]
    fn the_tree_finds_what_brute_force_finds() {
        let mut rng = Rng::new(7);
        let tris = random_tris(2000, &mut rng);
        let bvh = Bvh::build(&tris);
        assert!(bvh.nodes.len() > 1, "premise: the tree actually split");
        let mut hits = 0;
        for _ in 0..1000 {
            let origin = Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) * 30.0 - 15.0;
            let dir =
                (Vec3::new(rng.next_f32(), rng.next_f32(), rng.next_f32()) * 2.0 - 1.0).normalize();
            let brute = tris
                .iter()
                .enumerate()
                .filter_map(|(i, t)| intersect(t, origin, dir).map(|(d, _, _)| (d, i as u32)))
                .min_by(|a, b| a.0.total_cmp(&b.0));
            let tree = bvh.closest(origin, dir, f32::INFINITY);
            assert_eq!(
                tree.map(|h| (h.t, h.triangle)),
                brute,
                "ray from {origin} along {dir}"
            );
            assert_eq!(bvh.occluded(origin, dir, f32::INFINITY), brute.is_some());
            hits += brute.is_some() as u32;
        }
        assert!(hits > 100, "premise: enough rays hit something ({hits})");
    }

    #[test]
    fn a_hit_beyond_t_max_is_not_a_hit() {
        let tri = [
            Vec3::new(-1.0, -1.0, 5.0),
            Vec3::new(1.0, -1.0, 5.0),
            Vec3::new(0.0, 1.0, 5.0),
        ];
        let bvh = Bvh::build(&[tri]);
        assert!(bvh.closest(Vec3::ZERO, Vec3::Z, 10.0).is_some());
        assert!(bvh.closest(Vec3::ZERO, Vec3::Z, 4.0).is_none());
        assert!(!bvh.occluded(Vec3::ZERO, Vec3::Z, 4.0));
    }

    #[test]
    fn an_empty_tree_hits_nothing() {
        let bvh = Bvh::build(&[]);
        assert!(bvh.closest(Vec3::ZERO, Vec3::Z, f32::INFINITY).is_none());
    }
}
