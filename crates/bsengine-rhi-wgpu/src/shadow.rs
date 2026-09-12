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

// Re-exported rather than redefined. `bsengine-core` owns these because it
// also owns `ShadowSettings`, whose defaults are exactly these values; a
// second definition here would let the default a project falls back to drift
// from the array bound the uniform is built against, and the symptom would be
// a cascade the shader can select but the uniform has no room for.
pub use bsengine_core::shadow_config::{
    DEFAULT_SHADOW_DISTANCE as SHADOW_DISTANCE, DEFAULT_SPLIT_EXPONENT as SPLIT_EXPONENT,
    MAX_CASCADES,
};

/// Cascades must not cost more shadow-map memory than the single map they
/// replaced.
///
/// A compile-time assertion rather than a test, following
/// `LightUniformData must stay LIGHT_UNIFORM_SIZE bytes` in `surface.rs`:
/// everything here is known to the compiler, so a build failure is strictly
/// better than a test failure.
///
/// Pinned because nothing else would notice. Four 2048-square layers is 67 MiB
/// against the 16 MiB one map used, and quadrupling a default VRAM budget
/// produces no test failure and no warning — it just costs every player four
/// times the memory for shadows. Unity and Godot both divide a fixed total
/// among cascades; this asserts we do too.
const _: () = assert!(
    (SHADOW_MAP_SIZE as u64) * (SHADOW_MAP_SIZE as u64) * 4 * (MAX_CASCADES as u64)
        <= 2048 * 2048 * 4,
    "all cascades together must not exceed the memory the single 2048-square \
     Depth32Float shadow map used; raising it is a VRAM decision"
);

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

/// The far distance of each cascade, in world units from the camera's near
/// plane, ending at `shadow_distance`.
///
/// Returns `count` boundaries; cascade `i` covers `splits[i - 1]..splits[i]`,
/// with cascade 0 starting at the camera. `count` is clamped to
/// `1..=`[`MAX_CASCADES`] and `shadow_distance` to a positive number, so a
/// nonsense setting degrades to one cascade rather than producing an empty
/// schedule that would silently disable shadows.
pub fn cascade_splits(shadow_distance: f32, count: usize, exponent: f32) -> Vec<f32> {
    let count = count.clamp(1, MAX_CASCADES);
    let far = if shadow_distance.is_finite() && shadow_distance > 1e-3 {
        shadow_distance
    } else {
        SHADOW_DISTANCE
    };
    let exponent = if exponent.is_finite() && exponent > 0.0 {
        exponent
    } else {
        SPLIT_EXPONENT
    };
    (1..=count)
        .map(|i| far * (i as f32 / count as f32).powf(exponent))
        .collect()
}

/// Shrinks a split schedule to fit inside the camera's own frustum depth.
///
/// A shadow distance past the camera's far plane is an ordinary configuration
/// — the default 200 against a 100-unit camera — and without this every
/// cascade beyond the far plane collapses onto it, because [`slice_frustum`]
/// clamps. Four shadow passes would render three cascades' worth of coverage,
/// with no error and no warning. Unity reaches the same place by clamping
/// shadow distance to the far plane; rescaling keeps all `n` cascades distinct
/// rather than wasting the ones that fell off the end.
///
/// This is a separate step from [`directional_cascade_view_projs`] on purpose.
/// It used to happen *inside* that function, which left the fitted matrices
/// covering ranges the caller's own split list did not describe — and the
/// shader selects a cascade by comparing against that list. A fragment 80
/// units out was assigned a cascade whose matrix reached only 56, landed
/// outside it, and came back lit. One rule, one place.
pub fn clamp_splits_to_frustum(camera_view_proj: Mat4, splits: &[f32]) -> Vec<f32> {
    let corners = frustum_corners_world(camera_view_proj);
    let near_c = (corners[0] + corners[1] + corners[2] + corners[3]) / 4.0;
    let far_c = (corners[4] + corners[5] + corners[6] + corners[7]) / 4.0;
    let depth = (far_c - near_c).length();
    let reach = splits.last().copied().unwrap_or(0.0);
    if !depth.is_finite() || reach <= 1e-6 || reach <= depth {
        return splits.to_vec();
    }
    let scale = depth / reach;
    splits.iter().map(|s| s * scale).collect()
}

