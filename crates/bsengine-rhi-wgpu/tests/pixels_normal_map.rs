//! Normal maps: does a tangent-space map turn the shading normal the way it
//! was baked to, in the frame of the mesh's tangents?
//!
//! The scene is one white quad facing +y, its `u` along +x and its `v` along
//! +z, looked at from straight above, lit by a sun and nothing else. A normal
//! map of a single texel tilts the whole quad by a known angle, so the quad's
//! brightness is Lambert's law at a known angle -- and a quad tilted toward
//! the sun by exactly the sun's angle from overhead must look like a flat quad
//! with the sun overhead.

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::{Mat4, Vec3};

/// The quad: 6 x 6 at y = 0, facing +y, `u` = +x and `v` = +z. Its tangents
/// are generated at registration: +x, and -- because `cross(+y, +x)` is -z
/// while `v` runs along +z -- handedness -1. So the map's green (+v) points
/// along +z only if the handedness is honoured.
fn quad(h: &mut Harness) -> u64 {
    let corner = |x: f32, z: f32| bsengine_rhi_wgpu::Vertex {
        position: [x, 0.0, z],
        color: [1.0; 3],
        normal: [0.0, 1.0, 0.0],
        uv: [x / 6.0 + 0.5, z / 6.0 + 0.5],
        uv2: [0.0; 2],
        tangent: [0.0; 4],
    };
    let v = [
        corner(-3.0, -3.0),
        corner(3.0, -3.0),
        corner(3.0, 3.0),
        corner(-3.0, 3.0),
    ];
    // Wound to face +y.
    h.mesh(&v, &[0, 2, 1, 0, 3, 2])
}

/// A one-texel normal map: the tangent-space normal `n`, encoded as an image
/// stores it (`n * 0.5 + 0.5`), and uploaded linear as a normal map is.
fn map(h: &mut Harness, n: Vec3) -> u64 {
    let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    let n = n.normalize();
    h.texture_with(linear(), 1, 1, &[enc(n.x), enc(n.y), enc(n.z), 255])
}

fn linear() -> bsengine_core::TextureImportSettings {
    bsengine_core::TextureImportSettings {
        srgb: false,
        mipmaps: false,
        ..Default::default()
    }
}

/// The tangent-space normal tilted by `degrees` toward +u (`along_v` false)
/// or +v.
fn tilted(degrees: f32, along_v: bool) -> Vec3 {
    let (s, c) = degrees.to_radians().sin_cos();
    if along_v {
        Vec3::new(0.0, s, c)
    } else {
        Vec3::new(s, 0.0, c)
    }
}

/// A sun shining *toward* `-to_light`, no ambient: the quad's brightness is
/// the sun's alone. At half strength, so a lit quad sits in the middle of
/// the tonemapper's curve: at full strength it is in the shoulder, where
/// cos 45 and 1 come out 10 levels apart.
fn sun(to_light: Vec3) -> Light {
    Light {
        direction: -to_light.normalize(),
        color: Vec3::splat(0.5),
        ambient: Vec3::ZERO,
        ..Light::default()
    }
}

/// The angle every test tilts by, and slants the sun by.
const ANGLE: f32 = 45.0;

/// The quad's brightness at the centre of the frame.
fn brightness(h: &mut Harness, draw: Draw, light: Light) -> f32 {
    h.render(&Scene {
        draws: vec![draw.roughness(1.0)],
        light,
        camera_pos: Vec3::new(0.0, 6.0, 0.01),
        ..Scene::default()
    })
    .centre_luma()
}

/// [`ANGLE`] from overhead, toward +x.
fn toward_x() -> Vec3 {
    Vec3::new(ANGLE.to_radians().sin(), ANGLE.to_radians().cos(), 0.0)
}

