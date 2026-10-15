//! Baked lightmaps, measured at the pixel.
//!
//! The fixture: a quad facing the camera, lit by nothing but its lightmap --
//! no sun, no ambient, no bloom, no SSAO, tonemap `None` -- so a lit pixel can
//! only be the atlas's doing. The quad is hand-built with a `uv2` that runs
//! left to right across it and a `uv` that is the same single point at every
//! vertex, so a shader that read the wrong coordinate set would paint the
//! whole quad one colour and the left/right tests would say so.
//!
//! Atlases are 16x16 RGBM images made here, asymmetric on purpose: a symmetric
//! or flat one reads the same whichever way it is sampled.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, TextureImportSettings, ToneMap, ToneMappingMode};
use bsengine_rhi_wgpu::Vertex;
use common::{Draw, Harness, Light, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const CX: u32 = WIDTH / 2;
const CY: u32 = HEIGHT / 2;
/// Pixels from the centre to sample at: inside the quad (which spans about 36
/// pixels either side of the centre at this distance), clear of its middle
/// seam. Measured, not assumed: a first version sampled at 30 and 45 and read
/// the background.
const OFF: u32 = 18;
const RANGE: f32 = bsengine_core::LIGHTMAP_RGBM_RANGE;

/// A colour of any brightness as the RGBM texel the baker would write.
fn rgbm(rgb: [f32; 3]) -> [u8; 4] {
    let peak = rgb.iter().cloned().fold(0.0, f32::max);
    let m = ((peak / RANGE).clamp(1e-4, 1.0) * 255.0).ceil() / 255.0;
    let channel = |v: f32| ((v / (m * RANGE)).clamp(0.0, 1.0) * 255.0).round() as u8;
    [
        channel(rgb[0]),
        channel(rgb[1]),
        channel(rgb[2]),
        (m * 255.0).round() as u8,
    ]
}

/// A 16x16 atlas, `left` in the left half and `right` in the right.
fn atlas(h: &mut Harness, left: [f32; 3], right: [f32; 3]) -> u64 {
    let mut rgba = Vec::with_capacity(16 * 16 * 4);
    for _y in 0..16 {
        for x in 0..16 {
            rgba.extend_from_slice(&rgbm(if x < 8 { left } else { right }));
        }
    }
    h.texture_with(
        // Linear, as the sidecar of a real lightmap says: the encoded values
        // are not colours and must not be decoded as sRGB.
        TextureImportSettings {
            srgb: false,
            mipmaps: false,
            ..TextureImportSettings::default()
        },
        16,
        16,
        &rgba,
    )
}

/// A two-unit quad facing the camera. `uv2` runs 0..1 left to right; `uv` is
/// one point, so only `uv2` can tell the halves apart.
fn quad(h: &mut Harness) -> u64 {
    let vertex = |x: f32, y: f32, u: f32, v: f32| Vertex {
        position: [x, y, 0.0],
        color: [1.0; 3],
        normal: [0.0, 0.0, 1.0],
        uv: [0.25, 0.25],
        uv2: [u, v],
    };
    h.mesh(
        &[
            vertex(-1.0, -1.0, 0.0, 1.0),
            vertex(1.0, -1.0, 1.0, 1.0),
            vertex(1.0, 1.0, 1.0, 0.0),
            vertex(-1.0, 1.0, 0.0, 0.0),
        ],
        &[0, 1, 2, 0, 2, 3],
    )
}

fn scene(draws: Vec<Draw>, ambient: Vec3) -> Scene {
    Scene {
        draws,
        light: Light {
            color: Vec3::ZERO,
            ambient,
            ..Light::default()
        },
        bloom: Some(Bloom::default().disabled()),
        ssao: Some(AmbientOcclusion::default().disabled()),
        tone_map: Some(ToneMap::new(ToneMappingMode::None)),
        ..Scene::default()
    }
}

fn white_quad(mesh: u64) -> Draw {
    Draw::new(mesh, Vec3::ZERO).colour(Vec3::ONE).roughness(1.0)
}

fn rgb(p: &Pixels, x: u32, y: u32) -> [u8; 3] {
    let [r, g, b, _] = p.at(x, y);
    [r, g, b]
}

/// The left of the quad is lit by the atlas's left half and the right by its
/// right: red on one side, blue on the other, neither bleeding into the other.
#[test]
fn a_lightmapped_surface_shows_the_atlas_where_its_uv2_points() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let lightmap = atlas(&mut h, [0.5, 0.0, 0.0], [0.0, 0.0, 0.5]);
    let p = h.render(&scene(
        vec![white_quad(mesh).lightmapped(lightmap, [1.0, 1.0, 0.0, 0.0])],
        Vec3::ZERO,
    ));
    let (left, right) = (rgb(&p, CX - OFF, CY), rgb(&p, CX + OFF, CY));
    assert!(
        left[0] > 80 && left[1] < 10 && left[2] < 10,
        "left should be the atlas's red: {left:?}"
    );
    assert!(
        right[2] > 80 && right[0] < 10 && right[1] < 10,
        "right should be the atlas's blue: {right:?}"
    );
}

