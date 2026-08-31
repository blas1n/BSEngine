//! Light shafts: does the shadow map actually reach the fog?
//!
//! The claim these tests make is **not** "the fog is bright" -- uniform fog is
//! bright too, and the froxel volume shipped bright in sub-step 1/2. The claim
//! is that an occluder standing between the sun and the camera's fog volume
//! makes that fog *darker*, which is the one thing a shadowless injection pass
//! cannot do. Dark gaps where the light does not reach are what makes a beam a
//! beam.
//!
//! Every scene here is deliberately empty of visible geometry. The occluder
//! floats above the camera's frustum, outside it, so the only thing it can
//! change is the light reaching the froxels -- not the pixels it covers, since
//! it covers none. `the_occluder_is_never_drawn_in_the_frame` is what holds
//! that property down; without it a "darker" reading could just be the
//! occluder's own silhouette, and the measurement would be of the wrong thing
//! entirely.
//!
//! Bloom, SSAO and tone mapping are off for the same reason `pixels_fog.rs`
//! turns them off: all three sit downstream of the fog pass, all three are
//! brightness-dependent, and any of them would fold its own curve into a
//! measurement meant to isolate one term.

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

/// The sun travels straight down, so it comes from directly overhead. Straight
/// down rather than the harness's default slant because it makes the shadow
/// volume trivially predictable: everything under the occluder is in shadow,
/// and nothing else is.
const SUN: Vec3 = Vec3::new(0.0, -1.0, 0.0);

/// The occluder's scale: a wide, thin slab. Wide enough (40 x 30) that the
/// froxels along the camera's centre ray sit well inside its shadow rather than
/// near an edge, where the shadow map's own resolution would be what the
/// measurement was actually about.
const SLAB: Vec3 = Vec3::new(40.0, 1.0, 30.0);

/// The occluder between the sun and the fog volume.
///
/// `y = 14` puts it above the camera's frustum: the slab's lowest, furthest
/// corner is at `y = 13.5, z = -10`, which is 15 units along the view axis,
/// where the 60-degree frustum reaches only `y = 8.66`. That is what makes it
/// invisible while still shadowing everything below it.
///
/// Its z span (`5 +- 15`, so -10 to 20) covers the camera at `z = 5` and the
/// first fifteen units of its centre ray -- a little over half of all the light
/// that ray in-scatters, since the far half is attenuated by the near half's
/// transmittance.
const OVERHEAD: Vec3 = Vec3::new(0.0, 14.0, 5.0);

/// The same slab, moved out of the light path.
///
/// `x = 60` spans 40 to 80, entirely outside the shadow map's own 60-unit box
/// (see `common::light_view_proj`), so it occludes nothing the camera can see.
/// Still outside the frustum too: at 15 units the view reaches only `x = 11.5`.
const ASIDE: Vec3 = Vec3::new(60.0, 14.0, 5.0);

/// Isotropic white fog, thick enough to read clearly and thin enough that the
/// far half of the ray still contributes.
///
/// Isotropic on purpose: with `g = 0` the phase function is the constant
/// `1/(4*pi)` in every direction, so it cannot be what separates two of these
/// frames. The only term left that can is the shadow.
fn shaft_fog() -> bsengine_core::VolumetricFog {
    bsengine_core::VolumetricFog {
        enabled: true,
        density: 0.05,
        color: Vec3::ONE.into(),
        anisotropy: 0.0,
    }
}

/// A frame with no visible geometry, optionally an occluder overhead, and
/// optionally fog.
fn scene(cube: u64, occluder: Option<Vec3>, fog: Option<bsengine_core::VolumetricFog>) -> Scene {
    Scene {
        draws: occluder
            .map(|centre| vec![Draw::new(cube, centre).scaled(SLAB, centre)])
            .unwrap_or_default(),
        light: Light {
            direction: SUN,
            ..Light::default()
        },
        bloom: Some(bsengine_core::Bloom::default().disabled()),
        ssao: Some(bsengine_core::AmbientOcclusion::default().disabled()),
        tone_map: Some(bsengine_core::ToneMap::default().disabled()),
        fog,
        ..Scene::default()
    }
}

/// Brightness of the fog at the centre of the frame, where the camera's ray
/// runs the length of the occluder's shadow.
fn fog_luma(h: &mut Harness, occluder: Option<Vec3>, cube: u64) -> f32 {
    h.render(&scene(cube, occluder, Some(shaft_fog())))
        .centre_luma()
}

