//! Where an adaptive light-probe volume puts its probes: bricks of 4 x 4 x 4,
//! fine near geometry and coarse in open space -- Unity's Adaptive Probe
//! Volumes, Unreal's Volumetric Lightmap and Godot's LightmapGI all place
//! probes this way, instead of one uniform grid that is either coarse
//! everywhere or spends its budget on empty air.
//!
//! A brick's 64 probes sit on its corners and on the lattice between them,
//! three cells per axis. Each level is three times finer than the one above
//! it (Unity's ratio), so a child brick spans exactly one cell of its parent's
//! lattice, and every probe of a coarse level is also a lattice point of the
//! finer ones. Probes are kept once per lattice point: bricks side by side
//! share their face probes, so there is no seam within a level; across a
//! level change the shared points agree and only the finer points between
//! them differ (the T-junction Unity and Unreal accept too).
//!
//! The volume's box is first cut into a grid of the coarsest bricks. A brick
//! is cut into its 27 children while it is coarser than the finest level and
//! some triangle overlaps it, grown by half its probe spacing so a surface on
//! a brick's face refines the bricks on both sides. Levels are cut whole, the
//! coarsest first; a level that would take the volume past its probe budget
//! is not cut at all, so a small budget gives an even, coarser layout rather
//! than one that is fine in whatever corner was visited first.
//!
//! Lookup ([`BrickLayout::locate`], and the same walk in the shader) goes
//! through a tree: one node per top-level brick, each either a brick or a
//! group of 27 child nodes.

use glam::Vec3;
use std::collections::HashMap;

/// Probes per brick edge; a brick has `BRICK_PROBES^3`.
pub const BRICK_PROBES: u32 = 4;
/// Cells per brick edge, and how much finer each level is than the one above.
pub const BRICK_CELLS: u32 = BRICK_PROBES - 1;
/// Most levels a layout has: from the coarsest spacing to 27 times finer.
pub const MAX_LEVELS: u32 = 4;
/// A node that is a brick: the low bits are its index in
/// [`BrickLayout::bricks`]. A node without it is a group: the low bits are
/// the index of the first of its 27 children in [`BrickLayout::nodes`].
pub const LEAF: u32 = 1 << 31;

/// What a layout is asked for: the box, how fine and how coarse its probes
/// may be, and how many it may have.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BrickSettings {
    /// World-space minimum corner of the box.
    pub origin: Vec3,
    /// World-space size of the box (full extent).
    pub extent: Vec3,
    /// The finest probe spacing, near geometry.
    pub min_spacing: f32,
    /// The coarsest probe spacing, in open space. The box is cut into whole
    /// bricks, so the spacing used is this or a little less.
    pub max_spacing: f32,
    /// Most probes the layout may have.
    pub max_probes: usize,
    /// Most bricks the layout may have.
    pub max_bricks: usize,
}

/// One brick: where it is, its level (0 coarsest) and its 64 probes, x
/// fastest, then y, then z.
#[derive(Clone, Debug, PartialEq)]
pub struct Brick {
    /// World-space minimum corner.
    pub lo: Vec3,
    /// World-space size; its probes are `size / 3` apart.
    pub size: Vec3,
    /// 0 for the coarsest level.
    pub level: u32,
    /// Indices into [`BrickLayout::probes`].
    pub probes: [u32; (BRICK_PROBES * BRICK_PROBES * BRICK_PROBES) as usize],
}

/// One probe: where it is, and the spacing of the finest brick that uses it
/// -- how far it may be moved from there (DDGI's relocation keeps a probe
/// within 0.45 of it) and how far its rays need to see.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// World-space position.
    pub position: Vec3,
    /// The finest probe spacing of the bricks it belongs to.
    pub spacing: Vec3,
}

