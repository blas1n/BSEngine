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
        tangent: [0.0; 4],
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

/// The four light kinds a scene can bake.
#[derive(Clone, Copy, Debug)]
enum Kind {
    Sun,
    Point,
    Spot,
    Rect,
}

/// One light of `kind` in front of the quad, shining at it, flagged `baked`
/// or not, on a scene with no ambient.
fn lit_by(kind: Kind, baked: bool, draws: Vec<Draw>) -> Scene {
    let mut scene = scene(draws, Vec3::ZERO);
    let ahead = Vec3::new(0.0, 0.0, 2.0);
    match kind {
        Kind::Sun => {
            scene.light.direction = Vec3::NEG_Z;
            scene.light.color = Vec3::ONE;
            scene.light.sun_baked = baked;
        }
        Kind::Point => scene
            .light
            .point_entries
            .push(bsengine_rhi_wgpu::PointLightEntry {
                position: ahead,
                color: Vec3::ONE,
                intensity: 3.0,
                range: 10.0,
                rotation: glam::Quat::IDENTITY,
                cookie: None,
                baked,
            }),
        Kind::Spot => scene.light.spots.push(bsengine_rhi_wgpu::SpotLightEntry {
            position: ahead,
            direction: Vec3::NEG_Z,
            right: Vec3::X,
            color: Vec3::ONE,
            intensity: 3.0,
            range: 10.0,
            inner_angle: 40f32.to_radians(),
            outer_angle: 45f32.to_radians(),
            cookie: None,
            cookie_is_profile: false,
            shadows: false,
            baked,
        }),
        // half_width x half_height = X x Y = +Z, so it shines out of -Z: at
        // the quad.
        Kind::Rect => scene
            .light
            .rects
            .push(bsengine_rhi_wgpu::area_light::RectLightEntry {
                position: ahead,
                half_width: Vec3::X * 0.5,
                half_height: Vec3::Y * 0.5,
                color: Vec3::ONE,
                intensity: 4.0,
                range: 10.0,
                shadows: false,
                baked,
            }),
    }
    scene
}

/// `full` mode at run time: a lightmapped surface skips every light whose
/// direct light is baked -- its lightmap already holds it -- while a surface
/// without a lightmap is still lit by that light, and a lightmapped one is
/// still lit by a light whose direct light is not baked. For each kind, as
/// each has its own flag and its own loop in the shader. The lightmap here is
/// black, so a lightmapped quad that skipped the light is black.
#[test]
fn a_lightmapped_surface_skips_baked_lights_and_only_those() {
    let mut h = Harness::new();
    let mesh = quad(&mut h);
    let black = atlas(&mut h, [0.0; 3], [0.0; 3]);
    for kind in [Kind::Sun, Kind::Point, Kind::Spot, Kind::Rect] {
        let plain = white_quad(mesh);
        let baked_map = || white_quad(mesh).lightmapped(black, [1.0, 1.0, 0.0, 0.0]);
        let unmapped = rgb(&h.render(&lit_by(kind, true, vec![plain])), CX, CY)[0];
        let realtime = rgb(&h.render(&lit_by(kind, false, vec![baked_map()])), CX, CY)[0];
        let skipped = rgb(&h.render(&lit_by(kind, true, vec![baked_map()])), CX, CY)[0];
        assert!(
            unmapped > 40,
            "{kind:?}: premise -- the light lights a surface with no lightmap: {unmapped}"
        );
        assert!(
            realtime > 40,
            "{kind:?}: a lightmapped surface is still lit by a light that is not baked: {realtime}"
        );
        assert!(
            skipped < 8,
            "{kind:?}: a lightmapped surface skips a baked light: {skipped}"
        );
    }
}

// ---- directional lightmaps ---------------------------------------------