/// The ambient term is one of the two things a lightmap replaces. A surface
/// with a black lightmap in a brightly ambient-lit scene must go black, not
/// stay as bright as the ambient makes it -- the hard side, which an
/// implementation that adds the lightmap to the ambient (or ignores the
/// lightmap) fails.
#[test]
fn a_lightmap_replaces_the_ambient_light_rather_than_adding_to_it() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let black = atlas(&mut h, [0.0; 3], [0.0; 3]);
    let ambient = Vec3::splat(0.5);

    let plain = h.render(&scene(vec![white_quad(mesh)], ambient));
    assert!(
        rgb(&plain, CX, CY)[0] > 100,
        "premise: without a lightmap the ambient lights the quad: {:?}",
        rgb(&plain, CX, CY)
    );
    let baked = h.render(&scene(
        vec![white_quad(mesh).lightmapped(black, [1.0, 1.0, 0.0, 0.0])],
        ambient,
    ));
    assert!(
        rgb(&baked, CX, CY)[0] < 8,
        "a black lightmap leaves nothing of the ambient: {:?}",
        rgb(&baked, CX, CY)
    );
}

/// A lightmap's brightness is its value: a grey atlas of 0.5 lights the quad
/// about as a flat ambient of 0.5 does (a lightmap also takes the diffuse
/// energy split, `kd`, which at normal incidence on a rough dielectric is a
/// little under one), and 0.25 is darker than 0.5. Pins the decode -- the
/// range and the alpha multiply -- to an absolute level, not just a hue.
#[test]
fn the_rgbm_decode_recovers_the_stored_brightness() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let half = atlas(&mut h, [0.5; 3], [0.5; 3]);
    let quarter = atlas(&mut h, [0.25; 3], [0.25; 3]);

    let reference = h.render(&scene(vec![white_quad(mesh)], Vec3::splat(0.5)));
    let lit_half = h.render(&scene(
        vec![white_quad(mesh).lightmapped(half, [1.0, 1.0, 0.0, 0.0])],
        Vec3::ZERO,
    ));
    let lit_quarter = h.render(&scene(
        vec![white_quad(mesh).lightmapped(quarter, [1.0, 1.0, 0.0, 0.0])],
        Vec3::ZERO,
    ));
    let (want, got, dim) = (
        rgb(&reference, CX, CY)[0] as f32,
        rgb(&lit_half, CX, CY)[0] as f32,
        rgb(&lit_quarter, CX, CY)[0] as f32,
    );
    assert!(want > 100.0, "premise: the reference is lit: {want}");
    assert!(
        (got - want).abs() < want * 0.15,
        "0.5 in the atlas should light like 0.5 ambient: {got} vs {want}"
    );
    assert!(dim < got * 0.8, "0.25 is darker than 0.5: {dim} vs {got}");
}