/// Where a volume's probes are and how to find the ones around a point.
#[derive(Clone, Debug, PartialEq)]
pub struct BrickLayout {
    /// World-space minimum corner of the box.
    pub origin: Vec3,
    /// World-space size of the box.
    pub extent: Vec3,
    /// Top-level bricks per axis.
    pub top: [u32; 3],
    /// Levels the layout may use (the finest is `levels - 1`).
    pub levels: u32,
    /// The tree: the first `top` product are the top-level bricks' nodes, x
    /// fastest; see [`LEAF`].
    pub nodes: Vec<u32>,
    /// Every brick.
    pub bricks: Vec<Brick>,
    /// Every probe, once.
    pub probes: Vec<Probe>,
    /// Whether the budget stopped a level from being cut: the layout is
    /// coarser near geometry than `min_spacing` asked for.
    pub truncated: bool,
    /// Whether even the top level did not fit the budget at `max_spacing`,
    /// so the whole volume is coarser than that.
    pub coarsened: bool,
}

/// A brick while the layout is cut: its level, its integer position at that
/// level, and the triangles that overlap it.
struct Cut {
    level: u32,
    at: [u32; 3],
    tris: Vec<u32>,
}

impl BrickLayout {
    /// The layout for `settings` around `triangles` (world space).
    pub fn build(settings: &BrickSettings, triangles: &[[Vec3; 3]]) -> Self {
        let extent = settings.extent.max(Vec3::splat(1e-3));
        let mut max_spacing = settings.max_spacing.max(1e-3);
        let min_spacing = settings.min_spacing.clamp(1e-3, max_spacing);
        let top_for = |spacing: f32| {
            [0, 1, 2].map(|a| ((extent[a] / (BRICK_CELLS as f32 * spacing)).ceil() as u32).max(1))
        };
        // The top level alone must fit -- its bricks, and their shared
        // lattice, `3 n + 1` probes an axis. A box too large for the budget at
        // `max_spacing` is cut coarser instead, as a grid's resolution is
        // clamped rather than refused.
        let fits = |t: [u32; 3]| {
            t.iter().map(|&n| n as usize).product::<usize>() <= settings.max_bricks.max(1)
                && t.iter()
                    .map(|&n| (BRICK_CELLS * n + 1) as usize)
                    .product::<usize>()
                    <= settings.max_probes.max(BRICK_PROBES.pow(3) as usize)
        };
        let mut top = top_for(max_spacing);
        let coarsened = !fits(top);
        while !fits(top) {
            max_spacing *= 1.25;
            top = top_for(max_spacing);
        }
        let brick0 = extent / Vec3::new(top[0] as f32, top[1] as f32, top[2] as f32);
        // The coarsest spacing actually used, and how many times it can be
        // cut by three before it is finer than asked for.
        let spacing0 = (brick0 / BRICK_CELLS as f32).min_element();
        let mut levels = 1;
        while levels < MAX_LEVELS
            && spacing0 / (BRICK_CELLS.pow(levels) as f32) >= min_spacing * (1.0 - 1e-4)
        {
            levels += 1;
        }
        let layout = Layout {
            origin: settings.origin,
            brick0,
            levels,
        };

        let all: Vec<u32> = (0..triangles.len() as u32).collect();
        let mut leaves: Vec<Cut> = Vec::new();
        for z in 0..top[2] {
            for y in 0..top[1] {
                for x in 0..top[0] {
                    let at = [x, y, z];
                    let tris = layout.overlapping(0, at, &all, triangles);
                    leaves.push(Cut { level: 0, at, tris });
                }
            }
        }
        let mut truncated = false;
        for level in 0..levels - 1 {
            if !leaves
                .iter()
                .any(|c| c.level == level && !c.tris.is_empty())
            {
                break;
            }
            let mut next = Vec::with_capacity(leaves.len());
            for cut in &leaves {
                if cut.level != level || cut.tris.is_empty() {
                    next.push(Cut {
                        level: cut.level,
                        at: cut.at,
                        tris: cut.tris.clone(),
                    });
                    continue;
                }
                for c in 0..BRICK_CELLS.pow(3) {
                    let at = [
                        cut.at[0] * BRICK_CELLS + c % BRICK_CELLS,
                        cut.at[1] * BRICK_CELLS + (c / BRICK_CELLS) % BRICK_CELLS,
                        cut.at[2] * BRICK_CELLS + c / (BRICK_CELLS * BRICK_CELLS),
                    ];
                    let tris = layout.overlapping(level + 1, at, &cut.tris, triangles);
                    next.push(Cut {
                        level: level + 1,
                        at,
                        tris,
                    });
                }
            }
            if next.len() > settings.max_bricks || layout.probe_count(&next) > settings.max_probes {
                truncated = true;
                break;
            }
            leaves = next;
        }
        let mut out = layout.assemble(settings, top, truncated, &leaves);
        out.coarsened = coarsened;
        out
    }

