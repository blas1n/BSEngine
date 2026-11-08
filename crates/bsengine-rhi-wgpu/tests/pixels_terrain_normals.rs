//! Terrain layers' normal maps.
//!
//! Terrain has no vertex tangents: its layers are mapped by world position, so
//! the shader takes world +x as the tangent and world +z as the bitangent.
//! These tests light a flat terrain chunk from one side and read whether a
//! layer's normal map turns the surface toward that side or away from it --
//! which is the only way to see *which* way the frame points, and so the only
//! way to catch a tangent or a bitangent pointing backwards. A normal map
//! that merely changed the shading would pass a "something changed" test
//! with either sign.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, ToneMap, ToneMappingMode};
use common::{Harness, Light, Scene};
use glam::{Mat4, Vec3};

/// A one-texel normal map holding the tangent-space normal `n`, linear --
/// as the terrain uploads a layer's normal map (`TextureCache::upload_data`).
fn normal_map(h: &mut Harness, n: Vec3) -> u64 {
    let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    let n = n.normalize();
    h.texture_with(
        bsengine_core::TextureImportSettings {
            srgb: false,
            mipmaps: false,
            ..Default::default()
        },
        1,
        1,
        &[enc(n.x), enc(n.y), enc(n.z), 255],
    )
}

/// The setup every test here renders: a grey chunk -- the unit cube scaled
/// flat, top face at y = 0.5 -- seen from straight above, lit only by a sun
/// from `towards_light` and 45 degrees up, with nothing between the shading
/// and the pixel (no ambient, bloom, AO or tone curve).
struct Terrain {
    cube: u64,
    layers: [u64; 4],
}

impl Terrain {
    fn new(h: &mut Harness) -> Self {
        let grey = h.two_colour_texture([150, 150, 150, 255], [150, 150, 150, 255]);
        Self {
            cube: h.cube(),
            layers: [grey; 4],
        }
    }

    fn render(
        &self,
        h: &mut Harness,
        towards_light: Vec3,
        weight: u64,
        normals: [Option<u64>; 4],
    ) -> f32 {
        let sun = Light {
            direction: -(towards_light + Vec3::Y).normalize(),
            ambient: Vec3::ZERO,
            ..Light::default()
        };
        self.render_lit(h, sun, weight, normals)
    }

    fn render_lit(
        &self,
        h: &mut Harness,
        light: Light,
        weight: u64,
        normals: [Option<u64>; 4],
    ) -> f32 {
        let pixels = h.render(&Scene {
            terrain: vec![(
                self.cube,
                Mat4::from_scale(Vec3::new(20.0, 1.0, 20.0)),
                self.layers,
                weight,
                normals,
            )],
            camera_pos: Vec3::new(0.0, 6.5, 0.01),
            look_at: Vec3::new(0.0, 0.5, 0.0),
            light,
            bloom: Some(Bloom::default().disabled()),
            ssao: Some(AmbientOcclusion::default().disabled()),
            tone_map: Some(ToneMap::new(ToneMappingMode::None)),
            ..Scene::default()
        });
        pixels.centre_luma()
    }
}

/// A weight texture putting all of the chunk on one layer, or split.
fn weight(h: &mut Harness, rgba: [u8; 4]) -> u64 {
    h.two_colour_texture(rgba, rgba)
}

/// A normal map leaning toward +u is a surface leaning toward world +x: lit
/// from +x it brightens, lit from -x it darkens. The tangent's sign is the
/// whole of what this checks; a frame built on -x reads the other way round.
#[test]
fn a_layer_normal_map_leaning_toward_u_turns_the_terrain_toward_x() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    let plus_u = normal_map(&mut h, Vec3::new(0.5, 0.0, 1.0));

    let flat_lit_from_x = t.render(&mut h, Vec3::X, layer0, [None; 4]);
    let mapped_lit_from_x = t.render(&mut h, Vec3::X, layer0, [Some(plus_u), None, None, None]);
    let flat_lit_from_minus_x = t.render(&mut h, -Vec3::X, layer0, [None; 4]);
    let mapped_lit_from_minus_x =
        t.render(&mut h, -Vec3::X, layer0, [Some(plus_u), None, None, None]);

    assert!(
        flat_lit_from_x > 20.0,
        "premise: the sun lights the flat chunk at all: {flat_lit_from_x}"
    );
    assert!(
        mapped_lit_from_x > flat_lit_from_x + 5.0,
        "leaning toward +x, toward the light: brighter ({flat_lit_from_x} -> {mapped_lit_from_x})"
    );
    assert!(
        mapped_lit_from_minus_x < flat_lit_from_minus_x - 5.0,
        "leaning toward +x, away from the light: darker \
         ({flat_lit_from_minus_x} -> {mapped_lit_from_minus_x})"
    );
}

