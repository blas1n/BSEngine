//! CPU software occlusion culling: rasterize conservative occluder boxes
//! into a small depth buffer, then test candidate bounding boxes against
//! it. Deliberately GPU-free -- everything here is a pure function over
//! plain math types, so it is unit-testable without an adapter, and it
//! resolves within the same frame instead of a frame late the way a GPU
//! occlusion query would.
//!
//! # The correctness rule
//!
//! There is exactly one failure mode that matters: culling something that
//! was actually visible. A missed culling opportunity costs a little
//! performance; a false cull is a visible rendering bug. Everything in
//! this module is therefore biased toward under-culling, via two
//! mechanisms that compose:
//!
//! 1. The occluder box the caller supplies must fit *inside* the real
//!    geometry (enforced by whoever builds an `Occluder`, not here).
//! 2. Rasterization is inner-conservative: a pixel is written only when
//!    all four of its corners fall inside the projected triangle. Pixels
//!    straddling a triangle edge, and thin seams between two triangles of
//!    the same box, are left unwritten. Both leave *holes*, and a hole
//!    reads as "not known to be covered", which can only under-cull.

use glam::{Mat4, Vec3, Vec4Swizzles};

/// Edge length of the square software depth buffer. Small on purpose: the
/// cost is per-pixel per-occluder-triangle every frame, and occlusion only
/// needs to answer a coarse "is this whole object behind that wall".
pub const OCCLUSION_BUFFER_SIZE: usize = 128;

/// A square software depth buffer holding, per pixel, the nearest occluder
/// depth known to cover that pixel. `f32::INFINITY` means "nothing covers
/// this pixel" -- the value that makes an empty buffer occlude nothing.
#[derive(Clone)]
pub struct OcclusionBuffer {
    depths: Vec<f32>,
}

impl Default for OcclusionBuffer {
    fn default() -> Self {
        Self {
            depths: vec![f32::INFINITY; OCCLUSION_BUFFER_SIZE * OCCLUSION_BUFFER_SIZE],
        }
    }
}

impl OcclusionBuffer {
    /// Resets every pixel to "uncovered". Called once per frame before
    /// rasterizing that frame's occluders; the allocation is reused.
    pub fn clear(&mut self) {
        self.depths.fill(f32::INFINITY);
    }

    /// True when no pixel has been written since the last `clear` -- i.e.
    /// this frame had no usable occluders, so nothing can be culled.
    pub fn is_empty(&self) -> bool {
        self.depths.iter().all(|d| d.is_infinite())
    }

    /// The nearest occluder depth known to cover this pixel, or
    /// `f32::INFINITY` when nothing covers it.
    fn depth_at(&self, x: usize, y: usize) -> f32 {
        self.depths[y * OCCLUSION_BUFFER_SIZE + x]
    }

    fn write_nearer(&mut self, x: usize, y: usize, depth: f32) {
        let slot = &mut self.depths[y * OCCLUSION_BUFFER_SIZE + x];
        if depth < *slot {
            *slot = depth;
        }
    }
}

/// One projected vertex: buffer-space x/y in pixels, plus NDC depth.
#[derive(Clone, Copy)]
struct Projected {
    x: f32,
    y: f32,
    depth: f32,
}

/// Projects a world-space point into buffer space. Returns `None` when the
/// point is at or behind the eye (`w <= 0`), which is the case a
/// perspective divide cannot represent -- callers must treat any box with
/// such a corner as unusable rather than guessing.
fn project(view_proj: Mat4, world: Vec3) -> Option<Projected> {
    let clip = view_proj * world.extend(1.0);
    if clip.w <= 1e-6 {
        return None;
    }
    let ndc = clip.xyz() / clip.w;
    let size = OCCLUSION_BUFFER_SIZE as f32;
    Some(Projected {
        x: (ndc.x * 0.5 + 0.5) * size,
        y: (0.5 - ndc.y * 0.5) * size,
        depth: ndc.z,
    })
}