/// The tangent: a map tilting the normal 45 degrees toward +u turns the quad
/// to face a sun 45 degrees toward +x, so it is as bright as a flat quad
/// under a sun overhead (Lambert's cosine is 1 for both), brighter than the
/// flat quad under that same sun (cos 45), and the map tilted the other way
/// is darker still (cos 90: unlit). A tangent along -x, or along z, or a map read in
/// any frame but the mesh's, fails one of the three.
#[test]
fn a_normal_map_turns_the_surface_toward_the_light_along_its_tangent() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let toward = map(&mut h, tilted(ANGLE, false));
    let away = map(&mut h, tilted(-ANGLE, false));

    let flat_overhead = brightness(&mut h, Draw::new(q, Vec3::ZERO), sun(Vec3::Y));
    let flat = brightness(&mut h, Draw::new(q, Vec3::ZERO), sun(toward_x()));
    let mapped = brightness(
        &mut h,
        Draw::new(q, Vec3::ZERO).normal_mapped(toward, 1.0),
        sun(toward_x()),
    );
    let mapped_away = brightness(
        &mut h,
        Draw::new(q, Vec3::ZERO).normal_mapped(away, 1.0),
        sun(toward_x()),
    );
    assert!(
        flat_overhead > flat + 10.0,
        "premise: the slanted sun is dimmer on a flat quad: {flat_overhead} vs {flat}"
    );
    assert!(
        (mapped - flat_overhead).abs() <= 0.03 * flat_overhead,
        "tilted to face the sun, the quad is lit as if the sun were overhead: \
         {mapped} vs {flat_overhead}"
    );
    assert!(
        mapped_away < flat - 10.0,
        "tilted away, it is darker than flat: {mapped_away} vs {flat}"
    );
}

/// The bitangent, and so the handedness: the quad's `v` runs along +z, which
/// is `cross(normal, tangent) * w` only for the -1 its tangents carry. A map
/// tilting toward +v must then face a sun 45 degrees toward +z -- with the
/// handedness ignored it would face away from it.
#[test]
fn green_tilts_the_normal_along_v_by_the_tangents_handedness() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let toward_v = map(&mut h, tilted(ANGLE, true));
    let to_z = Vec3::new(0.0, ANGLE.to_radians().cos(), ANGLE.to_radians().sin());

    let flat_overhead = brightness(&mut h, Draw::new(q, Vec3::ZERO), sun(Vec3::Y));
    let flat = brightness(&mut h, Draw::new(q, Vec3::ZERO), sun(to_z));
    let mapped = brightness(
        &mut h,
        Draw::new(q, Vec3::ZERO).normal_mapped(toward_v, 1.0),
        sun(to_z),
    );
    assert!(
        flat_overhead > flat + 10.0,
        "premise: {flat_overhead} vs {flat}"
    );
    assert!(
        (mapped - flat_overhead).abs() <= 0.03 * flat_overhead,
        "+v faces the +z sun: {mapped} vs {flat_overhead}"
    );
}

/// `normal_scale` scales the map's tilt: at 0.5 the 45-degree map tilts the
/// quad by atan(0.5 tan 45) = 27 degrees, so it is lit between the flat quad
/// and the full tilt -- strictly, so a scale that is ignored (the full tilt)
/// or applied as zero (flat) both fail.
#[test]
fn the_normal_scale_scales_the_tilt() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let toward = map(&mut h, tilted(ANGLE, false));
    let flat = brightness(&mut h, Draw::new(q, Vec3::ZERO), sun(toward_x()));
    let half = brightness(
        &mut h,
        Draw::new(q, Vec3::ZERO).normal_mapped(toward, 0.5),
        sun(toward_x()),
    );
    let full = brightness(
        &mut h,
        Draw::new(q, Vec3::ZERO).normal_mapped(toward, 1.0),
        sun(toward_x()),
    );
    assert!(
        flat + 3.0 < half && half + 3.0 < full,
        "flat {flat} < half {half} < full {full}"
    );
}