/// The bitangent, the same way: leaning toward +v is leaning toward world +z,
/// the direction v grows in. The handedness is what this checks -- `cross(n,
/// t)` instead of `cross(t, n)` points the bitangent at -z and fails it --
/// and it is the half of the frame the +u test cannot see.
#[test]
fn a_layer_normal_map_leaning_toward_v_turns_the_terrain_toward_z() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    let plus_v = normal_map(&mut h, Vec3::new(0.0, 0.5, 1.0));

    let flat = t.render(&mut h, Vec3::Z, layer0, [None; 4]);
    let mapped = t.render(&mut h, Vec3::Z, layer0, [Some(plus_v), None, None, None]);
    let flat_away = t.render(&mut h, -Vec3::Z, layer0, [None; 4]);
    let mapped_away = t.render(&mut h, -Vec3::Z, layer0, [Some(plus_v), None, None, None]);

    assert!(flat > 20.0, "premise: the sun lights the chunk: {flat}");
    assert!(
        mapped > flat + 5.0,
        "leaning toward +z, toward the light: brighter ({flat} -> {mapped})"
    );
    assert!(
        mapped_away < flat_away - 5.0,
        "leaning toward +z, away from the light: darker ({flat_away} -> {mapped_away})"
    );
}

/// Each layer's map is its own layer's, and they blend by the splat weights:
/// a map on a layer that carries no weight changes nothing -- to the bit --
/// and a map on a layer carrying half the weight leans the surface part of
/// the way. A shader that applied layer 0's map everywhere, or ignored the
/// weights, passes the tests above and fails this one.
#[test]
fn layer_normal_maps_blend_by_the_splat_weights() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let all_layer0 = weight(&mut h, [255, 0, 0, 0]);
    let all_layer1 = weight(&mut h, [0, 255, 0, 0]);
    let half_each = weight(&mut h, [128, 128, 0, 0]);
    let plus_u = normal_map(&mut h, Vec3::new(0.5, 0.0, 1.0));
    let on_layer1 = [None, Some(plus_u), None, None];

    let flat = t.render(&mut h, Vec3::X, all_layer0, [None; 4]);
    let weightless = t.render(&mut h, Vec3::X, all_layer0, on_layer1);
    let half = t.render(&mut h, Vec3::X, half_each, on_layer1);
    let full = t.render(&mut h, Vec3::X, all_layer1, on_layer1);

    assert_eq!(
        weightless, flat,
        "a normal map on a layer with no weight here leaves the surface exactly flat"
    );
    assert!(
        full > flat + 5.0,
        "premise: layer 1's map, with all the weight, leans the surface ({flat} -> {full})"
    );
    assert!(
        half > flat + 2.0 && half < full - 2.0,
        "half the weight leans it part of the way: {flat} < {half} < {full}"
    );
}

/// A normal map that is named but not on the GPU -- still loading, or failed
/// -- is drawn as no map, not as the empty array layer it has: that layer is
/// zeros, which decode to a normal pointing down and into the corner. The
/// flag that says a layer has a map follows whether its texture is there, not
/// whether an id was given.
#[test]
fn a_normal_map_not_on_the_gpu_is_drawn_as_none() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    let plus_u = normal_map(&mut h, Vec3::new(0.5, 0.0, 1.0));
    let never_loaded = 999_999;

    let none = t.render(&mut h, Vec3::X, layer0, [None; 4]);
    let missing = t.render(
        &mut h,
        Vec3::X,
        layer0,
        [Some(never_loaded), None, None, None],
    );
    let mapped = t.render(&mut h, Vec3::X, layer0, [Some(plus_u), None, None, None]);

    assert!(
        mapped > none + 5.0,
        "premise: a map that is there turns the surface ({none} -> {mapped})"
    );
    assert_eq!(missing, none, "a map that is not there leaves it flat");
}

/// A layer with no normal map shades exactly flat -- not almost. The stand-in
/// it binds has to decode to exactly +z, and an eight-bit (128, 128, 255)
/// texel decodes to 0.0039 in x and y: a fifth of a degree, nothing under
/// the 45-degree sun above, but under a sun three degrees off the horizon it
/// is several percent of the light, brighter from one side than the other.
/// A flat surface lit the same from +x and from -x reads the same; that is
/// what this holds the no-map terrain to.
#[test]
fn a_layer_without_a_normal_map_is_exactly_flat() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    let grazing = |from: Vec3| {
        let up = 3.0f32.to_radians();
        Light {
            direction: -(from * up.cos() + Vec3::Y * up.sin()),
            // Bright, so that the few percent of light a grazing sun leaves
            // is still well above the eight-bit floor of the output.
            color: Vec3::splat(15.0),
            ambient: Vec3::ZERO,
            ..Light::default()
        }
    };

    let from_x = t.render_lit(&mut h, grazing(Vec3::X), layer0, [None; 4]);
    let from_minus_x = t.render_lit(&mut h, grazing(-Vec3::X), layer0, [None; 4]);

    assert!(
        from_x > 20.0,
        "premise: the grazing sun lights the chunk measurably: {from_x}"
    );
    assert!(
        (from_x - from_minus_x).abs() < 1.0,
        "lit the same from either side, a flat surface reads the same: \
         {from_x} from +x, {from_minus_x} from -x"
    );
}
