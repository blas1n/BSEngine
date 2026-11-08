//! Terrain layers' ORM maps: occlusion, roughness and metallic in R, G and B,
//! as glTF packs them and a material's ORM map is read.
//!
//! A layer with a map takes its surface from the map; one without keeps the
//! terrain's own (rough, not metallic, no occlusion). Each channel is checked
//! on the side only it can move -- a highlight for roughness, the diffuse
//! light for metallic, the ambient light for occlusion -- and occlusion also
//! on the side it must *not* move, the direct light, which a shader that
//! darkened everything would fail.

mod common;

use bsengine_core::{AmbientOcclusion, Bloom, TextureImportSettings, ToneMap, ToneMappingMode};
use bsengine_rhi_wgpu::TerrainDrawCall;
use common::{Harness, Light, Scene};
use glam::{Mat4, Vec3};

/// A one-texel ORM map, linear -- as the terrain uploads one
/// (`TextureCache::upload_data`).
fn orm_map(h: &mut Harness, occlusion: u8, roughness: u8, metallic: u8) -> u64 {
    h.texture_with(
        TextureImportSettings {
            srgb: false,
            mipmaps: false,
            ..Default::default()
        },
        1,
        1,
        &[occlusion, roughness, metallic, 255],
    )
}

/// A weight texture putting all of the chunk on one layer, or split.
fn weight(h: &mut Harness, rgba: [u8; 4]) -> u64 {
    h.two_colour_texture(rgba, rgba)
}

/// A grey chunk -- the unit cube scaled flat, top face at y = 0.5 -- seen
/// from straight above, with nothing between the shading and the pixel (no
/// bloom, AO or tone curve).
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

    fn centre(&self, h: &mut Harness, light: Light, weight: u64, orms: [Option<u64>; 4]) -> f32 {
        self.centre_with(h, light, weight, [None; 4], orms)
    }

    fn centre_with(
        &self,
        h: &mut Harness,
        light: Light,
        weight: u64,
        normals: [Option<u64>; 4],
        orms: [Option<u64>; 4],
    ) -> f32 {
        let chunk = TerrainDrawCall::new(
            self.cube,
            Mat4::from_scale(Vec3::new(20.0, 1.0, 20.0)),
            self.layers,
            weight,
        )
        .with_normals(normals)
        .with_orms(orms);
        h.render(&Scene {
            terrain: vec![chunk],
            camera_pos: Vec3::new(0.0, 6.5, 0.01),
            look_at: Vec3::new(0.0, 0.5, 0.0),
            light,
            bloom: Some(Bloom::default().disabled()),
            ssao: Some(AmbientOcclusion::default().disabled()),
            tone_map: Some(ToneMap::new(ToneMappingMode::None)),
            ..Scene::default()
        })
        .centre_luma()
    }
}

/// The sun straight overhead, behind the camera: its mirror reflection off the
/// flat chunk comes straight back up into the centre pixel.
fn overhead_sun() -> Light {
    Light {
        direction: -Vec3::Y,
        ambient: Vec3::ZERO,
        ..Light::default()
    }
}

/// The sun from the side, 45 degrees up: no mirror reflection reaches the
/// camera above, so what the centre shows is the diffuse light.
fn side_sun() -> Light {
    Light {
        direction: -(Vec3::X + Vec3::Y).normalize(),
        ambient: Vec3::ZERO,
        ..Light::default()
    }
}

/// Only ambient light, which is all the indirect light a terrain gets.
fn ambient_only() -> Light {
    Light {
        color: Vec3::ZERO,
        ambient: Vec3::splat(0.8),
        ..Light::default()
    }
}

/// G is roughness: a smooth layer gathers the overhead sun into a bright
/// highlight, a rough one spreads it. Against the map-less layer (the
/// terrain's own 0.9) as well, so a map read from the wrong channel -- R or
/// B are 255 and 0 here -- is seen as a surface that did not change.
#[test]
fn the_green_channel_is_roughness() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    let smooth = orm_map(&mut h, 255, 40, 0);
    let rough = orm_map(&mut h, 255, 255, 0);

    let none = t.centre(&mut h, overhead_sun(), layer0, [None; 4]);
    let glossy = t.centre(
        &mut h,
        overhead_sun(),
        layer0,
        [Some(smooth), None, None, None],
    );
    let matte = t.centre(
        &mut h,
        overhead_sun(),
        layer0,
        [Some(rough), None, None, None],
    );

    assert!(none > 20.0, "premise: the sun lights the chunk: {none}");
    assert!(
        glossy > none + 30.0,
        "a smooth layer shows the sun's highlight ({none} -> {glossy})"
    );
    assert!(
        matte < none,
        "a fully rough layer is rougher than the terrain's own 0.9 ({none} -> {matte})"
    );
}

