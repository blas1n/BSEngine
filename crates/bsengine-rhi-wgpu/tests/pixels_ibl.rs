//! Image-based lighting, measured at the pixel.
//!
//! The trap this file is written around: an assertion that merely detects *a
//! change* would pass just as happily if IBL simply brightened every frame by
//! a constant. So no test here settles for "the pixels differ". Each one
//! measures a *direction* -- towards the environment's colour, away from it as
//! roughness rises, tinted by albedo on a metal and not on a dielectric -- and
//! each is paired with the opposite-direction case that would catch a uniform
//! brightening.
//!
//! Two deliberate choices about the fixtures:
//!
//! - The mesh is the **plane**, not the cube. `cube_vertices` paints each face
//!   a different vertex colour and those multiply into the albedo, so a cube
//!   could never be given a white or a black albedo to reflect with. The plane
//!   is the one mesh with white vertex colours.
//! - `with_skybox` stays **false** even in the tests that load one. The skybox
//!   is then never drawn into the background, so a green pixel at the centre of
//!   the frame can only have come off the surface. Were the sky drawn, a
//!   surface that had drifted out of frame would leave the test measuring the
//!   sky itself and passing for the wrong reason.

mod common;

use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

/// Straight down at the plane, whose normal is +Y. The reflection vector at the
/// centre pixel is therefore +Y as well, which pins exactly which part of the
/// environment the surface is reflecting.
const OVERHEAD: Vec3 = Vec3::new(0.0, 6.0, 0.01);

/// Direct lighting switched off, so every lit photon in the frame arrives from
/// the environment. Without this the directional light's diffuse and specular
/// swamp the IBL term and none of the measurements below mean anything.
fn ibl_only_light() -> Light {
    Light {
        direction: Vec3::new(0.0, -1.0, 0.0),
        color: Vec3::ZERO,
        ambient: Vec3::splat(0.2),
        points: Vec::new(),
    }
}

/// A plane filling the middle of the frame, with the material under test.
fn surface(plane: u64, metallic: f32, roughness: f32, albedo: Vec3) -> Scene {
    Scene {
        draws: vec![Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(6.0, 1.0, 6.0), Vec3::ZERO)
            .colour(albedo)
            .metallic(metallic)
            .roughness(roughness)],
        light: ibl_only_light(),
        camera_pos: OVERHEAD,
        ..Scene::default()
    }
}

/// Sum of the per-channel gaps between the centre pixel and `target`.
///
/// Per-channel rather than luma on purpose: a frame that got uniformly
/// brighter moves *closer* in luma to a bright environment while moving no
/// closer in hue. Summing the channel gaps only falls when the pixel actually
/// takes on the environment's colour.
fn distance_to(pixels: &Pixels, target: [u8; 3]) -> u32 {
    let p = pixels.centre();
    (0..3).map(|i| p[i].abs_diff(target[i]) as u32).sum()
}

/// An equirectangular sky: a bright cap of `cap_rows` rows around +Y over a
/// dark remainder.
///
/// `v = 0` is the +Y pole in the equirect mapping the skybox shader uses, so
/// the top rows are exactly the patch a flat upward-facing mirror reflects. A
/// mirror sees the cap; a fully rough surface averages the cap against all the
/// darkness around it. That difference is the only thing prefilter mip
/// selection can produce, which is why the roughness test needs a structured
/// sky rather than the flat one.
fn sky_with_a_bright_cap(cap_rows: u32) -> (u32, u32, Vec<u8>) {
    const W: u32 = 64;
    const H: u32 = 32;
    const CAP: [u8; 4] = [0, 255, 0, 255];
    const REST: [u8; 4] = [8, 8, 8, 255];

    let mut data = Vec::with_capacity((W * H * 4) as usize);
    for y in 0..H {
        for _ in 0..W {
            data.extend_from_slice(if y < cap_rows { &CAP } else { &REST });
        }
    }
    (W, H, data)
}

/// The colour of the flat environments used below, as the framebuffer would
/// have to show it for a perfect mirror.
const GREEN_SKY: [u8; 4] = [0, 255, 0, 255];
const WHITE_SKY: [u8; 4] = [255, 255, 255, 255];