#[test]
fn the_occluder_is_never_drawn_in_the_frame() {
    // The guard the two tests below rest on, in the spirit of `pixels_fog.rs`'s
    // `cube_sample_is_really_on_the_cube`: it pins where the sample is taken
    // relative to the geometry, so the measurement cannot quietly become one of
    // the occluder's own pixels.
    //
    // Stronger than sampling around it, and cheaper: with fog off, adding the
    // occluder must change *nothing at all*. If any part of the slab were
    // inside the frustum this would fail, and the darkening the next test
    // measures could be the slab's silhouette rather than its shadow.
    let mut h = Harness::new();
    let cube = h.cube();
    let empty = h.render(&scene(cube, None, None));
    let with_occluder = h.render(&scene(cube, Some(OVERHEAD), None));
    assert!(
        !with_occluder.differs_from(&empty),
        "the occluder must fall entirely outside the camera's frustum, so that \
         everything it can change is the light reaching the fog. It is visible \
         somewhere: empty frame reads {}, occluded frame reads {}",
        empty.describe(),
        with_occluder.describe()
    );
}

#[test]
fn an_occluder_between_the_sun_and_the_volume_darkens_the_fog() {
    // THE test for this feature. The same foggy scene twice: once with a slab
    // between the sun and the camera's fog volume, once without it. Nothing the
    // camera can see changes -- the slab is outside the frustum, as the guard
    // above proves -- so the only thing that can move a pixel is how much light
    // reaches the froxels along its ray.
    //
    // A uniform-fog implementation, which is exactly what shipped in sub-step
    // 1/2, produces two identical frames here.
    let mut h = Harness::new();
    let cube = h.cube();

    let unshadowed = fog_luma(&mut h, None, cube);
    let shadowed = fog_luma(&mut h, Some(OVERHEAD), cube);

    // Reported unconditionally: these two numbers are the finding.
    println!(
        "centre fog luma: unshadowed {unshadowed:.1}, shadowed {shadowed:.1} \
         (ratio {:.2})",
        shadowed / unshadowed.max(1e-6)
    );

    assert!(
        unshadowed > 20.0,
        "the unshadowed fog reads {unshadowed:.1}, which is too dark to draw \
         any conclusion from: the comparison below needs the lit case to be \
         clearly lit, or 'darker' is measuring quantisation noise"
    );
    assert!(
        shadowed < unshadowed - 8.0,
        "an occluder between the sun and the fog volume must measurably darken \
         the fog: unshadowed {unshadowed:.1}, shadowed {shadowed:.1}. Equal \
         readings mean the injection pass is ignoring the shadow map, which is \
         fog rather than light shafts -- check that the froxel shadow bind \
         group is bound to the injection pipeline, that `light_view_proj` \
         reaches `FogUniform`, and that the sampled column really lies under \
         the occluder"
    );
}

#[test]
fn moving_the_occluder_out_of_the_light_path_restores_the_brightness() {
    // Without this, an implementation that darkened the fog whenever *any*
    // object existed -- or one that darkened it by a constant -- would pass the
    // test above. The slab is the same size and the same distance overhead;
    // only its position along X changes, taking its shadow with it.
    let mut h = Harness::new();
    let cube = h.cube();

    let unshadowed = fog_luma(&mut h, None, cube);
    let shadowed = fog_luma(&mut h, Some(OVERHEAD), cube);
    let moved_aside = fog_luma(&mut h, Some(ASIDE), cube);

    println!(
        "centre fog luma: no occluder {unshadowed:.1}, overhead {shadowed:.1}, \
         moved aside {moved_aside:.1}"
    );

    assert!(
        shadowed < unshadowed - 8.0,
        "the overhead occluder must still darken the fog ({shadowed:.1} vs \
         {unshadowed:.1}), or the restoration below is restoring nothing"
    );
    assert!(
        (moved_aside - unshadowed).abs() < 2.0,
        "moving the occluder out of the light path must restore the original \
         brightness: no occluder reads {unshadowed:.1}, occluder aside reads \
         {moved_aside:.1}. A difference here means the darkening does not track \
         where the occluder actually is -- a constant dimming would produce it \
         just as well"
    );
}