/// A renderer owns a rectangle of the atlas. With the right half selected
/// the whole quad is blue, and with the left half it is all red -- and the
/// whole-atlas reading of the same quad, run first, is red on one side and
/// blue on the other, so the selection is what changed it.
#[test]
fn scale_and_offset_pick_the_rectangle_a_renderer_owns() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let lightmap = atlas(&mut h, [0.5, 0.0, 0.0], [0.0, 0.0, 0.5]);
    let render = |h: &mut Harness, scale_offset| {
        h.render(&scene(
            vec![white_quad(mesh).lightmapped(lightmap, scale_offset)],
            Vec3::ZERO,
        ))
    };

    let whole = render(&mut h, [1.0, 1.0, 0.0, 0.0]);
    assert!(
        rgb(&whole, CX - OFF, CY)[0] > 80 && rgb(&whole, CX + OFF, CY)[2] > 80,
        "premise: the whole atlas is red then blue"
    );
    // uv2 * 0.5 + 0.5 spans only the right half of the atlas.
    let right = render(&mut h, [0.5, 1.0, 0.5, 0.0]);
    for x in [CX - OFF, CX + OFF] {
        let c = rgb(&right, x, CY);
        assert!(
            c[2] > 80 && c[0] < 10,
            "the right half is blue at {x}: {c:?}"
        );
    }
    // uv2 * 0.5 spans only the left half.
    let left = render(&mut h, [0.5, 1.0, 0.0, 0.0]);
    for x in [CX - OFF, CX + OFF] {
        let c = rgb(&left, x, CY);
        assert!(c[0] > 80 && c[2] < 10, "the left half is red at {x}: {c:?}");
    }
}

/// Two renderers with two atlases each read their own: the layers are kept
/// apart. And a steady scene draws each layer once, not every frame.
#[test]
fn each_atlas_gets_its_own_layer_and_is_drawn_once() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let red = atlas(&mut h, [0.5, 0.0, 0.0], [0.5, 0.0, 0.0]);
    let blue = atlas(&mut h, [0.0, 0.0, 0.5], [0.0, 0.0, 0.5]);
    let side_by_side = scene(
        vec![
            white_quad(mesh)
                .scaled(Vec3::splat(0.5), Vec3::new(-1.0, 0.0, 0.0))
                .lightmapped(red, [1.0, 1.0, 0.0, 0.0]),
            white_quad(mesh)
                .scaled(Vec3::splat(0.5), Vec3::new(1.0, 0.0, 0.0))
                .lightmapped(blue, [1.0, 1.0, 0.0, 0.0]),
        ],
        Vec3::ZERO,
    );
    let before = h.lightmap_layers_drawn();
    let p = h.render(&side_by_side);
    h.render(&side_by_side);
    h.render(&side_by_side);
    let (l, r) = (rgb(&p, CX - 36, CY), rgb(&p, CX + 36, CY));
    assert!(
        l[0] > 80 && l[2] < 10,
        "the left quad is the red atlas: {l:?}"
    );
    assert!(
        r[2] > 80 && r[0] < 10,
        "the right quad is the blue atlas: {r:?}"
    );
    assert_eq!(
        h.lightmap_layers_drawn() - before,
        2,
        "two atlases, three frames: each layer drawn once"
    );
}

/// A lightmap whose texture is not on the GPU is no lightmap: the surface
/// shades as it would without one, rather than going black while it loads.
#[test]
fn a_lightmap_that_has_not_loaded_is_no_lightmap() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let ambient = Vec3::splat(0.5);
    let plain = h.render(&scene(vec![white_quad(mesh)], ambient));
    let missing = h.render(&scene(
        vec![white_quad(mesh).lightmapped(987_654, [1.0, 1.0, 0.0, 0.0])],
        ambient,
    ));
    assert!(
        rgb(&plain, CX, CY)[0] > 100,
        "premise: the ambient lights the quad"
    );
    assert_eq!(
        rgb(&missing, CX, CY),
        rgb(&plain, CX, CY),
        "an id the registry does not hold changes nothing"
    );
}