/// B is metallic: a metal has no diffuse light, so lit from the side -- no
/// highlight in view -- a metallic layer goes dark where the plain one is lit.
#[test]
fn the_blue_channel_is_metallic() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    // The terrain's own roughness, so only metallic differs from no map.
    let metal = orm_map(&mut h, 255, 230, 255);
    let dielectric = orm_map(&mut h, 255, 230, 0);

    let plain = t.centre(
        &mut h,
        side_sun(),
        layer0,
        [Some(dielectric), None, None, None],
    );
    let metallic = t.centre(&mut h, side_sun(), layer0, [Some(metal), None, None, None]);

    assert!(
        plain > 20.0,
        "premise: the side sun lights the chunk: {plain}"
    );
    // Not to black: a rough metal's wide specular lobe still sends some of
    // the side sun up to the camera (measured: about 0.56 of the plain one).
    assert!(
        metallic < plain * 0.75,
        "a metal loses its diffuse light ({plain} -> {metallic})"
    );
}

/// R is occlusion, and it darkens the indirect light only: under ambient
/// light alone a fully occluded layer is black, and under the sun alone it is
/// exactly what it is without a map -- the direct light's own shadows say
/// what blocks it.
#[test]
fn the_red_channel_occludes_the_ambient_light_only() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    // The terrain's own roughness and metallic, so only occlusion differs.
    let occluded = orm_map(&mut h, 0, 230, 0);
    let open = orm_map(&mut h, 255, 230, 0);

    let ambient_open = t.centre(
        &mut h,
        ambient_only(),
        layer0,
        [Some(open), None, None, None],
    );
    let ambient_occluded = t.centre(
        &mut h,
        ambient_only(),
        layer0,
        [Some(occluded), None, None, None],
    );
    let sun_open = t.centre(&mut h, side_sun(), layer0, [Some(open), None, None, None]);
    let sun_occluded = t.centre(
        &mut h,
        side_sun(),
        layer0,
        [Some(occluded), None, None, None],
    );

    assert!(
        ambient_open > 20.0,
        "premise: the ambient light alone lights the chunk: {ambient_open}"
    );
    assert!(
        ambient_occluded < 1.0,
        "an occluded layer gets no ambient light: {ambient_occluded}"
    );
    assert!(
        sun_open > 20.0,
        "premise: the sun alone lights the chunk: {sun_open}"
    );
    assert_eq!(
        sun_occluded, sun_open,
        "occlusion leaves the direct light alone"
    );
}

/// Each layer's map is its own layer's, blended by the splat weights: a map
/// on a layer that carries no weight changes nothing -- to the bit -- and on
/// a layer carrying half the weight it moves the surface part of the way. A
/// shader that applied layer 0's map everywhere, or ignored the weights,
/// passes the tests above and fails this one.
#[test]
fn layer_orm_maps_blend_by_the_splat_weights() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let all_layer0 = weight(&mut h, [255, 0, 0, 0]);
    let all_layer1 = weight(&mut h, [0, 255, 0, 0]);
    let half_each = weight(&mut h, [128, 128, 0, 0]);
    let smooth = orm_map(&mut h, 255, 40, 0);
    let on_layer1 = [None, Some(smooth), None, None];

    let none = t.centre(&mut h, overhead_sun(), all_layer0, [None; 4]);
    let weightless = t.centre(&mut h, overhead_sun(), all_layer0, on_layer1);
    let half = t.centre(&mut h, overhead_sun(), half_each, on_layer1);
    let full = t.centre(&mut h, overhead_sun(), all_layer1, on_layer1);

    assert_eq!(
        weightless, none,
        "an ORM map on a layer with no weight here changes nothing"
    );
    assert!(
        full > none + 30.0,
        "premise: layer 1's map, with all the weight, shows ({none} -> {full})"
    );
    assert!(
        half > none + 2.0 && half < full - 2.0,
        "half the weight moves it part of the way: {none} < {half} < {full}"
    );
}

/// The normal maps and the ORM maps are separate arrays with separate flags:
/// a layer with an ORM map and no normal map is shaded flat, not with the ORM
/// map read as a normal (nor the other way round). Both maps here are the
/// same texel -- one linear 1x1 -- so only the flags and the bindings keep
/// them apart. Drawn as a normal map first, in the same renderer, so a flag
/// buffer cached for "layer 0 has a normal map" cannot be handed to "layer 0
/// has an ORM map" either.
#[test]
fn an_orm_map_is_not_a_normal_map() {
    let mut h = Harness::new();
    let t = Terrain::new(&mut h);
    let layer0 = weight(&mut h, [255, 0, 0, 0]);
    // Read as a normal, (1.0, 0.9, 0.0) decodes to (1.0, 0.8, -1.0), below
    // the surface; read as ORM it is the terrain's own surface (roughness
    // 0.9, no metal, no occlusion).
    let orm = orm_map(&mut h, 255, 230, 0);

    let none = t.centre(&mut h, side_sun(), layer0, [None; 4]);
    let as_normal = t.centre_with(
        &mut h,
        side_sun(),
        layer0,
        [Some(orm), None, None, None],
        [None; 4],
    );
    let with_orm = t.centre(&mut h, side_sun(), layer0, [Some(orm), None, None, None]);

    // Read as a normal it points below the surface, and the chunk goes dark.
    assert!(
        (as_normal - none).abs() > 5.0,
        "premise: read as a normal map, the texel turns the surface ({none} -> {as_normal})"
    );
    assert!(
        none > 20.0,
        "premise: the side sun lights the chunk: {none}"
    );
    assert!(
        (with_orm - none).abs() < 1.5,
        "an ORM map of the terrain's own surface does not tilt it: {none} -> {with_orm}"
    );
}
