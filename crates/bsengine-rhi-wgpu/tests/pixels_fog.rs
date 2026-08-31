//! Volumetric fog: is it actually *volumetric*?
//!
//! The assertion that matters here is depth dependence, not haze. An
//! implementation that added a constant everywhere -- no froxel volume, no
//! integration, a single `mix` against a fog colour -- would satisfy "the
//! screen got foggier" and every casual eyeball check. Only a working froxel
//! volume makes a far surface shift several times further than a near one, so
//! that ratio is what the first test measures and the only thing that proves
//! the depth axis works.
//!
//! Every scene here turns bloom, SSAO and tone mapping **off**. All three sit
//! downstream of the fog pass and all three are on by default, and each one
//! would fold its own depth- or brightness-dependent term into a measurement
//! meant to isolate a single pass. With them off, what reaches a pixel is the
//! scene colour, the fog, and the sRGB encode.

mod common;

use common::{Draw, Harness, Light, Pixels, Scene};
use glam::{Mat4, Vec3};

/// The cubes' constant colour, straight into the HDR target.
///
/// A *custom shader* rather than a lit material, deliberately: a lit cube's
/// colour depends on its distance to the light, its normals and the shadow
/// map, so a near and a far cube would start from different colours and the
/// near/far shift comparison would be measuring that difference as much as the
/// fog. A constant-colour shader makes the two cubes' unfogged pixels
/// identical, which leaves depth as the only thing that can separate them.
const CUBE_LINEAR: f32 = 0.6;

/// Cube centres. The near one is two world units in front of the camera, the
/// far one twenty-five -- both inside the frustum, neither overlapping the
/// other on screen.
const NEAR_CENTRE: Vec3 = Vec3::new(-1.0, 0.0, 3.0);
const FAR_CENTRE: Vec3 = Vec3::new(8.0, 0.0, -20.0);

/// The far cube is scaled up so it covers a comparable number of pixels: at
/// twenty-five units a unit cube is about six pixels across, and a sample
/// point on something that small is one bad rounding away from the background.
/// Scale changes nothing about the comparison, because the constant-colour
/// shader makes both cubes render the same value at any size.
const FAR_SCALE: f32 = 8.0;

/// A camera looking at the origin from `+Z`, the harness's default.
const CAMERA: Vec3 = Vec3::new(0.0, 0.0, 5.0);

/// Where a world-space point lands, in pixels.
///
/// Mirrors the harness's own projection (60 degrees, 0.1 to 100) so the sample
/// points are derived from the camera rather than read off a screenshot and
/// pasted in. `cube_sample_is_really_on_the_cube` below is what catches this
/// drifting out of step with `render_frame_at`.
fn screen_of(world: Vec3, camera_pos: Vec3) -> (u32, u32) {
    let proj = Mat4::perspective_rh(
        60.0_f32.to_radians(),
        common::WIDTH as f32 / common::HEIGHT as f32,
        0.1,
        100.0,
    );
    let clip = proj * Mat4::look_at_rh(camera_pos, Vec3::ZERO, Vec3::Y) * world.extend(1.0);
    let ndc = clip.truncate() / clip.w;
    (
        ((ndc.x * 0.5 + 0.5) * common::WIDTH as f32) as u32,
        ((-ndc.y * 0.5 + 0.5) * common::HEIGHT as f32) as u32,
    )
}

/// How far a pixel moved between two frames: the sum of the per-channel
/// differences, in 0-255 units.
///
/// Summed across channels rather than compared per channel because the fog
/// colour is not grey -- a coloured fog moves the three channels by different
/// amounts, and the question here is how far the pixel travelled, not which
/// way.
fn shift(a: &Pixels, b: &Pixels, at: (u32, u32)) -> f32 {
    let (p, q) = (a.at(at.0, at.1), b.at(at.0, at.1));
    (0..3).map(|i| (p[i] as f32 - q[i] as f32).abs()).sum()
}

/// The mesh and the flat shader every scene in this file draws with.
fn fixtures(h: &mut Harness) -> (u64, String) {
    let cube = h.cube();
    let shader = h.constant_colour_shader([CUBE_LINEAR; 3], "fog-flat");
    (cube, shader)
}

/// The two cubes, with everything downstream of the fog pass switched off.
fn two_cubes(cube: u64, shader: &str, fog: Option<bsengine_core::VolumetricFog>) -> Scene {
    Scene {
        draws: vec![
            Draw::new(cube, NEAR_CENTRE).shader(shader),
            Draw::new(cube, FAR_CENTRE)
                .scaled(Vec3::splat(FAR_SCALE), FAR_CENTRE)
                .shader(shader),
        ],
        bloom: Some(bsengine_core::Bloom::default().disabled()),
        ssao: Some(bsengine_core::AmbientOcclusion::default().disabled()),
        tone_map: Some(bsengine_core::ToneMap::default().disabled()),
        fog,
        ..Scene::default()
    }
}