/// The tangent turns with the object: the quad turned a quarter about y has
/// its `u` along -z, so the map that faced it toward a +x sun now faces it
/// toward a -z one. A tangent left in the mesh's own space would still point
/// along +x and face the wrong sun.
#[test]
fn the_tangent_turns_with_the_object() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let toward = map(&mut h, tilted(ANGLE, false));
    let turned = |d: Draw| Draw {
        transform: Mat4::from_rotation_y(std::f32::consts::FRAC_PI_2),
        ..d
    };
    let to_minus_z = Vec3::new(0.0, ANGLE.to_radians().cos(), -ANGLE.to_radians().sin());
    let flat_overhead = brightness(&mut h, turned(Draw::new(q, Vec3::ZERO)), sun(Vec3::Y));
    let mapped = brightness(
        &mut h,
        turned(Draw::new(q, Vec3::ZERO).normal_mapped(toward, 1.0)),
        sun(to_minus_z),
    );
    let along_x = brightness(
        &mut h,
        turned(Draw::new(q, Vec3::ZERO).normal_mapped(toward, 1.0)),
        sun(toward_x()),
    );
    assert!(
        (mapped - flat_overhead).abs() <= 0.03 * flat_overhead,
        "turned, it faces the -z sun: {mapped} vs {flat_overhead}"
    );
    assert!(
        along_x < mapped - 10.0,
        "and not the +x one any more: {along_x} vs {mapped}"
    );
}

/// The tangent is made perpendicular to the normal before the map is read:
/// a tangent leaning into the normal -- what interpolation leaves, and what a
/// morph target that bends the normal but not the tangent leaves more of --
/// must shade exactly as the perpendicular one does. Read as given, the
/// leaning tangent would skew the frame and the quad would miss the sun it
/// was tilted to face.
#[test]
fn a_tangent_leaning_into_the_normal_is_straightened_first() {
    let mut h = Harness::new();
    let leaning = {
        // Leaning 63 degrees into the normal: read as given, the 45-degree
        // map would tilt the quad by only 13 (a lean of 27 degrees, the first
        // fixture, came out 3% dark -- inside the tolerance, so it showed
        // nothing).
        let t = Vec3::new(1.0, 2.0, 0.0).normalize();
        let corner = |x: f32, z: f32| bsengine_rhi_wgpu::Vertex {
            position: [x, 0.0, z],
            color: [1.0; 3],
            normal: [0.0, 1.0, 0.0],
            uv: [x / 6.0 + 0.5, z / 6.0 + 0.5],
            uv2: [0.0; 2],
            // Given, so registration keeps it as it is.
            tangent: [t.x, t.y, t.z, -1.0],
        };
        let v = [
            corner(-3.0, -3.0),
            corner(3.0, -3.0),
            corner(3.0, 3.0),
            corner(-3.0, 3.0),
        ];
        h.mesh(&v, &[0, 2, 1, 0, 3, 2])
    };
    let toward = map(&mut h, tilted(ANGLE, false));
    let flat_overhead = brightness(&mut h, Draw::new(leaning, Vec3::ZERO), sun(Vec3::Y));
    let mapped = brightness(
        &mut h,
        Draw::new(leaning, Vec3::ZERO).normal_mapped(toward, 1.0),
        sun(toward_x()),
    );
    assert!(
        (mapped - flat_overhead).abs() <= 0.03 * flat_overhead,
        "straightened, the tangent is +x and the quad faces the sun: \
         {mapped} vs {flat_overhead}"
    );
}

/// The material's group follows its texture: the map reloaded under the
/// same id with the opposite tilt (a new object behind the id, a new
/// generation) darkens the quad on the next frame. A group cached by id
/// alone would keep the old object bound, and the quad bright.
#[test]
fn a_reloaded_normal_map_is_drawn_the_frame_after() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let id = map(&mut h, tilted(ANGLE, false));
    let draw = || Draw::new(q, Vec3::ZERO).normal_mapped(id, 1.0);
    let before = brightness(&mut h, draw(), sun(toward_x()));
    let away = tilted(-ANGLE, false);
    let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    assert!(h.replace_texture(
        id,
        linear(),
        1,
        1,
        &[enc(away.x), enc(away.y), enc(away.z), 255]
    ));
    let after = brightness(&mut h, draw(), sun(toward_x()));
    assert!(
        after < before - 20.0,
        "the reloaded map tilts it away: {after} vs {before}"
    );
}