    /// The brick `p` is in and `p`'s place in it, in its cells (0..=3 per
    /// axis); a point outside the box is clamped onto it. `None` for an empty
    /// layout.
    pub fn locate(&self, p: Vec3) -> Option<(u32, Vec3)> {
        if self.nodes.is_empty() {
            return None;
        }
        let top = Vec3::new(self.top[0] as f32, self.top[1] as f32, self.top[2] as f32);
        let t = ((p - self.origin) / self.extent.max(Vec3::splat(1e-6)))
            .clamp(Vec3::ZERO, Vec3::ONE)
            * top;
        let cell = t.floor().min(top - Vec3::ONE).max(Vec3::ZERO);
        let mut f = t - cell;
        let mut node = self.nodes[(cell.z as u32 * self.top[1] + cell.y as u32) as usize
            * self.top[0] as usize
            + cell.x as usize];
        // At most `MAX_LEVELS` steps: a tree that loops is a bug, not a hang.
        for _ in 0..MAX_LEVELS {
            if node & LEAF != 0 {
                return Some((node & !LEAF, f * BRICK_CELLS as f32));
            }
            let g = (f * BRICK_CELLS as f32)
                .floor()
                .clamp(Vec3::ZERO, Vec3::splat(2.0));
            f = f * BRICK_CELLS as f32 - g;
            let child = g.x as u32 + BRICK_CELLS * (g.y as u32 + BRICK_CELLS * g.z as u32);
            node = self.nodes[(node + child) as usize];
        }
        (node & LEAF != 0).then(|| (node & !LEAF, f * BRICK_CELLS as f32))
    }
}

/// The geometry every brick of a layout is cut from.
struct Layout {
    origin: Vec3,
    /// A top-level brick's size.
    brick0: Vec3,
    levels: u32,
}

impl Layout {
    /// A level's brick size.
    fn brick(&self, level: u32) -> Vec3 {
        self.brick0 / BRICK_CELLS.pow(level) as f32
    }

    /// How many finest-level lattice cells one lattice cell of `level` spans.
    fn stride(&self, level: u32) -> u32 {
        BRICK_CELLS.pow(self.levels - 1 - level)
    }

    /// The brick at `at` of `level`, and the triangles of `candidates` that
    /// overlap it grown by half its probe spacing.
    fn overlapping(
        &self,
        level: u32,
        at: [u32; 3],
        candidates: &[u32],
        triangles: &[[Vec3; 3]],
    ) -> Vec<u32> {
        let size = self.brick(level);
        let lo = self.origin + size * Vec3::new(at[0] as f32, at[1] as f32, at[2] as f32);
        let grow = size / BRICK_CELLS as f32 * 0.5;
        let centre = lo + size * 0.5;
        let half = size * 0.5 + grow;
        candidates
            .iter()
            .copied()
            .filter(|&t| triangle_overlaps_box(&triangles[t as usize], centre, half))
            .collect()
    }

