//! ORM and emissive maps: does each channel do what glTF says it does?
//!
//! Every assertion here is an equivalence rather than a closed-form value: a
//! map that scales a factor must look exactly like the factor scaled. That
//! is what glTF defines -- roughness is `roughnessFactor * G`, metallic
//! `metallicFactor * B`, emission `emissiveFactor * texture` -- so a frame
//! drawn with the map and one drawn with the product must agree to the last
//! level, and one that does not has the channel or the multiply wrong.

mod common;

use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

/// The quad of `pixels_normal_map.rs`: 6 x 6 facing +y, `u` along +x.
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
    h.mesh(&v, &[0, 2, 1, 0, 3, 2])
}

fn linear() -> bsengine_core::TextureImportSettings {
    bsengine_core::TextureImportSettings {
        srgb: false,
        mipmaps: false,
        ..Default::default()
    }
}

/// A one-texel ORM map: occlusion, roughness and metallic as bytes.
fn orm(h: &mut Harness, o: u8, r: u8, m: u8) -> u64 {
    h.texture_with(linear(), 1, 1, &[o, r, m, 255])
}

/// The quad from overhead with the sun overhead too -- the mirror angle, so
/// a smoother surface shows a brighter highlight -- and some ambient.
fn frame(h: &mut Harness, draw: Draw) -> Pixels {
    h.render(&Scene {
        draws: vec![draw],
        light: Light {
            direction: Vec3::NEG_Y,
            color: Vec3::splat(0.6),
            ambient: Vec3::splat(0.2),
            ..Light::default()
        },
        camera_pos: Vec3::new(0.0, 6.0, 0.01),
        ..Scene::default()
    })
}

/// `max_channel_diff` small enough that the two frames are the same image:
/// 1 level, for the rounding of a factor that went through 8 bits (51/255 is
/// 0.2 to three places).
fn same(a: &Pixels, b: &Pixels, what: &str) {
    let d = a.max_channel_diff(b);
    assert!(d <= 1, "{what}: frames differ by up to {d} levels");
}

/// White in every channel is no map at all: each decodes to exactly 1.0,
/// so the factors stand. A material that names an all-white ORM and
/// emissive map must look exactly as it does without them -- the default
/// group binds white for the same reason.
#[test]
fn white_maps_change_nothing() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let white_orm = orm(&mut h, 255, 255, 255);
    let white = h.texture_with(Default::default(), 1, 1, &[255, 255, 255, 255]);
    let base = || {
        Draw::new(q, Vec3::ZERO)
            .roughness(0.6)
            .metallic(0.3)
            .emissive(Vec3::new(0.1, 0.05, 0.0))
    };
    let plain = frame(&mut h, base());
    let mapped = frame(
        &mut h,
        base().orm_mapped(white_orm, 1.0).emissive_mapped(white),
    );
    same(&plain, &mapped, "white maps");
}

/// G is roughness, multiplying the factor: roughness 1 with G = 0.2 is
/// roughness 0.2. Premise: the two roughnesses really look different here
/// (the mirror-angle highlight), so agreeing is not agreeing by default.
#[test]
fn green_is_roughness_times_the_factor() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let smooth = orm(&mut h, 255, 51, 255);
    let rough = frame(&mut h, Draw::new(q, Vec3::ZERO).roughness(1.0));
    let factored = frame(&mut h, Draw::new(q, Vec3::ZERO).roughness(0.2));
    let mapped = frame(
        &mut h,
        Draw::new(q, Vec3::ZERO)
            .roughness(1.0)
            .orm_mapped(smooth, 1.0),
    );
    assert!(
        rough.max_channel_diff(&factored) > 10,
        "premise: roughness 1 and 0.2 differ here"
    );
    same(&mapped, &factored, "G = 0.2 against roughness 0.2");
}

/// B is metallic, multiplying the factor: metallic 1 with B = 0 is a
/// dielectric. Premise: metal and dielectric look different under this
/// light.
#[test]
fn blue_is_metallic_times_the_factor() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let dielectric = orm(&mut h, 255, 255, 0);
    let base = || Draw::new(q, Vec3::ZERO).roughness(0.4);
    let metal = frame(&mut h, base().metallic(1.0));
    let plain = frame(&mut h, base().metallic(0.0));
    let mapped = frame(&mut h, base().metallic(1.0).orm_mapped(dielectric, 1.0));
    assert!(
        metal.max_channel_diff(&plain) > 10,
        "premise: metal is not plastic here"
    );
    same(&mapped, &plain, "B = 0 against metallic 0");
}

/// R is occlusion, and it darkens only the light from everywhere. With the
/// sun off, R = 0.5 at full strength is the ambient halved; at strength 0
/// it is no occlusion at all; and with the ambient off, the sun's direct
/// light is untouched by R = 0, which glTF reserves occlusion from.
#[test]
fn red_occludes_the_indirect_light_and_only_that() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let half = orm(&mut h, 128, 255, 255);
    let none = orm(&mut h, 0, 255, 255);
    let lit = |h: &mut Harness, ambient: f32, sun: f32, draw: Draw| {
        h.render(&Scene {
            draws: vec![draw.roughness(1.0)],
            light: Light {
                direction: Vec3::NEG_Y,
                color: Vec3::splat(sun),
                ambient: Vec3::splat(ambient),
                ..Light::default()
            },
            camera_pos: Vec3::new(0.0, 6.0, 0.01),
            ..Scene::default()
        })
    };
    let d = || Draw::new(q, Vec3::ZERO);
    // 128/255 = 0.502: the ambient it is held to is 0.6 times that.
    let occluded = lit(&mut h, 0.6, 0.0, d().orm_mapped(half, 1.0));
    let halved = lit(&mut h, 0.6 * 128.0 / 255.0, 0.0, d());
    let full = lit(&mut h, 0.6, 0.0, d());
    assert!(
        full.max_channel_diff(&halved) > 10,
        "premise: halving the ambient shows"
    );
    same(&occluded, &halved, "R = 0.5 against the ambient halved");

    let ignored = lit(&mut h, 0.6, 0.0, d().orm_mapped(half, 0.0));
    same(&ignored, &full, "strength 0 against no map");

    let sunlit_black_r = lit(&mut h, 0.0, 0.8, d().orm_mapped(none, 1.0));
    let sunlit = lit(&mut h, 0.0, 0.8, d());
    assert!(sunlit.centre_luma() > 30.0, "premise: the sun lights it");
    same(&sunlit_black_r, &sunlit, "R = 0 under the sun alone");
}

/// The emissive map multiplies the emissive colour, texel by texel: a
/// two-colour map lands its halves the right way round -- red on one side,
/// blue on the other -- on a quad that emits white and reflects nothing.
#[test]
fn the_emissive_map_multiplies_the_emissive_colour() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let map = h.two_colour_texture([255, 0, 0, 255], [0, 0, 255, 255]);
    let pixels = h.render(&Scene {
        draws: vec![Draw::new(q, Vec3::ZERO)
            .colour(Vec3::ZERO)
            .emissive(Vec3::ONE)
            .emissive_mapped(map)],
        light: Light {
            color: Vec3::ZERO,
            ambient: Vec3::ZERO,
            ..Light::default()
        },
        camera_pos: Vec3::new(0.0, 6.0, 0.01),
        ..Scene::default()
    });
    let left = pixels.at(pixels.width / 4, pixels.height / 2);
    let right = pixels.at(pixels.width * 3 / 4, pixels.height / 2);
    assert!(
        left[0] > left[2] + 40 && right[2] > right[0] + 40,
        "red on one side and blue on the other: {left:?} {right:?}"
    );
}
