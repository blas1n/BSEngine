//! The GI albedo and indirect targets the opaque pass writes for
//! screen-space GI (`post_process::GI_ALBEDO_FORMAT`, `GI_INDIRECT_FORMAT`):
//! per pixel, the indirect diffuse light the surface was given and the
//! reflectance it was multiplied by -- held to the shading's own terms, and
//! zero wherever the opaque mesh pass did not draw.
//!
//! The fixture: a square about 40 pixels across in the middle of the frame,
//! nothing behind it, lit by the flat ambient alone.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, ToneMap, ToneMappingMode};
use common::{Draw, Harness, Light, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const ALBEDO: Vec3 = Vec3::new(0.6, 0.3, 0.15);
const AMBIENT: Vec3 = Vec3::new(0.5, 0.25, 1.0);

/// A unit square facing the camera (+z), white at every vertex -- the
/// harness cube's faces are tinted, and the vertex colour multiplies the
/// albedo, which is the very thing these tests read back.
fn white_square(h: &mut Harness) -> u64 {
    let v = |x: f32, y: f32| bsengine_rhi_wgpu::Vertex {
        position: [x, y, 0.0],
        color: [1.0, 1.0, 1.0],
        normal: [0.0, 0.0, 1.0],
        uv: [x + 0.5, 0.5 - y],
        uv2: [0.0, 0.0],
        tangent: [1.0, 0.0, 0.0, 1.0],
    };
    h.mesh(
        &[v(-0.5, -0.5), v(0.5, -0.5), v(0.5, 0.5), v(-0.5, 0.5)],
        &[0, 1, 2, 0, 2, 3],
    )
}

fn square(cube: u64) -> Draw {
    Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(6.0, 6.0, 1.0), Vec3::new(0.0, 0.0, -14.7))
        .colour(ALBEDO)
        .roughness(1.0)
}

fn scene(draws: Vec<Draw>) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient: AMBIENT,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ..Scene::default()
    }
}

fn at(v: &[[f32; 3]], x: u32, y: u32) -> Vec3 {
    Vec3::from(v[(y * WIDTH + x) as usize])
}

fn centre(v: &[[f32; 3]]) -> Vec3 {
    at(v, WIDTH / 2, HEIGHT / 2)
}

fn linear() -> bsengine_core::TextureImportSettings {
    bsengine_core::TextureImportSettings {
        srgb: false,
        mipmaps: false,
        ..Default::default()
    }
}

/// Under the flat ambient the surface's indirect light is `ambient * albedo`
/// and the reflectance it was multiplied by is the albedo -- both written
/// where the square is, and zero around it, where nothing opaque drew.
#[test]
fn the_opaque_pass_records_its_indirect_light_and_albedo() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    h.render(&scene(vec![square(cube)]));
    let (albedo, indirect) = h.gi();
    let a = centre(&albedo);
    assert!((a - ALBEDO).abs().max_element() < 1e-3, "albedo {a}");
    let i = centre(&indirect);
    let want = AMBIENT * ALBEDO;
    assert!(
        (i - want).abs().max_element() < 2e-3,
        "indirect {i} vs {want}"
    );
    for (x, y) in [(2, 2), (WIDTH - 3, HEIGHT / 2), (WIDTH / 2, HEIGHT - 3)] {
        assert_eq!(at(&albedo, x, y), Vec3::ZERO, "({x}, {y}) has no surface");
        assert_eq!(at(&indirect, x, y), Vec3::ZERO, "({x}, {y}) has no surface");
    }
}

/// Under the sky's light the reflectance is the albedo's diffuse share,
/// `(1 - F) (1 - metallic)`: a little under the albedo for a dielectric, and
/// nothing for a metal, which has no diffuse light to replace -- its
/// indirect diffuse is zero too. And the indirect light is that reflectance
/// times one irradiance for all three channels' worth of surface: the two
/// targets are the same light's two halves.
#[test]
fn under_the_sky_the_reflectance_is_the_diffuse_share() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    h.set_test_skybox([160, 180, 220, 255]);
    let with_sky = |draw: Draw| Scene {
        with_skybox: true,
        ..scene(vec![draw])
    };
    h.render(&with_sky(square(cube)));
    let (albedo, indirect) = h.gi();
    let a = centre(&albedo);
    let ratio = a / ALBEDO;
    assert!(
        ratio.min_element() > 0.9 && ratio.max_element() < 0.99,
        "the diffuse share of a dielectric: {ratio}"
    );
    assert!(
        (ratio.x - ratio.y).abs() < 0.02 && (ratio.y - ratio.z).abs() < 0.02,
        "one share for every channel of a dielectric: {ratio}"
    );
    let i = centre(&indirect);
    assert!(i.min_element() > 0.0, "premise: the sky lights it: {i}");

    h.render(&with_sky(square(cube).metallic(1.0)));
    let (albedo, indirect) = h.gi();
    assert!(centre(&albedo).max_element() < 1e-4, "{}", centre(&albedo));
    assert!(
        centre(&indirect).max_element() < 1e-4,
        "{}",
        centre(&indirect)
    );
}

