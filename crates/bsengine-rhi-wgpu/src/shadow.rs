//! Fitting the directional shadow map's frustum to the camera.
//!
//! A directional light has no position, so something has to decide *which*
//! part of the world its shadow map covers. This module makes that decision.
//!
//! It lives in this crate rather than in `bsengine-render`, next to the
//! texture it is sizing itself against, for a reason worth stating: the pixel
//! tests in `tests/pixels_lighting.rs` used to carry their own copy of this
//! calculation, because they cannot depend on `bsengine-render`. Their own
//! comment admitted the cost — "the shadow tests prove that the shadow
//! pipeline darkens what *this* matrix says is occluded. They do not prove the
//! runtime picks a good matrix." With the calculation here, the harness and
//! the engine call the same function, and the pixel tests observe the matrix
//! the game actually renders with.

use crate::surface::SHADOW_MAP_SIZE;
use glam::{Mat4, Vec3, Vec4};

/// How far from the camera directional shadows are drawn, in world units.
///
/// Everything beyond this is lit as though nothing occludes it. The number is
/// a quality-for-range trade: the shadow map is a fixed `SHADOW_MAP_SIZE`
/// texels square, so doubling the distance roughly halves the texels per world
/// unit and shadow edges get blockier. Covering a large world *without* paying
/// that is what cascades do, by giving the near slice a map of its own; a
/// single fit cannot.
///
/// 50 is chosen against the old behaviour rather than in the abstract. The
/// fixed box this replaced reached 30 units from the world origin, so this is
/// a longer reach than the engine had, not a shorter one.
pub const SHADOW_DISTANCE: f32 = 50.0;

/// Slack in front of and behind the shadowed sphere, in world units.
///
/// The light's near plane sits this far in front of the fitted sphere, so a
/// caster standing between the sphere and the light is still inside the depth
/// range instead of being clipped away and silently casting nothing.
const SHADOW_DEPTH_MARGIN: f32 = 50.0;

/// The eight world-space corners of a view-projection's frustum.
///
/// Near-plane corners are indices 0..4 and far-plane corners 4..8, each in the
/// order (-x,-y), (+x,-y), (-x,+y), (+x,+y) — so `corners[i]` and
/// `corners[i + 4]` are the two ends of one frustum edge, which is what
/// [`slice_frustum`] relies on.
pub fn frustum_corners_world(view_proj: Mat4) -> [Vec3; 8] {
    let inv = view_proj.inverse();
    let mut corners = [Vec3::ZERO; 8];
    let mut i = 0;
    // z spans 0..1, not -1..1: wgpu's clip space, matching the rh_zo
    // projections the rest of the renderer builds.
    for z in [0.0_f32, 1.0] {
        for y in [-1.0_f32, 1.0] {
            for x in [-1.0_f32, 1.0] {
                let p = inv * Vec4::new(x, y, z, 1.0);
                corners[i] = p.truncate() / p.w;
                i += 1;
            }
        }
    }
    corners
}

/// Reslices a frustum to the depth range `near..far`, measured in world units
/// along the view axis from the original near plane.
///
/// Separate from [`directional_light_view_proj`] because this is precisely
/// what cascaded shadow maps repeat: one call per split range, each feeding
/// its own fit. Today there is one call, for `0..`[`SHADOW_DISTANCE`].
pub fn slice_frustum(corners: [Vec3; 8], near: f32, far: f32) -> [Vec3; 8] {
    let near_centre = (corners[0] + corners[1] + corners[2] + corners[3]) / 4.0;
    let far_centre = (corners[4] + corners[5] + corners[6] + corners[7]) / 4.0;
    let depth = (far_centre - near_centre).length();
    if !depth.is_finite() || depth <= 1e-6 {
        return corners;
    }
    // The near and far planes are parallel and `depth` apart, so all four
    // edges advance along the view axis at the same rate: one shared
    // parameter reslices every one of them.
    let t_near = (near / depth).clamp(0.0, 1.0);
    let t_far = (far / depth).clamp(0.0, 1.0);
    let mut out = [Vec3::ZERO; 8];
    for i in 0..4 {
        out[i] = corners[i].lerp(corners[i + 4], t_near);
        out[i + 4] = corners[i].lerp(corners[i + 4], t_far);
    }
    out
}