/// The 8 corners of a local-space box, in world space.
fn box_corners(model: Mat4, center: Vec3, half_extents: Vec3) -> [Vec3; 8] {
    let mut out = [Vec3::ZERO; 8];
    let mut i = 0;
    for sx in [-1.0f32, 1.0] {
        for sy in [-1.0f32, 1.0] {
            for sz in [-1.0f32, 1.0] {
                let local = center + half_extents * Vec3::new(sx, sy, sz);
                out[i] = (model * local.extend(1.0)).truncate();
                i += 1;
            }
        }
    }
    out
}

/// Signed area of the triangle (a, b, c) in 2D; sign tells winding.
fn edge(ax: f32, ay: f32, bx: f32, by: f32, px: f32, py: f32) -> f32 {
    (bx - ax) * (py - ay) - (by - ay) * (px - ax)
}

/// Rasterizes one triangle inner-conservatively: a pixel is written only
/// when all four of its corners are inside the triangle. Depth is taken as
/// the triangle's maximum vertex depth, which is the farthest the triangle
/// could be at any covered pixel -- using the farthest rather than an
/// interpolated value keeps the stored depth from ever claiming the
/// occluder is nearer than it really is at that pixel.
fn rasterize_triangle(buf: &mut OcclusionBuffer, a: Projected, b: Projected, c: Projected) {
    let size = OCCLUSION_BUFFER_SIZE as f32;
    let min_x = a.x.min(b.x).min(c.x).floor().max(0.0) as usize;
    let max_x = (a.x.max(b.x).max(c.x).ceil().min(size - 1.0)).max(0.0) as usize;
    let min_y = a.y.min(b.y).min(c.y).floor().max(0.0) as usize;
    let max_y = (a.y.max(b.y).max(c.y).ceil().min(size - 1.0)).max(0.0) as usize;
    if min_x > max_x || min_y > max_y {
        return;
    }

    // Normalize winding so "inside" is consistently non-negative.
    let area = edge(a.x, a.y, b.x, b.y, c.x, c.y);
    if area.abs() < 1e-6 {
        return; // degenerate
    }
    let flip = if area < 0.0 { -1.0 } else { 1.0 };

    let depth = a.depth.max(b.depth).max(c.depth);

    for py in min_y..=max_y {
        for px in min_x..=max_x {
            let inside_all_corners = [
                (px as f32, py as f32),
                (px as f32 + 1.0, py as f32),
                (px as f32, py as f32 + 1.0),
                (px as f32 + 1.0, py as f32 + 1.0),
            ]
            .iter()
            .all(|&(cx, cy)| {
                edge(a.x, a.y, b.x, b.y, cx, cy) * flip >= 0.0
                    && edge(b.x, b.y, c.x, c.y, cx, cy) * flip >= 0.0
                    && edge(c.x, c.y, a.x, a.y, cx, cy) * flip >= 0.0
            });
            if inside_all_corners {
                buf.write_nearer(px, py, depth);
            }
        }
    }
}

/// Rasterizes one occluder box's 12 triangles into `buf`.
///
/// `center`/`half_extents` are in the entity's local space; `model` places
/// it in the world. If any of the box's 8 corners fails to project (it is
/// at or behind the eye), the whole box is skipped: a partially-projectable
/// box cannot be rasterized correctly, and skipping it only under-culls.
pub fn rasterize_occluder_box(
    buf: &mut OcclusionBuffer,
    view_proj: Mat4,
    model: Mat4,
    center: Vec3,
    half_extents: Vec3,
) {
    let corners = box_corners(model, center, half_extents);
    let mut projected = [Projected {
        x: 0.0,
        y: 0.0,
        depth: 0.0,
    }; 8];
    for (i, c) in corners.iter().enumerate() {
        match project(view_proj, *c) {
            Some(p) => projected[i] = p,
            None => return,
        }
    }

    // Corner index bit layout from `box_corners`: bit2 = x sign, bit1 = y
    // sign, bit0 = z sign (0 = -1, 1 = +1). These 12 triangles are the 6
    // faces, two each.
    const FACES: [[usize; 4]; 6] = [
        [0, 1, 3, 2], // x = -1
        [4, 6, 7, 5], // x = +1
        [0, 4, 5, 1], // y = -1
        [2, 3, 7, 6], // y = +1
        [0, 2, 6, 4], // z = -1
        [1, 5, 7, 3], // z = +1
    ];
    for f in FACES {
        rasterize_triangle(buf, projected[f[0]], projected[f[1]], projected[f[2]]);
        rasterize_triangle(buf, projected[f[0]], projected[f[2]], projected[f[3]]);
    }
}