/// The quad of the tests above with `uv` running as `uv2` does, so its
/// generated tangents are a real frame -- +x, handedness -1 (`v` runs down,
/// against `cross(+z, +x) = +y`) -- for a normal map to tilt it in. The
/// first quad's `uv` is one point, which gives no tangent to speak of.
fn bumpable_quad(h: &mut Harness) -> u64 {
    let vertex = |x: f32, y: f32, u: f32, v: f32| Vertex {
        position: [x, y, 0.0],
        color: [1.0; 3],
        normal: [0.0, 0.0, 1.0],
        uv: [u, v],
        uv2: [u, v],
        tangent: [0.0; 4],
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

/// A 16x16 directions texture, every texel `direction`, encoded as the bake
/// writes it (`v * 0.5 + 0.5`, linear).
fn directions(h: &mut Harness, direction: Vec3) -> u64 {
    let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    let texel = [enc(direction.x), enc(direction.y), enc(direction.z), 255];
    let rgba: Vec<u8> = (0..16 * 16).flat_map(|_| texel).collect();
    h.texture_with(
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

/// A one-texel normal map tilting the quad `degrees` toward its tangent (+x).
fn tilt(h: &mut Harness, degrees: f32) -> u64 {
    let (s, c) = degrees.to_radians().sin_cos();
    let enc = |v: f32| ((v * 0.5 + 0.5) * 255.0).round() as u8;
    h.texture_with(
        TextureImportSettings {
            srgb: false,
            mipmaps: false,
            ..TextureImportSettings::default()
        },
        1,
        1,
        &[enc(s), enc(0.0), enc(c), 255],
    )
}

/// The red channel at the centre, back to linear: the sRGB encode the frame
/// went through, undone, so ratios are the shader's.
fn linear_red(p: &Pixels, x: u32) -> f32 {
    let c = rgb(p, x, CY)[0] as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// The baked light, 45 degrees off the quad's normal toward +x, and all of
/// it from there (unit length).
fn from_the_right() -> Vec3 {
    Vec3::new(1.0, 0.0, 1.0).normalize()
}

/// Where the normal is the bake's, a directional lightmap is the plain one,
/// to the bit: `(1 + v.n) / (1 + v.n_bake)` is exactly 1. The hard side --
/// any other reading of the factor (the wrong normal for either, the wrong
/// layer) tilts a flat surface's light.
#[test]
fn a_directional_lightmap_lights_a_flat_surface_as_the_plain_one_does() {
    let mut h = Harness::new();
    let mesh = bumpable_quad(&mut h);
    let lightmap = atlas(&mut h, [0.3; 3], [0.3; 3]);
    let dirs = directions(&mut h, from_the_right());
    let plain = h.render(&scene(
        vec![white_quad(mesh).lightmapped(lightmap, [1.0, 1.0, 0.0, 0.0])],
        Vec3::ZERO,
    ));
    let directional = h.render(&scene(
        vec![white_quad(mesh)
            .lightmapped(lightmap, [1.0, 1.0, 0.0, 0.0])
            .directional(dirs)],
        Vec3::ZERO,
    ));
    assert!(
        rgb(&plain, CX, CY)[0] > 60,
        "premise: the lightmap lights it"
    );
    assert_eq!(
        plain.max_channel_diff(&directional),
        0,
        "a flat surface is lit as the plain lightmap lights it"
    );
}

/// A bumped surface is lit by where the baked light came from: tilted 45
/// degrees toward it, the factor is `2 / (1 + cos 45)` = 1.17; tilted 45
/// degrees away, `1 / (1 + cos 45)` = 0.59 -- closed forms, held to 4%.
/// Premise: without the directions the bumped quad is lit exactly as the
/// flat one (a plain lightmap does not see the normal), so the change is
/// the directions' alone.
#[test]
fn a_bumped_surface_is_lit_by_where_the_baked_light_came_from() {
    let mut h = Harness::new();
    let mesh = bumpable_quad(&mut h);
    let lightmap = atlas(&mut h, [0.3; 3], [0.3; 3]);
    let dirs = directions(&mut h, from_the_right());
    let (toward, away) = (tilt(&mut h, 45.0), tilt(&mut h, -45.0));
    let draw = |bump: Option<u64>, directional: bool| {
        let mut d = white_quad(mesh).lightmapped(lightmap, [1.0, 1.0, 0.0, 0.0]);
        if let Some(b) = bump {
            d = d.normal_mapped(b, 1.0);
        }
        if directional {
            d = d.directional(dirs);
        }
        d
    };
    let red = |h: &mut Harness, d: Draw| linear_red(&h.render(&scene(vec![d], Vec3::ZERO)), CX);
    let flat = red(&mut h, draw(None, true));
    let bumped_plain = red(&mut h, draw(Some(toward), false));
    assert!(
        (bumped_plain - flat).abs() < 0.01 * flat,
        "premise: a plain lightmap ignores the bump: {bumped_plain} vs {flat}"
    );
    let cos45 = std::f32::consts::FRAC_1_SQRT_2;
    let lit = red(&mut h, draw(Some(toward), true)) / flat;
    let shaded = red(&mut h, draw(Some(away), true)) / flat;
    assert!(
        (lit - 2.0 / (1.0 + cos45)).abs() < 0.04 * lit,
        "turned toward the light: {lit} vs {}",
        2.0 / (1.0 + cos45)
    );
    assert!(
        (shaded - 1.0 / (1.0 + cos45)).abs() < 0.04 * shaded,
        "turned away from it: {shaded} vs {}",
        1.0 / (1.0 + cos45)
    );
}

/// Each lightmap's directions are its own layer's: two atlases in one
/// frame, their light from opposite sides, the same bump on both -- each
/// quad lit by its own closed form (`2 / (1 + cos 45)` facing its light,
/// `1 / (1 + cos 45)` at right angles to it), against the same frame drawn
/// without directions. Both values, not "one is the brighter": a quad that
/// read another layer's directions -- or an empty one, `v = -1`, which
/// blacks it out -- still leaves the first brighter than the second.
#[test]
fn each_lightmap_reads_its_own_directions() {
    let mut h = Harness::new();
    let mesh = bumpable_quad(&mut h);
    let (a, b) = (
        atlas(&mut h, [0.3; 3], [0.3; 3]),
        atlas(&mut h, [0.3; 3], [0.3; 3]),
    );
    let right = directions(&mut h, from_the_right());
    let left = directions(&mut h, Vec3::new(-1.0, 0.0, 1.0).normalize());
    let toward = tilt(&mut h, 45.0);
    let quad_at = |x: f32, lm: u64, dirs: Option<u64>| {
        let d = Draw::new(mesh, Vec3::new(x, 0.0, 0.0))
            .scaled(Vec3::splat(0.45), Vec3::new(x, 0.0, 0.0))
            .colour(Vec3::ONE)
            .roughness(1.0)
            .lightmapped(lm, [1.0, 1.0, 0.0, 0.0])
            .normal_mapped(toward, 1.0);
        match dirs {
            Some(dirs) => d.directional(dirs),
            None => d,
        }
    };
    let both = |h: &mut Harness, r: Option<u64>, l: Option<u64>| {
        let p = h.render(&scene(
            vec![quad_at(-0.5, a, r), quad_at(0.5, b, l)],
            Vec3::ZERO,
        ));
        (linear_red(&p, CX - 9), linear_red(&p, CX + 9))
    };
    let (plain_a, plain_b) = both(&mut h, None, None);
    assert!(
        plain_a > 0.05 && plain_b > 0.05,
        "premise: both quads are in view and lit: {plain_a} {plain_b}"
    );
    let (lit_a, lit_b) = both(&mut h, Some(right), Some(left));
    let cos45 = std::f32::consts::FRAC_1_SQRT_2;
    let (facing, across) = (2.0 / (1.0 + cos45), 1.0 / (1.0 + cos45));
    assert!(
        (lit_a / plain_a - facing).abs() < 0.04 * facing,
        "the first faces its light: {} vs {facing}",
        lit_a / plain_a
    );
    assert!(
        (lit_b / plain_b - across).abs() < 0.04 * across,
        "the second is at right angles to its own: {} vs {across}",
        lit_b / plain_b
    );
}