#[test]
fn a_smooth_metal_reflects_the_environment_colour() {
    let mut h = Harness::new();
    let plane = h.plane();
    let mirror = surface(plane, 1.0, 0.0, Vec3::ONE);

    let without = h.render(&mirror);
    assert!(!h.has_ibl(), "no skybox has been loaded yet");

    h.set_test_skybox(GREEN_SKY);
    assert!(h.has_ibl(), "loading a skybox should have built the IBL maps");
    let with_sky = h.render(&mirror);

    let env = [GREEN_SKY[0], GREEN_SKY[1], GREEN_SKY[2]];
    let far = distance_to(&without, env);
    let near = distance_to(&with_sky, env);
    let [r0, g0, b0, _] = without.centre();
    let [r1, g1, b1, _] = with_sky.centre();

    // Every channel has to move the right way individually. A frame that had
    // simply been brightened would raise the red and blue channels too, and
    // fail here even though its total distance to a bright green might have
    // fallen.
    assert!(
        r1 <= r0 && g1 >= g0 && b1 <= b0,
        "each channel should move towards the green environment: \
         {:?} -> {:?}",
        [r0, g0, b0],
        [r1, g1, b1]
    );
    assert!(
        near * 4 < far,
        "a roughness-0 metal should end up far closer to the environment's \
         colour: distance {far} without the sky ({:?}), {near} with it ({:?})",
        [r0, g0, b0],
        [r1, g1, b1]
    );
    assert!(
        g1 > 150 && r1 < 40 && b1 < 40,
        "the mirror should read as green rather than merely greener, saw {}",
        with_sky.describe()
    );
}

#[test]
fn a_rough_surface_barely_reflects() {
    let mut h = Harness::new();
    let plane = h.plane();
    let smooth = surface(plane, 1.0, 0.0, Vec3::ONE);
    let rough = surface(plane, 1.0, 1.0, Vec3::ONE);

    // First under a flat sky. Every prefiltered mip of a constant environment
    // is that same constant, so whatever gap appears here is the split-sum
    // BRDF term alone -- nothing to do with mip selection.
    h.set_test_skybox(GREEN_SKY);
    let flat_smooth = h.render(&smooth).centre()[1] as f32;
    let flat_rough = h.render(&rough).centre()[1] as f32;

    // Then under a sky with a bright cap at +Y and darkness everywhere else.
    // The BRDF term is identical to the flat case; any *extra* falloff can
    // only be the rough surface reading a blurrier mip, where the cap has been
    // averaged against the dark surroundings.
    let (w, ht, data) = sky_with_a_bright_cap(4);
    h.set_test_skybox_image(w, ht, &data);
    let capped_smooth = h.render(&smooth).centre()[1] as f32;
    let capped_rough = h.render(&rough).centre()[1] as f32;

    assert!(
        capped_smooth > 150.0,
        "the mirror should still find the bright cap, saw green {capped_smooth}"
    );
    assert!(
        capped_rough * 3.0 < capped_smooth,
        "a roughness-1 surface should show far less of the cap than a mirror: \
         green {capped_rough} against {capped_smooth}"
    );

    let flat_retained = flat_rough / flat_smooth;
    let capped_retained = capped_rough / capped_smooth;
    assert!(
        capped_retained * 2.0 < flat_retained,
        "roughness must cost more under a structured sky than under a flat \
         one -- the difference is the prefilter blur. Retained: {capped_retained} \
         with the cap, {flat_retained} flat. Equal retention means the shader \
         is sampling one mip at every roughness."
    );
}