    /// A brick's probes as finest-level lattice keys, x fastest.
    fn keys(&self, level: u32, at: [u32; 3]) -> impl Iterator<Item = [u32; 3]> {
        let s = self.stride(level);
        let base = at.map(|a| a * BRICK_CELLS * s);
        (0..BRICK_PROBES.pow(3)).map(move |i| {
            [
                base[0] + (i % BRICK_PROBES) * s,
                base[1] + (i / BRICK_PROBES % BRICK_PROBES) * s,
                base[2] + (i / (BRICK_PROBES * BRICK_PROBES)) * s,
            ]
        })
    }

    fn probe_count(&self, leaves: &[Cut]) -> usize {
        let mut seen = std::collections::HashSet::new();
        for cut in leaves {
            seen.extend(self.keys(cut.level, cut.at));
        }
        seen.len()
    }

    fn assemble(
        &self,
        settings: &BrickSettings,
        top: [u32; 3],
        truncated: bool,
        leaves: &[Cut],
    ) -> BrickLayout {
        let finest = self.brick(self.levels - 1) / BRICK_CELLS as f32;
        let mut index: HashMap<[u32; 3], u32> = HashMap::new();
        let mut probes: Vec<Probe> = Vec::new();
        let mut bricks = Vec::with_capacity(leaves.len());
        // Unset nodes are filled as the leaves are walked in; every node ends
        // set, since a cut brick is replaced by all 27 of its children.
        const UNSET: u32 = u32::MAX;
        let mut nodes = vec![UNSET; (top[0] * top[1] * top[2]) as usize];
        for cut in leaves {
            let spacing = self.brick(cut.level) / BRICK_CELLS as f32;
            let mut ids = [0u32; (BRICK_PROBES * BRICK_PROBES * BRICK_PROBES) as usize];
            for (slot, key) in self.keys(cut.level, cut.at).enumerate() {
                let id = *index.entry(key).or_insert_with(|| {
                    probes.push(Probe {
                        position: self.origin
                            + finest * Vec3::new(key[0] as f32, key[1] as f32, key[2] as f32),
                        spacing,
                    });
                    probes.len() as u32 - 1
                });
                let p = &mut probes[id as usize];
                p.spacing = p.spacing.min(spacing);
                ids[slot] = id;
            }
            let brick = bricks.len() as u32;
            let size = self.brick(cut.level);
            bricks.push(Brick {
                lo: self.origin
                    + size * Vec3::new(cut.at[0] as f32, cut.at[1] as f32, cut.at[2] as f32),
                size,
                level: cut.level,
                probes: ids,
            });

            // Down from the top-level brick to this one, making groups on
            // the way.
            let shift = BRICK_CELLS.pow(cut.level);
            let root = cut.at.map(|a| a / shift);
            let mut slot = ((root[2] * top[1] + root[1]) * top[0] + root[0]) as usize;
            for l in 1..=cut.level {
                if nodes[slot] == UNSET {
                    nodes[slot] = nodes.len() as u32;
                    nodes.extend(std::iter::repeat_n(UNSET, BRICK_CELLS.pow(3) as usize));
                }
                let d = BRICK_CELLS.pow(cut.level - l);
                let g = cut.at.map(|a| a / d % BRICK_CELLS);
                slot = nodes[slot] as usize
                    + (g[0] + BRICK_CELLS * (g[1] + BRICK_CELLS * g[2])) as usize;
            }
            nodes[slot] = brick | LEAF;
        }
        debug_assert!(!nodes.contains(&UNSET), "every node is set");
        BrickLayout {
            origin: settings.origin,
            extent: settings.extent.max(Vec3::splat(1e-3)),
            top,
            levels: self.levels,
            nodes,
            bricks,
            probes,
            truncated,
            coarsened: false,
        }
    }
}