/// Moderate fog: thick enough that twenty-five units is most of the way to
/// opaque, thin enough that two units is nearly clear. Isotropic, so the phase
/// function contributes the same constant everywhere and cannot be what makes
/// the near and far samples differ.
fn moderate_fog() -> bsengine_core::VolumetricFog {
    bsengine_core::VolumetricFog {
        enabled: true,
        density: 0.05,
        // Distinctly red, so "shifted toward the fog colour" is a direction on
        // screen and not just a magnitude.
        color: Vec3::new(1.0, 0.2, 0.2).into(),
        anisotropy: 0.0,
    }
}

#[test]
fn cube_sample_is_really_on_the_cube() {
    // `screen_of` reproduces the harness's projection rather than asking it,
    // so it can silently drift. Sampling the background instead of the cube
    // would make the near/far comparison below meaningless while still
    // producing two numbers to divide, so this pins both sample points to the
    // flat colour the cubes actually render.
    let mut h = Harness::new();
    let (cube, shader) = fixtures(&mut h);
    let plain = h.render(&two_cubes(cube, &shader, None));

    // 0.6 linear through the sRGB encode the offscreen target applies.
    let expected = (1.055 * CUBE_LINEAR.powf(1.0 / 2.4) - 0.055) * 255.0;
    for (label, centre) in [("near", NEAR_CENTRE), ("far", FAR_CENTRE)] {
        let at = screen_of(centre, CAMERA);
        let p = plain.at(at.0, at.1);
        assert!(
            (p[0] as f32 - expected).abs() < 12.0,
            "the {label} cube's sample point {at:?} reads {p:?}, not the flat \
             {expected:.0} the constant-colour shader writes -- the sample is \
             off the cube, so every fog measurement taken there is measuring \
             the background"
        );
    }
}

#[test]
fn fog_affects_distant_surfaces_much_more_than_near_ones() {
    // THE test. Two cubes that render the identical colour when unfogged, one
    // two units away and one twenty-five. A constant haze added everywhere
    // moves both by the same amount; a real froxel volume, integrated front to
    // back along Z, moves the far one far further. The ratio is the proof the
    // depth axis exists.
    let mut h = Harness::new();
    let (cube, shader) = fixtures(&mut h);
    let plain = h.render(&two_cubes(cube, &shader, None));
    let fogged = h.render(&two_cubes(cube, &shader, Some(moderate_fog())));

    let near_at = screen_of(NEAR_CENTRE, CAMERA);
    let far_at = screen_of(FAR_CENTRE, CAMERA);
    let near = shift(&plain, &fogged, near_at);
    let far = shift(&plain, &fogged, far_at);

    // Reported unconditionally: the numbers are the finding, and a test that
    // only prints them on failure hides the margin it is passing by.
    println!(
        "near cube {near_at:?}: {:?} -> {:?}  (shift {near:.1})\n\
         far  cube {far_at:?}: {:?} -> {:?}  (shift {far:.1})\n\
         ratio far/near = {:.2}",
        plain.at(near_at.0, near_at.1),
        fogged.at(near_at.0, near_at.1),
        plain.at(far_at.0, far_at.1),
        fogged.at(far_at.0, far_at.1),
        far / near.max(1e-6),
    );

    assert!(
        near > 2.0,
        "the near cube should still be fogged a little -- a shift of {near} \
         means the fog is not reaching the near field at all, which is a \
         different bug from the one this test is about"
    );
    assert!(
        far > 4.0 * near,
        "the far cube must shift several times further than the near one; got \
         far {far:.1} vs near {near:.1} (ratio {:.2}). Equal shifts mean the \
         fog is a constant added to every pixel and the froxel volume's depth \
         axis is not doing anything: check that the exponential Z mapping \
         agrees between froxel_slice_depth, the injection shader and \
         depth_to_froxel_w, that the integration accumulates along Z, and that \
         fog.enabled reaches the shader",
        far / near.max(1e-6),
    );

    // ...and it moved *toward the fog colour*, not merely somewhere. The fog
    // is red, so the far cube's red channel must end up ahead of its green by
    // more than the near cube's does.
    let redness = |p: &Pixels, at: (u32, u32)| {
        let c = p.at(at.0, at.1);
        c[0] as f32 - c[1] as f32
    };
    let near_red = redness(&fogged, near_at) - redness(&plain, near_at);
    let far_red = redness(&fogged, far_at) - redness(&plain, far_at);
    assert!(
        far_red > near_red + 5.0,
        "the far cube must take on more of the red fog colour than the near \
         one: far gained {far_red:.1} of red-over-green, near gained {near_red:.1}"
    );
}

#[test]
fn zero_density_leaves_the_image_unchanged() {
    // Zero extinction means zero scattering *and* full transmittance, so the
    // apply pass must hand back the scene byte for byte -- not "close enough".
    // A shader that mixed toward the fog colour by a density-independent
    // amount, or an integration that divided by a density it had clamped away
    // from zero, fails exactly here.
    let mut h = Harness::new();
    let (cube, shader) = fixtures(&mut h);
    let off = h.render(&two_cubes(cube, &shader, None));
    let zero = h.render(&two_cubes(
        cube,
        &shader,
        Some(bsengine_core::VolumetricFog {
            enabled: true,
            density: 0.0,
            ..moderate_fog()
        }),
    ));
    assert!(
        !zero.differs_from(&off),
        "density 0 must be pixel-identical to no fog at all; fog-off reads {}, \
         zero-density reads {}",
        off.describe(),
        zero.describe()
    );

    // Non-vacuity: the same harness, the same scene, a real density -- if this
    // did not differ, the equality above would be measuring nothing.
    let real = h.render(&two_cubes(cube, &shader, Some(moderate_fog())));
    assert!(
        real.differs_from(&off),
        "a non-zero density must change the image, or the test above proves \
         nothing about density"
    );
}