/// The directional shadow pass's orthographic view-projection, fitted around
/// the part of the camera's view that is close enough to shadow.
///
/// Uses rh_zo (0..1 depth) to match wgpu's depth buffer convention.
///
/// # Why this takes the camera
///
/// This used to look at `Vec3::ZERO` unconditionally, pinning the shadowed
/// region to a fixed 60x60 box on the world origin however far away the player
/// walked. `games/scale-level`'s authored entities span x = 0..152, so most of
/// that shipped level could never receive a directional shadow — not at any
/// resolution, because the frustum did not move.
///
/// # Why a sphere
///
/// The region is fitted as the bounding *sphere* of the sliced frustum, not as
/// a tight box around it. A box refitted each frame changes size as the camera
/// turns, and a shadow map whose world-per-texel ratio changes every frame
/// makes every shadow edge crawl. Rotating the camera rotates all eight
/// corners rigidly about it, so a sphere's radius is untouched by rotation and
/// the ratio holds still. Unity calls this "stable fit", and it is why
/// Unreal's whole-scene dynamic shadow also fits a sphere.
///
/// The cost is that a sphere is a loose fit — wider than a tight box would
/// need — which is resolution traded for the absence of crawling.
pub fn directional_light_view_proj(light_dir: Vec3, camera_view_proj: Mat4) -> Mat4 {
    let dir = light_dir.normalize_or(Vec3::NEG_Y);
    let corners = slice_frustum(
        frustum_corners_world(camera_view_proj),
        0.0,
        SHADOW_DISTANCE,
    );
    let centre = corners.iter().copied().sum::<Vec3>() / 8.0;
    let radius = corners
        .iter()
        .map(|c| c.distance(centre))
        .fold(0.0_f32, f32::max);
    // A degenerate camera — identity matrices before one exists, a zero-extent
    // viewport — still has to yield an invertible matrix rather than an
    // orthographic projection of width zero.
    let (centre, radius) = if centre.is_finite() && radius.is_finite() && radius > 1e-3 {
        (centre, radius)
    } else {
        (Vec3::ZERO, 1.0)
    };
    let up = if dir.y.abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    // Snapped to whole texels *in light space*, the space the shadow map is
    // rasterized in — snapping in world space would align it to nothing.
    // Without this the sphere slides a fraction of a texel each frame and the
    // edges crawl even though the radius is stable.
    //
    // The basis is built from the light direction alone, deliberately.
    // Measured against a view matrix that is itself aimed at the centre, the
    // centre sits at that view's origin — where rounding it does nothing at
    // all. This was written that way first, and the snapping stayed dead code
    // until a mutation test deleted it and not one test failed.
    let basis = Mat4::look_at_rh(Vec3::ZERO, dir, up);
    let texel = 2.0 * radius / SHADOW_MAP_SIZE as f32;
    let centre_ls = basis.transform_point3(centre);
    let snapped_ls = Vec3::new(
        (centre_ls.x / texel).round() * texel,
        (centre_ls.y / texel).round() * texel,
        centre_ls.z,
    );
    let centre = basis.inverse().transform_point3(snapped_ls);
    let eye = centre - dir * (radius + SHADOW_DEPTH_MARGIN);
    let view = Mat4::look_at_rh(eye, centre, up);
    let proj = Mat4::orthographic_rh(
        -radius,
        radius,
        -radius,
        radius,
        0.1,
        2.0 * radius + 2.0 * SHADOW_DEPTH_MARGIN,
    );
    proj * view
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A camera at `eye` looking at `target`, with the projection the engine
    /// builds for a 16:9 viewport.
    fn camera(eye: Vec3, target: Vec3) -> Mat4 {
        Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, 100.0)
            * Mat4::look_at_rh(eye, target, Vec3::Y)
    }

    /// Whether a world-space point lands inside the shadow map's clip volume.
    fn shadowed(vp: Mat4, p: Vec3) -> bool {
        let c = vp * p.extend(1.0);
        if c.w.abs() <= f32::EPSILON {
            return false;
        }
        let n = c.truncate() / c.w;
        (-1.0..=1.0).contains(&n.x) && (-1.0..=1.0).contains(&n.y) && (0.0..=1.0).contains(&n.z)
    }

    const SUN: Vec3 = Vec3::new(-0.3, -1.0, -0.2);

    /// Whether a matrix is genuinely invertible, asserted as a round trip.
    ///
    /// These tests read `determinant().abs() > 1e-6` before. An orthographic
    /// matrix's determinant shrinks with the volume it covers, so that
    /// threshold was quietly also an assertion about the fit's *size*: a
    /// mutation that widened the fit to the camera's whole 100-unit frustum
    /// failed it, while the matrix inverted perfectly well. Cascades will fit
    /// wider boxes on purpose.
    fn inverts_cleanly(vp: Mat4) -> bool {
        let inv = vp.inverse();
        inv.to_cols_array().iter().all(|f| f.is_finite())
            && (vp * inv).abs_diff_eq(Mat4::IDENTITY, 1e-3)
    }

    #[test]
    fn light_view_proj_is_invertible() {
        let vp = directional_light_view_proj(
            Vec3::new(-0.4, -0.8, -0.4).normalize(),
            camera(Vec3::new(0.0, 4.0, 7.0), Vec3::ZERO),
        );
        assert!(inverts_cleanly(vp), "light VP should be invertible: {vp:?}");
    }

    #[test]
    fn light_view_proj_up_axis_does_not_degenerate() {
        // Straight down — the up vector has to switch to Z without NaN.
        let vp = directional_light_view_proj(
            Vec3::new(0.0, -1.0, 0.0),
            camera(Vec3::new(0.0, 4.0, 7.0), Vec3::ZERO),
        );
        assert!(inverts_cleanly(vp), "{vp:?}");
    }

    #[test]
    fn a_degenerate_camera_still_yields_an_invertible_matrix() {
        let vp = directional_light_view_proj(SUN, Mat4::ZERO);
        assert!(
            inverts_cleanly(vp),
            "a zero camera matrix must not produce a zero-width projection: {vp:?}"
        );
        assert!(vp.to_cols_array().iter().all(|f| f.is_finite()), "{vp:?}");
    }

    /// The defect this module was written to fix. The old fit centred a fixed
    /// box on the world origin, so a camera looking at the far end of a long
    /// level shadowed nothing it could see.
    #[test]
    fn the_shadowed_region_follows_the_camera() {
        // Roughly where `games/scale-level` ends: its entities span x = 0..152.
        let far = Vec3::new(140.0, 0.0, 0.0);
        let vp = directional_light_view_proj(SUN, camera(far + Vec3::new(0.0, 4.0, 7.0), far));
        assert!(
            shadowed(vp, far),
            "what the camera is looking at must be inside the shadow frustum; \
             at x = {} it was not",
            far.x
        );
        assert!(
            !shadowed(vp, Vec3::ZERO),
            "and the world origin, 140 units behind the camera, must not be — \
             otherwise the region did not move, it merely grew"
        );
    }

    /// The converse, asserted in the same shape so neither can pass alone: a
    /// camera at the origin must shadow the origin and *not* the far end.
    #[test]
    fn the_shadowed_region_leaves_what_the_camera_cannot_see() {
        let vp = directional_light_view_proj(SUN, camera(Vec3::new(0.0, 4.0, 7.0), Vec3::ZERO));
        assert!(shadowed(vp, Vec3::ZERO), "the origin is being looked at");
        assert!(
            !shadowed(vp, Vec3::new(140.0, 0.0, 0.0)),
            "x = 140 is far outside {SHADOW_DISTANCE} units and must fall outside the fit"
        );
    }

    /// The reason the fit is a sphere. Turning the camera on the spot must not
    /// change how much world one shadow texel covers, or every shadow edge
    /// crawls while the player looks around.
    #[test]
    fn turning_the_camera_does_not_resize_the_shadowed_region() {
        let eye = Vec3::new(10.0, 4.0, 10.0);
        // The projected width of a fixed world-space segment is the texel
        // ratio, read off the matrix without needing the texture.
        let width = |target: Vec3| {
            let vp = directional_light_view_proj(SUN, camera(eye, target));
            let a = vp * Vec3::new(0.0, 0.0, 0.0).extend(1.0);
            let b = vp * Vec3::new(1.0, 0.0, 0.0).extend(1.0);
            (b.x / b.w - a.x / a.w).abs()
        };
        let north = width(eye + Vec3::new(0.0, 0.0, -1.0));
        let east = width(eye + Vec3::new(1.0, 0.0, 0.0));
        let diagonal = width(eye + Vec3::new(0.7, -0.2, 0.7));
        assert!(
            (north - east).abs() < north * 0.01 && (north - diagonal).abs() < north * 0.01,
            "one world unit must cover the same share of the shadow map whichever \
             way the camera faces: {north} north, {east} east, {diagonal} diagonal"
        );
    }

    /// Texel snapping, which is what stops shadow edges crawling as the player
    /// walks: the shadow map's grid must advance in whole texels rather than
    /// sliding continuously.
    ///
    /// Asserted as a staircase rather than as "a small step moves it a little".
    /// The first version of this test nudged the camera a millimetre and
    /// checked the grid moved less than a tenth of a texel — which an
    /// unsnapped fit also satisfies, because a millimetre *is* a fortieth of a
    /// texel here. It passed against an implementation whose snapping was
    /// dead code.
    #[test]
    fn the_shadow_grid_advances_in_whole_texels() {
        let eye = Vec3::new(10.0, 4.0, 10.0);
        let look = Vec3::new(10.0, 0.0, 0.0);
        let probe = Vec3::new(10.0, 0.0, 0.0);
        let texel_clip = 2.0 / SHADOW_MAP_SIZE as f32;
        // Where a fixed world point lands in the shadow map, as a fraction of
        // one texel. Snapping quantises the map's origin, so this fraction is
        // the same at every camera position; without it the point drifts
        // smoothly across the texel and this sweeps 0..1.
        let residual = |d: f32| {
            let shift = Vec3::new(d, 0.0, d * 0.6);
            let vp = directional_light_view_proj(SUN, camera(eye + shift, look + shift));
            let c = vp * probe.extend(1.0);
            (c.x / c.w / texel_clip).rem_euclid(1.0)
        };
        // Steps of roughly a third of a texel, over several texels' worth of
        // travel, so an unsnapped fit cannot stay at one value by luck.
        let first = residual(0.0);
        for i in 1..12 {
            let r = residual(i as f32 * 0.015);
            let drift = (r - first).abs().min(1.0 - (r - first).abs());
            assert!(
                drift < 0.02,
                "step {i} put the probe {r} of a texel into the grid where step 0 \
                 put it {first}; a snapped grid holds this fixed and only a \
                 continuously sliding one drifts"
            );
        }
    }

    #[test]
    fn slicing_shortens_the_frustum_from_the_far_end() {
        let vp = camera(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO);
        let full = frustum_corners_world(vp);
        let sliced = slice_frustum(full, 0.0, 20.0);
        // The near plane is untouched and the far plane has come closer.
        for i in 0..4 {
            assert!(
                (sliced[i] - full[i]).length() < 1e-3,
                "slicing from 0 must leave the near corners alone"
            );
            assert!(
                sliced[i + 4].distance(sliced[i]) < full[i + 4].distance(full[i]),
                "corner {i}'s edge should be shorter after slicing to 20 of 100 units"
            );
        }
    }

    #[test]
    fn slicing_beyond_the_far_plane_is_clamped_not_extrapolated() {
        let vp = camera(Vec3::new(0.0, 0.0, 10.0), Vec3::ZERO);
        let full = frustum_corners_world(vp);
        // The camera's far plane is 100 units out; asking for 1000 must not
        // invent a frustum ten times too big.
        let sliced = slice_frustum(full, 0.0, 1000.0);
        for i in 0..8 {
            assert!(
                (sliced[i] - full[i]).length() < 1e-3,
                "corner {i} should be clamped to the camera's own far plane"
            );
        }
    }
}