#[test]
fn a_scene_with_no_skybox_is_pixel_identical() {
    let mut h = Harness::new();
    let plane = h.plane();
    let mirror = surface(plane, 1.0, 0.0, Vec3::ONE);

    let before = h.render(&mirror);
    assert!(!h.has_ibl());

    // The identity check below is only worth anything if this scene is one IBL
    // would visibly change. Prove that first, twice, with two different
    // environments -- otherwise "unchanged" could just mean "insensitive".
    h.set_test_skybox(GREEN_SKY);
    let green = h.render(&mirror);
    assert!(
        green.differs_from(&before) && green.centre()[1] > before.centre()[1] + 60,
        "the green sky should have visibly lit the mirror, saw {}",
        green.describe()
    );
    h.set_test_skybox(WHITE_SKY);
    let white = h.render(&mirror);
    assert!(
        white.differs_from(&green),
        "a different environment should give a different frame"
    );

    h.clear_test_skybox();
    assert!(!h.has_ibl(), "clearing the skybox drops the IBL maps");
    let after = h.render(&mirror);

    assert_eq!(
        after.data, before.data,
        "with no skybox the frame must be byte-for-byte what it was before any \
         IBL existed. It is not -- so the dummy-bound fallback is leaking \
         environment light into every skyboxless scene, which is exactly the \
         'IBL just brightened the whole frame' failure this test exists for."
    );

    // And the fallback must still be the *old expression*, not merely a
    // constant. `light.ambient * albedo` is a product: halving one factor and
    // doubling the other has to land on the same pixel. An IBL term added on
    // top -- which depends on albedo but not on ambient -- would break that.
    let mut dim_albedo = surface(plane, 0.0, 0.5, Vec3::splat(0.5));
    dim_albedo.light.ambient = Vec3::splat(0.4);
    let mut dim_ambient = surface(plane, 0.0, 0.5, Vec3::ONE);
    dim_ambient.light.ambient = Vec3::splat(0.2);
    assert_eq!(
        h.render(&dim_albedo).data,
        h.render(&dim_ambient).data,
        "the no-skybox path should still be exactly `ambient * albedo`"
    );

    // And `light.ambient` must still be *reaching* the surface. Byte-equality
    // above would hold just as well if the fallback had stopped evaluating the
    // ambient term at all -- if, say, `ibl_enabled` were stuck on and every
    // skyboxless frame were quietly lit by the black dummy cube instead. Two
    // ambients, one twice the other, must give two different pixels.
    let mut bright_ambient = surface(plane, 0.0, 0.5, Vec3::ONE);
    bright_ambient.light.ambient = Vec3::splat(0.4);
    let dim = h.render(&dim_ambient).centre_luma();
    let bright = h.render(&bright_ambient).centre_luma();
    assert!(
        bright > dim + 20.0,
        "doubling the ambient should brighten a skyboxless frame: {dim} -> \
         {bright}. It did not, so the flat-ambient fallback is not what the \
         shader is evaluating."
    );
}

#[test]
fn metal_tints_its_reflection_but_a_dielectric_does_not() {
    let mut h = Harness::new();
    let plane = h.plane();

    // A white environment, so any colour in these frames is the material's
    // doing rather than the sky's.
    h.set_test_skybox(WHITE_SKY);

    let red = Vec3::new(1.0, 0.0, 0.0);
    let metal_red = h.render(&surface(plane, 1.0, 0.0, red));
    let metal_black = h.render(&surface(plane, 1.0, 0.0, Vec3::ZERO));
    let dielectric_black = h.render(&surface(plane, 0.0, 0.0, Vec3::ZERO));
    let dielectric_red = h.render(&surface(plane, 0.0, 0.0, red));

    // A metal's f0 *is* its albedo, so a red metal under a white sky reflects
    // red.
    let [mr, mg, mb, _] = metal_red.centre();
    assert!(
        mr > 150 && mg < 20 && mb < 20,
        "a red metal should reflect a white environment as red, saw {}",
        metal_red.describe()
    );

    // The other half of the same claim, and the one that stops "tinted"
    // meaning "brightened": with no albedo a metal has no f0 and so reflects
    // essentially nothing.
    assert!(
        metal_black.centre_luma() < 5.0,
        "a black metal has f0 = 0 and should reflect almost nothing, saw {}",
        metal_black.describe()
    );

    // A dielectric's f0 is 0.04 white regardless of albedo, so the same black
    // surface that killed the metal's reflection still shows a dim, neutral
    // one.
    let [dr, dg, db, _] = dielectric_black.centre();
    let spread = dr.max(dg).max(db) - dr.min(dg).min(db);
    assert!(
        spread <= 3,
        "a dielectric's reflection should be achromatic, saw {}",
        dielectric_black.describe()
    );
    assert!(
        dielectric_black.centre_luma() > 15.0,
        "f0 = 0.04 should still be visible against black, saw {}",
        dielectric_black.describe()
    );
    assert!(
        dielectric_black.centre_luma() > metal_black.centre_luma() + 10.0,
        "the dielectric floor is what separates these two: dielectric {}, \
         metal {}",
        dielectric_black.describe(),
        metal_black.describe()
    );

    // Finally: the dielectric's reflection is albedo-*independent*, not just
    // achromatic. Painting the same surface red must leave the off-albedo
    // channels exactly where the black one put them -- the red arrives through
    // the diffuse term, the neutral reflection through f0.
    let [_, rg, rb, _] = dielectric_red.centre();
    assert!(
        rg.abs_diff(dg) <= 3 && rb.abs_diff(db) <= 3,
        "a dielectric's reflection should not pick up albedo: black {:?} \
         against red {:?}",
        [dr, dg, db],
        dielectric_red.centre()
    );
}