/// One orthographic view-projection per cascade, each fitted to its own slice
/// of the camera's frustum.
///
/// This is the single-fit [`directional_light_view_proj`] repeated over
/// [`cascade_splits`]'s ranges. Each cascade is fitted and texel-snapped
/// independently, which is the property that makes cascades worth having: the
/// near cascade's sphere is small, so its texels are dense, regardless of how
/// far the last cascade reaches.
pub fn directional_cascade_view_projs(
    light_dir: Vec3,
    camera_view_proj: Mat4,
    splits: &[f32],
) -> Vec<Mat4> {
    let corners = frustum_corners_world(camera_view_proj);
    let mut near = 0.0;
    splits
        .iter()
        .map(|&far| {
            let vp = fit_light_to_corners(light_dir, slice_frustum(corners, near, far));
            // Cascades abut rather than overlap: each starts where the last
            // ended. A gap here would be a band of world at a cascade boundary
            // that no map covers, which reads as a stripe of missing shadow.
            near = far;
            vp
        })
        .collect()
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
    let corners = slice_frustum(
        frustum_corners_world(camera_view_proj),
        0.0,
        SHADOW_DISTANCE,
    );
    fit_light_to_corners(light_dir, corners)
}

/// Fits a directional light's orthographic view-projection around eight
/// world-space points, as a texel-snapped bounding sphere.
///
/// Shared by the single fit and every cascade, deliberately: a cascade that
/// fitted or snapped by even slightly different rules than its neighbour would
/// show the difference as a visible seam at the boundary, and the two rules
/// would have to be kept in agreement by hand.
pub fn fit_light_to_corners(light_dir: Vec3, corners: [Vec3; 8]) -> Mat4 {
    let dir = light_dir.normalize_or(Vec3::NEG_Y);
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
        camera_with_far(eye, target, 100.0)
    }

    /// The same, with the far plane as an argument.
    ///
    /// Cascade tests need a camera that can actually see as far as the shadow
    /// distance reaches. A 100-unit far plane against a 200-unit shadow
    /// distance is a real configuration, but it exercises the rescaling path
    /// rather than the ordinary one, so it belongs in its own test.
    fn camera_with_far(eye: Vec3, target: Vec3, far: f32) -> Mat4 {
        Mat4::perspective_rh(60.0_f32.to_radians(), 16.0 / 9.0, 0.1, far)
            * Mat4::look_at_rh(eye, target, Vec3::Y)
    }

    /// A single fit over an explicit distance.
    ///
    /// The two region tests below name the distance they assume instead of
    /// reading [`SHADOW_DISTANCE`]. Raising that constant from 50 to 200 for
    /// cascades made their "must not be shadowed" points fall *inside* the
    /// region — they failed, correctly, but for a reason that had nothing to
    /// do with the property they guard. A test's premise should not be a
    /// tunable someone else is free to change.
    fn fit_at(light_dir: Vec3, cam: Mat4, distance: f32) -> Mat4 {
        fit_light_to_corners(
            light_dir,
            slice_frustum(frustum_corners_world(cam), 0.0, distance),
        )
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
        let vp = fit_at(SUN, camera(far + Vec3::new(0.0, 4.0, 7.0), far), 50.0);
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
        let vp = fit_at(SUN, camera(Vec3::new(0.0, 4.0, 7.0), Vec3::ZERO), 50.0);
        assert!(shadowed(vp, Vec3::ZERO), "the origin is being looked at");
        assert!(
            !shadowed(vp, Vec3::new(140.0, 0.0, 0.0)),
            "x = 140 is far outside the 50 units this fit was asked for and must \
             fall outside it"
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
    fn splits_end_at_the_shadow_distance_and_pack_towards_the_camera() {
        let splits = cascade_splits(200.0, 4, SPLIT_EXPONENT);
        assert_eq!(splits.len(), 4);
        assert!(
            (splits[3] - 200.0).abs() < 1e-3,
            "the last cascade must reach exactly the shadow distance, not {}",
            splits[3]
        );
        // Strictly increasing, and each cascade wider than the one before it.
        // Equal widths would mean the exponent was ignored, which is the whole
        // mechanism.
        let mut prev_far = 0.0;
        let mut prev_width = 0.0;
        for (i, &far) in splits.iter().enumerate() {
            let width = far - prev_far;
            assert!(far > prev_far, "cascade {i} does not advance: {splits:?}");
            assert!(
                width > prev_width,
                "cascade {i} is no wider than the one before it ({width} vs \
                 {prev_width}); cascades must grow away from the camera or the \
                 near field gains no density: {splits:?}"
            );
            prev_far = far;
            prev_width = width;
        }
    }

    #[test]
    fn the_split_exponent_default_tracks_unity_s_four_cascade_defaults() {
        // Unity's 4-cascade defaults are 6.7 / 20 / 46.7 / 100 percent of the
        // shadow distance. Asserted because the doc comment claims the shapes
        // agree, and a claim in a comment nobody checks is just a hope.
        let got: Vec<f32> = cascade_splits(100.0, 4, SPLIT_EXPONENT);
        let unity = [6.7_f32, 20.0, 46.7, 100.0];
        for (i, (&g, &u)) in got.iter().zip(unity.iter()).enumerate() {
            assert!(
                (g - u).abs() < 10.0,
                "cascade {i} boundary {g}% is not within 10 points of Unity's {u}%"
            );
        }
    }

    #[test]
    fn a_nonsense_cascade_count_degrades_to_one_rather_than_none() {
        // An empty schedule would disable shadows entirely while looking like
        // a successful call.
        assert_eq!(cascade_splits(200.0, 0, 2.0).len(), 1);
        assert_eq!(cascade_splits(200.0, 99, 2.0).len(), MAX_CASCADES);
        assert_eq!(cascade_splits(f32::NAN, 2, 2.0).len(), 2);
        for s in cascade_splits(-5.0, 3, f32::INFINITY) {
            assert!(s.is_finite() && s > 0.0, "degenerate input produced {s}");
        }
    }

    /// The reason cascades exist: the near cascade must cover less world per
    /// texel than the far one. If every cascade fitted the same size sphere,
    /// four maps would buy exactly nothing over one.
    #[test]
    fn the_near_cascade_is_denser_than_the_far_one() {
        let eye = Vec3::new(10.0, 4.0, 10.0);
        // Far plane past the shadow distance, so this measures the ordinary
        // case rather than the rescaling one.
        let cam = camera_with_far(eye, eye + Vec3::new(0.0, 0.0, -1.0), 500.0);
        let splits = cascade_splits(200.0, 4, SPLIT_EXPONENT);
        let vps = directional_cascade_view_projs(SUN, cam, &splits);
        assert_eq!(vps.len(), 4);
        // Projected width of a fixed world-space segment: bigger means the
        // cascade packs more map into less world, i.e. denser.
        let density = |vp: Mat4| {
            let a = vp * Vec3::ZERO.extend(1.0);
            let b = vp * Vec3::X.extend(1.0);
            (b.x / b.w - a.x / a.w).abs()
        };
        let mut prev = f32::INFINITY;
        for (i, vp) in vps.iter().enumerate() {
            let d = density(*vp);
            assert!(
                d < prev,
                "cascade {i} is no less dense than cascade {}: {d} vs {prev}. \
                 Cascades that all fit the same volume buy nothing over a \
                 single map.",
                i.saturating_sub(1)
            );
            prev = d;
        }
        // And the near cascade must beat what a single fit over the whole
        // shadow distance manages, which is the whole claim of the feature.
        let single = density(fit_at(SUN, cam, 200.0));
        assert!(
            density(vps[0]) > single * 2.0,
            "cascade 0 ({}) should be far denser than one fit over the whole \
             shadow distance ({single})",
            density(vps[0])
        );
    }

    #[test]
    fn cascades_abut_so_no_band_of_world_is_uncovered() {
        let eye = Vec3::new(0.0, 3.0, 0.0);
        let cam = camera_with_far(eye, eye + Vec3::new(0.0, 0.0, -1.0), 500.0);
        let splits = cascade_splits(200.0, 4, SPLIT_EXPONENT);
        let vps = directional_cascade_view_projs(SUN, cam, &splits);
        // Sample along the view axis through every boundary. Each point must
        // land inside at least one cascade; a gap is a stripe with no shadow.
        for i in 0..=400 {
            let d = i as f32 * 0.5;
            if d < 0.5 || d > splits[3] {
                continue;
            }
            let p = eye + Vec3::new(0.0, 0.0, -d);
            assert!(
                vps.iter().any(|vp| shadowed(*vp, p)),
                "nothing covers {d} units down the view axis, but it is inside \
                 the {} unit shadow distance",
                splits[3]
            );
        }
    }

    /// A shadow distance that reaches past the camera's far plane must still
    /// produce `n` distinct cascades.
    ///
    /// Found by a test, not by reading: with a 100-unit camera and the default
    /// 200-unit shadow distance, `slice_frustum`'s clamping made cascades 3
    /// and 4 collapse onto the far plane — four shadow passes rendering three
    /// cascades' worth of coverage, at no error and no warning.
    #[test]
    fn a_shadow_distance_past_the_camera_far_plane_keeps_cascades_distinct() {
        let eye = Vec3::new(0.0, 3.0, 0.0);
        let cam = camera_with_far(eye, eye + Vec3::new(0.0, 0.0, -1.0), 100.0);
        // Through `DirectionalCascades`, not the raw fit: clamping is its own
        // step now, precisely so the splits a caller keeps are the ones its
        // matrices cover. Calling the fit with unclamped splits is what the
        // shader used to do by accident.
        let vps = DirectionalCascades::new(SUN, cam, 200.0, 4, 0.0).view_projs;
        let density = |vp: Mat4| {
            let a = vp * Vec3::ZERO.extend(1.0);
            let b = vp * Vec3::X.extend(1.0);
            (b.x / b.w - a.x / a.w).abs()
        };
        let mut prev = f32::INFINITY;
        for (i, vp) in vps.iter().enumerate() {
            let d = density(*vp);
            assert!(
                d < prev * 0.999,
                "cascade {i} is not meaningfully less dense than the last \
                 ({d} vs {prev}); the shadow distance overran the camera's far \
                 plane and the cascades collapsed onto it"
            );
            prev = d;
        }
    }

    /// The property whose absence let a real bug through: for every cascade,
    /// the range `splits` advertises must be inside the matrix `view_projs`
    /// holds for it.
    ///
    /// The two were computed by different code paths — the fit rescaled the
    /// ranges to the camera's frustum, the split list did not — so the shader
    /// selected cascade *i* for a depth that cascade *i*'s matrix did not
    /// cover, and the fragment came back lit. A pixel test caught it; this
    /// catches it without a GPU.
    #[test]
    fn every_cascade_covers_the_range_its_split_advertises() {
        for far_plane in [100.0_f32, 500.0] {
            let eye = Vec3::new(0.0, 3.0, 0.0);
            let cam = camera_with_far(eye, eye + Vec3::new(0.0, 0.0, -1.0), far_plane);
            let c = DirectionalCascades::new(SUN, cam, 200.0, 4, 0.0);
            let mut near = 0.0;
            for (i, &far) in c.splits.iter().enumerate() {
                // Sample inside the advertised range, away from its edges so
                // this is about coverage and not about boundary rounding.
                for t in [0.25_f32, 0.5, 0.75] {
                    let d = near + (far - near) * t;
                    let p = eye + Vec3::new(0.0, 0.0, -d);
                    assert!(
                        shadowed(c.view_projs[i], p),
                        "cascade {i} advertises {near}..{far} but its matrix does \
                         not cover {d} (camera far plane {far_plane}). The shader \
                         selects by the advertised range, so anything it does not \
                         cover comes back lit."
                    );
                }
                near = far;
            }
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

/// Everything the directional shadow pass and the lighting shaders need to
/// agree on for one frame.
///
/// One value rather than four loose parameters because they have to be
/// consistent: the matrices, the distances the shader compares against to pick
/// between them, and how many of them are live. A caller that passed matrices
/// from one frame and splits from another would get shadows sampled out of the
/// wrong cascade, which looks like flickering rather than like an error.
#[derive(Debug, Clone)]
pub struct DirectionalCascades {
    /// One view-projection per live cascade, near to far.
    pub view_projs: Vec<Mat4>,
    /// Far distance of each cascade, matching `view_projs` element for element.
    pub splits: Vec<f32>,
    /// Cross-fade width at each boundary, as a fraction of that cascade's far
    /// distance. Zero switches hard.
    pub blend: f32,
}

impl DirectionalCascades {
    /// Fits cascades to a camera for one frame.
    pub fn new(
        light_dir: Vec3,
        camera_view_proj: Mat4,
        shadow_distance: f32,
        count: usize,
        blend: f32,
    ) -> Self {
        // Clamped before the fit, and the clamped list is what gets stored:
        // the shader picks a cascade by comparing depth against these numbers,
        // so they have to be the ranges the matrices actually cover.
        let splits = clamp_splits_to_frustum(
            camera_view_proj,
            &cascade_splits(shadow_distance, count, SPLIT_EXPONENT),
        );
        let view_projs = directional_cascade_view_projs(light_dir, camera_view_proj, &splits);
        Self {
            view_projs,
            splits,
            blend: if blend.is_finite() {
                blend.clamp(0.0, 1.0)
            } else {
                0.0
            },
        }
    }

    /// The cascade a probe capture should sample: the widest, since a probe
    /// gathers light from all around itself rather than from a camera's near
    /// field.
    pub fn widest(&self) -> (Mat4, u32) {
        let last = self.view_projs.len().saturating_sub(1);
        (
            self.view_projs.get(last).copied().unwrap_or(Mat4::IDENTITY),
            last as u32,
        )
    }
}