/// The occlusion map darkens the indirect light, and so the reflectance it
/// came through: both halve under an occlusion of a half.
#[test]
fn occlusion_darkens_both_halves() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    let half = h.texture_with(linear(), 1, 1, &[128, 255, 0, 255]);
    let none = h.texture_with(linear(), 1, 1, &[255, 255, 0, 255]);
    h.render(&scene(vec![square(cube).orm_mapped(none, 1.0)]));
    let (a1, i1) = h.gi();
    h.render(&scene(vec![square(cube).orm_mapped(half, 1.0)]));
    let (a2, i2) = h.gi();
    let k = 128.0 / 255.0;
    assert!(
        ((centre(&a2) / centre(&a1)) - Vec3::splat(k))
            .abs()
            .max_element()
            < 0.02,
        "{} vs {}",
        centre(&a2),
        centre(&a1)
    );
    assert!(
        ((centre(&i2) / centre(&i1)) - Vec3::splat(k))
            .abs()
            .max_element()
            < 0.01,
        "{} vs {}",
        centre(&i2),
        centre(&i1)
    );
}

/// A translucent pane over the square leaves the square's values: the pane
/// is drawn by the transparent pass, whose writes to these targets are
/// masked off -- screen-space GI replaces the light of what the pane shows,
/// not of the pane.
#[test]
fn a_translucent_pane_leaves_what_is_behind_it() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    h.render(&scene(vec![square(cube)]));
    let (a1, i1) = h.gi();
    let pane = Draw::new(cube, Vec3::ZERO)
        .scaled(Vec3::new(8.0, 8.0, 0.1), Vec3::new(0.0, 0.0, -14.0))
        .colour(Vec3::new(0.1, 0.9, 0.1))
        .opacity(0.5);
    h.render(&scene(vec![square(cube), pane]));
    let (a2, i2) = h.gi();
    assert_eq!(centre(&a2), centre(&a1));
    assert_eq!(centre(&i2), centre(&i1));
}

/// Under MSAA the two targets resolve like the normal does: inside the
/// square, the same values as without -- after a frame of another colour,
/// so a target that did not resolve shows that frame's.
#[test]
fn msaa_resolves_both_targets() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    h.render(&scene(vec![square(cube)]));
    let (a1, i1) = h.gi();
    // Another colour first, without MSAA: a resolve that did not happen
    // would leave these values behind, not the ones expected.
    h.render(&scene(vec![square(cube).colour(Vec3::new(0.1, 0.8, 0.5))]));
    h.render(&Scene {
        msaa: 4,
        ..scene(vec![square(cube)])
    });
    let (a4, i4) = h.gi();
    assert!(
        centre(&a4).max_element() > 0.1,
        "premise: the square was drawn under MSAA: {}",
        centre(&a4)
    );
    assert!((centre(&a4) - centre(&a1)).abs().max_element() < 1e-3);
    assert!((centre(&i4) - centre(&i1)).abs().max_element() < 1e-3);
}

/// A custom shader's surface records nothing: it computes no such terms, so
/// screen-space GI must leave it alone (its targets are masked off).
#[test]
fn a_custom_shader_records_nothing() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    let shader = h.constant_colour_shader([0.2, 0.7, 0.4], "gi targets");
    h.render(&scene(vec![square(cube).shader(&shader)]));
    let p = h.render(&scene(vec![square(cube).shader(&shader)]));
    assert!(
        p.centre()[1] > 100,
        "premise: the custom shader drew: {:?}",
        p.centre()
    );
    let (albedo, indirect) = h.gi();
    assert_eq!(centre(&albedo), Vec3::ZERO);
    assert_eq!(centre(&indirect), Vec3::ZERO);
}

fn linear_rgbm(rgb: f32) -> [u8; 4] {
    // A grey `rgb` as RGBM in `LIGHTMAP_RGBM_RANGE`: the multiplier is the
    // value over the range, the channels full.
    let m = ((rgb / bsengine_core::LIGHTMAP_RGBM_RANGE) * 255.0).round() as u8;
    [255, 255, 255, m]
}

/// The sky's irradiance is not the only indirect light that has a diffuse
/// share: the light probes' and the lightmap's are multiplied by it too, and
/// the reflectance recorded is that share, not the bare albedo. The
/// lightmap's is a closed form -- a uniform atlas of 0.4 gives an indirect
/// light of the reflectance times 0.4 -- and the probes' is held to the same
/// share as the sky's.
#[test]
fn probes_and_lightmaps_record_the_diffuse_share() {
    let mut h = Harness::new();
    let cube = white_square(&mut h);
    let texel = linear_rgbm(0.4);
    let atlas = h.texture_with(linear(), 1, 1, &texel);
    let e = f32::from(texel[3]) / 255.0 * bsengine_core::LIGHTMAP_RGBM_RANGE;
    h.render(&scene(vec![
        square(cube).lightmapped(atlas, [1.0, 1.0, 0.0, 0.0])
    ]));
    let (albedo, indirect) = h.gi();
    let ratio = centre(&albedo) / ALBEDO;
    assert!(
        ratio.min_element() > 0.9 && ratio.max_element() < 0.99,
        "the lightmap's diffuse share: {ratio}"
    );
    let want = centre(&albedo) * e;
    assert!(
        (centre(&indirect) - want).abs().max_element() < 2e-3,
        "the lightmap's light through it: {} vs {want}",
        centre(&indirect)
    );

    let probes = Scene {
        light_probes: Some(bsengine_core::LightProbeVolume {
            half_extents: Vec3::splat(20.0).into(),
            resolution: [2, 2, 2],
            ..Default::default()
        }),
        ..scene(vec![square(cube)])
    };
    h.render(&probes);
    let (albedo, indirect) = h.gi();
    let ratio = centre(&albedo) / ALBEDO;
    assert!(
        ratio.min_element() > 0.9 && ratio.max_element() < 0.99,
        "the probes' diffuse share: {ratio}"
    );
    assert!(
        centre(&indirect).min_element() > 0.0,
        "premise: the probes light it"
    );
}