#[test]
fn a_scene_with_no_fog_component_is_pixel_identical() {
    // The strongest regression this branch has. Every one of the pre-existing
    // pixel tests renders with no `VolumetricFog` at all, and their reference
    // behaviour is only preserved while the disabled path is an exact
    // passthrough -- not a passthrough through an extra sampler, an extra
    // format conversion, or a `mix` by zero.
    let mut h = Harness::new();
    let (cube, shader) = fixtures(&mut h);
    let absent = h.render(&two_cubes(cube, &shader, None));

    // A component that is present but disabled, carrying deliberately violent
    // parameters. If `enabled` were ignored anywhere -- in the CPU-side guard
    // that skips the dispatch, or in the shader's early return -- this frame
    // would be unmistakable.
    let present_but_off = h.render(&two_cubes(
        cube,
        &shader,
        Some(bsengine_core::VolumetricFog {
            enabled: false,
            density: 0.9,
            color: Vec3::new(0.0, 1.0, 0.0).into(),
            anisotropy: 0.9,
        }),
    ));
    assert!(
        !present_but_off.differs_from(&absent),
        "a disabled VolumetricFog must render exactly as no component at all; \
         absent reads {}, disabled reads {}",
        absent.describe(),
        present_but_off.describe()
    );

    // Non-vacuity again: the same violent parameters, enabled.
    let enabled = h.render(&two_cubes(
        cube,
        &shader,
        Some(bsengine_core::VolumetricFog {
            enabled: true,
            density: 0.9,
            color: Vec3::new(0.0, 1.0, 0.0).into(),
            anisotropy: 0.9,
        }),
    ));
    assert!(
        enabled.differs_from(&absent),
        "enabling that same fog must change the image, or the two equality \
         assertions above are comparing a pass that never runs"
    );
}

#[test]
fn positive_anisotropy_brightens_the_view_toward_the_light() {
    // The phase function, end to end. An empty scene, so every pixel is pure
    // in-scattered light and nothing else, viewed twice: once looking into the
    // directional light and once with it behind the camera. Only the phase
    // function can separate those two frames -- the medium, the density, the
    // colour and the depth range are identical.
    let mut h = Harness::new();

    // The light travels along -Z, so it comes *from* +Z: a camera at +Z looks
    // away from it, and one at -Z looks into it.
    let sun = Light {
        direction: Vec3::new(0.0, 0.0, -1.0),
        ..Light::default()
    };
    let view = |camera_pos: Vec3, anisotropy: f32| Scene {
        draws: Vec::new(),
        light: sun.clone(),
        camera_pos,
        bloom: Some(bsengine_core::Bloom::default().disabled()),
        ssao: Some(bsengine_core::AmbientOcclusion::default().disabled()),
        tone_map: Some(bsengine_core::ToneMap::default().disabled()),
        fog: Some(bsengine_core::VolumetricFog {
            enabled: true,
            density: 0.05,
            color: Vec3::ONE.into(),
            anisotropy,
        }),
        ..Scene::default()
    };
    let toward = Vec3::new(0.0, 0.0, -5.0);
    let away = Vec3::new(0.0, 0.0, 5.0);

    // The control. With g = 0 the phase function is the constant 1/(4*pi), so
    // the two views must read the same. Without this, a renderer that simply
    // made the -Z camera brighter for some unrelated reason would pass the
    // real assertion below.
    let iso_toward = h.render(&view(toward, 0.0)).centre_luma();
    let iso_away = h.render(&view(away, 0.0)).centre_luma();
    assert!(
        (iso_toward - iso_away).abs() < 2.0,
        "isotropic fog must look the same in both directions, got {iso_toward:.1} \
         toward the light and {iso_away:.1} away from it -- something other than \
         the phase function is separating these two views, and the assertion \
         below would inherit it"
    );

    let fwd_toward = h.render(&view(toward, 0.7)).centre_luma();
    let fwd_away = h.render(&view(away, 0.7)).centre_luma();
    println!(
        "anisotropy 0.0: toward {iso_toward:.1}, away {iso_away:.1}\n\
         anisotropy 0.7: toward {fwd_toward:.1}, away {fwd_away:.1}"
    );
    assert!(
        fwd_toward > fwd_away + 10.0,
        "forward scattering (g = 0.7) must brighten the view into the light \
         relative to the view away from it: toward {fwd_toward:.1}, away \
         {fwd_away:.1}. Equal readings mean the Henyey-Greenstein term is not \
         reaching the injection pass, so the fog is uniform grey rather than \
         scattering"
    );
}