/// Whether a triangle overlaps a box (`centre`, `half` extents): the
/// separating-axis test of Akenine-Moller -- the box's three axes, the
/// triangle's normal, and the nine cross products of their edges.
pub fn triangle_overlaps_box(tri: &[Vec3; 3], centre: Vec3, half: Vec3) -> bool {
    let v = tri.map(|p| p - centre);
    let e = [v[1] - v[0], v[2] - v[1], v[0] - v[2]];
    let separated = |axis: Vec3| {
        if axis.length_squared() < 1e-20 {
            return false;
        }
        let p = v.map(|q| q.dot(axis));
        let r = half.x * axis.x.abs() + half.y * axis.y.abs() + half.z * axis.z.abs();
        p.iter().copied().fold(f32::INFINITY, f32::min) > r
            || p.iter().copied().fold(f32::NEG_INFINITY, f32::max) < -r
    };
    for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
        if separated(axis) {
            return false;
        }
    }
    if separated(e[0].cross(e[1])) {
        return false;
    }
    for edge in e {
        for axis in [Vec3::X, Vec3::Y, Vec3::Z] {
            if separated(edge.cross(axis)) {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings(extent: Vec3, min: f32, max: f32, max_probes: usize) -> BrickSettings {
        BrickSettings {
            origin: Vec3::ZERO,
            extent,
            min_spacing: min,
            max_spacing: max,
            max_probes,
            max_bricks: 1_000_000,
        }
    }

    /// A floor across the box at height `y`: two triangles.
    fn floor(extent: Vec3, y: f32) -> Vec<[Vec3; 3]> {
        let (a, b, c, d) = (
            Vec3::new(0.0, y, 0.0),
            Vec3::new(extent.x, y, 0.0),
            Vec3::new(extent.x, y, extent.z),
            Vec3::new(0.0, y, extent.z),
        );
        vec![[a, b, c], [a, c, d]]
    }

    #[test]
    fn the_overlap_test_finds_what_touches_the_box_and_nothing_else() {
        let tri = [
            Vec3::new(-1.0, 0.0, -1.0),
            Vec3::new(1.0, 0.0, -1.0),
            Vec3::new(0.0, 0.0, 1.0),
        ];
        assert!(triangle_overlaps_box(&tri, Vec3::ZERO, Vec3::splat(0.1)));
        assert!(!triangle_overlaps_box(
            &tri,
            Vec3::new(0.0, 0.2, 0.0),
            Vec3::splat(0.1)
        ));
        // Beside the triangle's slanted edge, inside its bounding box: only
        // an edge cross product separates them.
        assert!(!triangle_overlaps_box(
            &tri,
            Vec3::new(0.9, 0.0, 0.8),
            Vec3::splat(0.1)
        ));
        assert!(triangle_overlaps_box(
            &tri,
            Vec3::new(0.4, 0.05, 0.0),
            Vec3::splat(0.1)
        ));
        // A slanted triangle whose bounding box holds the box: only its
        // normal (1, 1, 1) separates them.
        let slanted = [
            Vec3::new(3.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
            Vec3::new(0.0, 0.0, 3.0),
        ];
        assert!(!triangle_overlaps_box(
            &slanted,
            Vec3::ZERO,
            Vec3::splat(0.5)
        ));
        assert!(triangle_overlaps_box(
            &slanted,
            Vec3::splat(1.0),
            Vec3::splat(0.5)
        ));
        // A narrow triangle pointing at the box from +x, its tip 0.01 past
        // the box's face: no edge of it separates them -- only the box's x.
        let tip = [
            Vec3::new(0.11, 0.0, 0.0),
            Vec3::new(1.0, 0.6, 0.0),
            Vec3::new(1.3, -0.6, 0.0),
        ];
        assert!(!triangle_overlaps_box(&tip, Vec3::ZERO, Vec3::splat(0.1)));
    }

    /// A floor just past the face between two bricks refines both: the
    /// probes on either side of it are the ones that light it, and the lower
    /// brick reaches it only through the half spacing it is grown by. (On
    /// the face itself both boxes touch it, grown or not.)
    #[test]
    fn a_surface_by_a_bricks_face_refines_both_sides() {
        let extent = Vec3::new(12.0, 24.0, 12.0);
        let l = BrickLayout::build(
            &settings(extent, 4.0 / 3.0, 4.0, 1_000_000),
            &floor(extent, 12.25),
        );
        assert_eq!(l.levels, 2);
        let fine_below = l
            .bricks
            .iter()
            .any(|b| b.level == 1 && (b.lo.y + b.size.y - 12.0).abs() < 1e-3);
        let fine_above = l
            .bricks
            .iter()
            .any(|b| b.level == 1 && (b.lo.y - 12.0).abs() < 1e-3);
        assert!(
            fine_below && fine_above,
            "below {fine_below}, above {fine_above}"
        );
    }

    /// A probe several bricks share is moved and traced by the finest of
    /// their spacings -- and no spacing is coarser than asked for, whatever
    /// the box's size.
    #[test]
    fn a_probe_has_the_finest_spacing_of_its_bricks_and_none_exceeds_the_max() {
        let extent = Vec3::new(12.0, 24.0, 12.0);
        let l = BrickLayout::build(&settings(extent, 0.4, 4.0, 1_000_000), &floor(extent, 6.0));
        let mut want = vec![Vec3::splat(f32::INFINITY); l.probes.len()];
        for b in &l.bricks {
            for &id in &b.probes {
                want[id as usize] = want[id as usize].min(b.size / 3.0);
            }
        }
        assert!(
            l.bricks.iter().map(|b| b.level).max() > l.bricks.iter().map(|b| b.level).min(),
            "premise: bricks of different levels"
        );
        for (p, w) in l.probes.iter().zip(&want) {
            assert!((p.spacing - *w).abs().max_element() < 1e-5, "{p:?} vs {w}");
        }
        for x in [1.0, 11.9, 12.1, 25.0, 100.0] {
            let l = BrickLayout::build(&settings(Vec3::new(x, 3.0, 7.0), 0.5, 4.0, 1_000_000), &[]);
            let spacing = l.bricks[0].size / 3.0;
            assert!(spacing.max_element() <= 4.0 + 1e-4, "{x}: {spacing}");
        }
    }

    /// No geometry: the box is all top-level bricks, its probes their shared
    /// lattice -- `(3 n + 1)` per axis, each once.
    #[test]
    fn open_space_is_coarse_and_its_probes_shared() {
        let extent = Vec3::new(24.0, 12.0, 12.0);
        let l = BrickLayout::build(&settings(extent, 0.5, 4.0, 100_000), &[]);
        assert_eq!(l.top, [2, 1, 1], "24 / (3 x 4) and 12 / 12");
        assert!(l.levels > 1, "premise: finer levels were available");
        assert_eq!(l.bricks.len(), 2);
        assert!(l.bricks.iter().all(|b| b.level == 0));
        assert_eq!(l.probes.len(), 7 * 4 * 4, "the face between them is shared");
        assert!(!l.truncated);
    }

    /// A floor refines the bricks it passes through down to the finest
    /// level, and only those: a brick above the floor's reach stays coarse,
    /// and every probe sits on the lattice of its spacing.
    #[test]
    fn geometry_refines_the_bricks_it_passes_through() {
        let extent = Vec3::splat(12.0);
        // Two top bricks high; the floor in the lower one, on a lattice
        // plane of the finest level (12 x 4/9).
        let extent = Vec3::new(extent.x, 24.0, extent.z);
        let y = 16.0 / 3.0;
        let l = BrickLayout::build(
            &settings(extent, 4.0 / 9.0, 4.0, 1_000_000),
            &floor(extent, y),
        );
        assert_eq!(l.levels, 3, "4, 4/3, 4/9");
        let finest = l.bricks.iter().filter(|b| b.level == 2).count();
        assert!(finest > 0, "premise: the floor reached the finest level");
        for b in &l.bricks {
            let grow = 0.5 * b.size.y / 3.0;
            let near = b.lo.y - grow <= y && b.lo.y + b.size.y + grow >= y;
            if b.level < 2 {
                assert!(!near, "a brick the floor touches is cut: {b:?}");
            }
        }
        assert!(
            l.bricks.iter().any(|b| b.level == 0 && b.lo.y >= 12.0),
            "the upper brick, far from the floor, stays coarse"
        );
        // Fine near the floor: a probe within a finest spacing of it.
        let fine = 4.0 / 9.0;
        assert!(l
            .probes
            .iter()
            .any(|p| (p.position.y - y).abs() < 1e-3 && (p.spacing.y - fine).abs() < 1e-4));
    }

    /// Every point of the box finds the brick that contains it, and its
    /// place there -- the walk the shader does too, so the two can be held
    /// to each other.
    #[test]
    fn every_point_finds_the_brick_that_contains_it() {
        let extent = Vec3::new(12.0, 24.0, 12.0);
        let l = BrickLayout::build(&settings(extent, 0.4, 4.0, 1_000_000), &floor(extent, 6.0));
        assert!(
            l.bricks.iter().any(|b| b.level == 2),
            "premise: three levels in use"
        );
        let mut state = 7u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 8) as f32 / (1u32 << 24) as f32
        };
        for _ in 0..2000 {
            let p = Vec3::new(next(), next(), next()) * extent;
            let (b, local) = l.locate(p).expect("a brick");
            let brick = &l.bricks[b as usize];
            let want = (p - brick.lo) / brick.size * 3.0;
            assert!(
                (local - want).abs().max_element() < 1e-3,
                "{p}: local {local}, want {want} in {brick:?}"
            );
            assert!(local.min_element() >= -1e-4 && local.max_element() <= 3.0 + 1e-4);
        }
        // The far corner, exactly on the box: clamped into the last brick.
        let (b, local) = l.locate(extent).unwrap();
        let brick = &l.bricks[b as usize];
        assert!(((brick.lo + brick.size) - extent).abs().max_element() < 1e-3);
        assert!((local - Vec3::splat(3.0)).abs().max_element() < 1e-3);
    }

    /// A brick's probes are where its lattice says, and a probe shared by
    /// two bricks is one probe.
    #[test]
    fn a_bricks_probes_are_its_lattice_points_once() {
        let extent = Vec3::new(12.0, 24.0, 12.0);
        let l = BrickLayout::build(&settings(extent, 0.4, 4.0, 1_000_000), &floor(extent, 6.0));
        let mut uses = vec![0u32; l.probes.len()];
        for b in &l.bricks {
            for (i, &id) in b.probes.iter().enumerate() {
                let local = Vec3::new((i % 4) as f32, (i / 4 % 4) as f32, (i / 16) as f32);
                let want = b.lo + b.size * local / 3.0;
                let got = l.probes[id as usize].position;
                assert!((got - want).abs().max_element() < 1e-3, "{got} vs {want}");
                uses[id as usize] += 1;
            }
        }
        assert!(uses.iter().all(|&u| u > 0), "no probe belongs to no brick");
        assert!(uses.iter().any(|&u| u > 1), "premise: bricks share probes");
        let mut positions: Vec<[i64; 3]> = l
            .probes
            .iter()
            .map(|p| p.position.to_array().map(|c| (c * 1000.0).round() as i64))
            .collect();
        positions.sort();
        positions.dedup();
        assert_eq!(positions.len(), l.probes.len(), "each position once");
    }

    /// Over budget, a level is left uncut as a whole: the layout stays
    /// within the budget, says so, and is still a full cover of the box.
    #[test]
    fn a_level_over_budget_is_not_cut() {
        let extent = Vec3::new(12.0, 24.0, 12.0);
        let tris = floor(extent, 6.0);
        let full = BrickLayout::build(&settings(extent, 0.4, 4.0, 1_000_000), &tris);
        let budget = full.probes.len() - 1;
        let l = BrickLayout::build(&settings(extent, 0.4, 4.0, budget), &tris);
        assert!(l.truncated);
        assert!(l.probes.len() <= budget);
        assert!(
            l.bricks.iter().all(|b| b.level < 2),
            "the finest level was not cut at all"
        );
        assert!(
            l.bricks.iter().any(|b| b.level == 1),
            "the level that fit was"
        );
        let volume: f32 = l
            .bricks
            .iter()
            .map(|b| b.size.x * b.size.y * b.size.z)
            .sum();
        assert!(
            (volume - extent.x * extent.y * extent.z).abs() < 1e-2,
            "{volume}"
        );
    }

    /// A box too large for the budget even at its coarsest is cut coarser,
    /// says so, and fits; and the brick budget holds like the probe budget.
    #[test]
    fn a_box_too_large_for_the_budget_is_coarsened() {
        let extent = Vec3::new(240.0, 12.0, 240.0);
        let fit = BrickLayout::build(&settings(extent, 0.5, 4.0, 1_000_000), &[]);
        assert!(!fit.coarsened);
        assert_eq!(fit.bricks.len(), 400, "premise: 20 x 1 x 20 at 4 m");
        let l = BrickLayout::build(&settings(extent, 0.5, 4.0, 1_000), &[]);
        assert!(l.coarsened);
        assert!(l.probes.len() <= 1_000, "{}", l.probes.len());
        let s = BrickSettings {
            max_bricks: 100,
            ..settings(extent, 0.5, 4.0, 1_000_000)
        };
        let l = BrickLayout::build(&s, &[]);
        assert!(l.coarsened && l.bricks.len() <= 100, "{}", l.bricks.len());
        // And a level that would pass the brick budget is not cut.
        let tris = floor(Vec3::new(12.0, 24.0, 12.0), 6.0);
        let s = BrickSettings {
            max_bricks: 20,
            ..settings(Vec3::new(12.0, 24.0, 12.0), 0.4, 4.0, 1_000_000)
        };
        let l = BrickLayout::build(&s, &tris);
        assert!(l.truncated && l.bricks.len() <= 20, "{}", l.bricks.len());
    }

    /// Every layout has at least eight probes per brick -- each brick brings
    /// its own interior and lower faces, which no other brick of any level
    /// shares -- so a probe buffer sized for a layout's probes (`n / 8`
    /// bricks to `n` probes) always has room for its bricks. Held over
    /// layouts of every shape: open, a floor, scattered slivers.
    #[test]
    fn a_layout_has_at_least_eight_probes_per_brick() {
        let mut state = 3u32;
        let mut next = || {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            (state >> 8) as f32 / (1u32 << 24) as f32
        };
        let extent = Vec3::new(12.0, 24.0, 12.0);
        let mut cases: Vec<Vec<[Vec3; 3]>> = vec![Vec::new(), floor(extent, 6.0)];
        for n in [1, 5, 40] {
            let mut tris = Vec::new();
            for _ in 0..n {
                let p = Vec3::new(next(), next(), next()) * extent;
                let d = Vec3::new(next(), next(), next()) * 0.3;
                tris.push([p, p + d, p + Vec3::new(d.y, d.z, d.x)]);
            }
            cases.push(tris);
        }
        for (i, tris) in cases.iter().enumerate() {
            for min in [0.4, 1.4] {
                let l = BrickLayout::build(&settings(extent, min, 4.0, 1_000_000), tris);
                assert!(
                    l.probes.len() >= 8 * l.bricks.len(),
                    "case {i}, min {min}: {} probes, {} bricks",
                    l.probes.len(),
                    l.bricks.len()
                );
            }
        }
    }

    /// `min_spacing` no finer than `max_spacing`'s level is one level; one
    /// finer than four levels is clamped to four.
    #[test]
    fn levels_follow_the_spacings() {
        let e = Vec3::splat(12.0);
        assert_eq!(
            BrickLayout::build(&settings(e, 4.0, 4.0, 10), &[]).levels,
            1
        );
        assert_eq!(
            BrickLayout::build(&settings(e, 1.0, 4.0, 10), &[]).levels,
            2
        );
        assert_eq!(
            BrickLayout::build(&settings(e, 0.01, 4.0, 10), &[]).levels,
            MAX_LEVELS
        );
    }
}