/// True only when the candidate is definitely hidden.
///
/// The candidate is described by a world-space center and half-extents --
/// an *over*-estimate of the object (its bounding box), which is the safe
/// direction: testing something larger than the object makes it harder to
/// declare occluded, never easier.
///
/// Returns `false` (not occluded, the safe answer) when: the buffer is
/// empty, any of the candidate's corners is unprojectable, its screen rect
/// falls outside the buffer, any pixel of that rect is uncovered, or any
/// covered pixel's stored occluder depth is not strictly nearer than the
/// candidate's own nearest depth.
pub fn box_occluded(
    buf: &OcclusionBuffer,
    view_proj: Mat4,
    world_center: Vec3,
    world_half_extents: Vec3,
) -> bool {
    if buf.is_empty() {
        return false;
    }

    let corners = box_corners(Mat4::IDENTITY, world_center, world_half_extents);
    let mut min_x = f32::MAX;
    let mut max_x = f32::MIN;
    let mut min_y = f32::MAX;
    let mut max_y = f32::MIN;
    // The candidate's NEAREST depth: if even its closest point is behind
    // the occluder, all of it is.
    let mut nearest_depth = f32::MAX;
    for c in corners {
        let Some(p) = project(view_proj, c) else {
            return false;
        };
        min_x = min_x.min(p.x);
        max_x = max_x.max(p.x);
        min_y = min_y.min(p.y);
        max_y = max_y.max(p.y);
        nearest_depth = nearest_depth.min(p.depth);
    }

    let size = OCCLUSION_BUFFER_SIZE as f32;
    // Any part of the candidate outside the buffer is unknown territory,
    // and unknown must never mean occluded.
    if min_x < 0.0 || min_y < 0.0 || max_x >= size || max_y >= size {
        return false;
    }

    let x0 = min_x.floor() as usize;
    let x1 = (max_x.ceil() as usize).min(OCCLUSION_BUFFER_SIZE - 1);
    let y0 = min_y.floor() as usize;
    let y1 = (max_y.ceil() as usize).min(OCCLUSION_BUFFER_SIZE - 1);

    for py in y0..=y1 {
        for px in x0..=x1 {
            if buf.depth_at(px, py) >= nearest_depth {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera at +Z looking down -Z, the convention the engine's own
    /// `Transform::view_matrix` produces for an unrotated camera.
    fn test_view_proj() -> Mat4 {
        let proj = Mat4::perspective_rh(60.0f32.to_radians(), 1.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO, Vec3::Y);
        proj * view
    }

    #[test]
    fn a_fresh_buffer_is_empty_and_covers_nothing() {
        let buf = OcclusionBuffer::default();
        assert!(buf.is_empty());
        assert!(buf.depth_at(64, 64).is_infinite());
    }

    #[test]
    fn rasterizing_a_big_box_covers_the_center_of_the_buffer() {
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            test_view_proj(),
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 0.5),
        );
        assert!(!buf.is_empty(), "a box filling the view must write pixels");
        assert!(
            buf.depth_at(64, 64).is_finite(),
            "the center pixel must be covered by a box centered in view"
        );
    }

    #[test]
    fn a_box_entirely_behind_the_camera_writes_nothing() {
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            test_view_proj(),
            Mat4::from_translation(Vec3::new(0.0, 0.0, 500.0)),
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 0.5),
        );
        assert!(
            buf.is_empty(),
            "a box behind the eye has unprojectable corners and must be skipped entirely"
        );
    }

    #[test]
    fn clear_resets_every_pixel() {
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            test_view_proj(),
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 0.5),
        );
        assert!(!buf.is_empty());
        buf.clear();
        assert!(buf.is_empty(), "clear must restore the empty state");
    }

    /// The candidate sits at x = 2 rather than dead centre, and that offset
    /// is load-bearing. Each box face is drawn as two triangles, and
    /// inner-conservative rasterization leaves a one-pixel unwritten seam
    /// along their shared diagonal -- measured as exactly the 104 interior
    /// pixels where `px + py == 127`, i.e. straight through the buffer
    /// centre. A candidate centred on the view axis has its screen rect
    /// straddling that seam, so it reads as partly uncovered and stays
    /// visible. That is the module's bias behaving as designed (a hole
    /// under-culls, which is safe), and it does not depend on the
    /// occluder's size: widening the wall to half-extent 20 or 100 leaves
    /// the seam in the same place. Offsetting the candidate clear of the
    /// seam is what makes this test measure the depth comparison it is
    /// actually about.
    #[test]
    fn a_small_box_directly_behind_a_big_occluder_is_occluded() {
        let vp = test_view_proj();
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            vp,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 0.5),
        );
        assert!(
            box_occluded(&buf, vp, Vec3::new(2.0, 0.0, -20.0), Vec3::splat(0.5)),
            "a small box far behind a large wall must be reported occluded"
        );
    }

    /// The regression test this whole module exists to keep passing: an
    /// object *beside* the wall, not behind it, must never be culled. This
    /// is the failure mode that produces a visible rendering bug rather
    /// than a missed optimization.
    #[test]
    fn a_box_beside_the_occluder_is_never_occluded() {
        let vp = test_view_proj();
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            vp,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(1.0, 1.0, 0.5),
        );
        assert!(
            !box_occluded(&buf, vp, Vec3::new(6.0, 0.0, -20.0), Vec3::splat(0.5)),
            "an object beside the occluder must stay visible"
        );
    }

    #[test]
    fn a_box_in_front_of_the_occluder_is_not_occluded() {
        let vp = test_view_proj();
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            vp,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 0.5),
        );
        assert!(
            !box_occluded(&buf, vp, Vec3::new(0.0, 0.0, 5.0), Vec3::splat(0.5)),
            "an object between the camera and the wall must stay visible"
        );
    }

    #[test]
    fn an_empty_buffer_occludes_nothing() {
        let vp = test_view_proj();
        let buf = OcclusionBuffer::default();
        assert!(
            !box_occluded(&buf, vp, Vec3::new(0.0, 0.0, -20.0), Vec3::splat(0.5)),
            "with no occluders rasterized, nothing may be culled"
        );
    }

    #[test]
    fn a_box_only_partly_behind_the_occluder_is_not_occluded() {
        let vp = test_view_proj();
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            vp,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(2.0, 2.0, 0.5),
        );
        // Wide enough that its screen rect spills past the occluder's edge.
        assert!(
            !box_occluded(
                &buf,
                vp,
                Vec3::new(0.0, 0.0, -20.0),
                Vec3::new(8.0, 8.0, 0.5)
            ),
            "a candidate whose screen rect extends past the occluder must stay visible"
        );
    }

    #[test]
    fn a_candidate_crossing_the_near_plane_is_not_occluded() {
        let vp = test_view_proj();
        let mut buf = OcclusionBuffer::default();
        rasterize_occluder_box(
            &mut buf,
            vp,
            Mat4::IDENTITY,
            Vec3::ZERO,
            Vec3::new(5.0, 5.0, 0.5),
        );
        assert!(
            !box_occluded(&buf, vp, Vec3::new(0.0, 0.0, 10.0), Vec3::splat(50.0)),
            "an unprojectable candidate must get the safe answer, not a guess"
        );
    }
}
