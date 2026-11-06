//! Lit custom shaders: a graph or a hand-written `surface` says what is there,
//! and the engine lights it.
//!
//! The strongest thing there is to say about one is that it *is* the standard
//! material where it changes nothing, so most of this file renders a scene
//! twice -- once with the standard material, once through a `LitOutput` with
//! nothing connected -- and asks for the same bytes, one rendering feature at a
//! time. A do-nothing implementation cannot pass that: every test also asserts
//! that the feature it is about changes the standard material's picture, and
//! that an unlit shader's picture stays out of it.

mod common;

use bsengine_core::{MotionBlur, ScreenSpaceGlobalIllumination, ScreenSpaceReflections};
use bsengine_shadergraph::{Edge, GraphNode, NodeKind, ShaderGraph};
use common::{Draw, Harness, Light, Pixels, PointLight, Scene};
use glam::{Mat4, Vec3};

/// `kind`s feeding the named ports of a `LitOutput`.
fn lit_graph(ports: &[(&str, NodeKind)]) -> ShaderGraph {
    let node = |id: u32, kind| GraphNode {
        id,
        kind,
        position: [0.0, 0.0],
    };
    let mut nodes = vec![node(0, NodeKind::LitOutput)];
    let mut edges = Vec::new();
    for (i, (port, kind)) in ports.iter().enumerate() {
        let id = 1 + i as u32;
        nodes.push(node(id, kind.clone()));
        edges.push(Edge {
            from: (id, "out".to_string()),
            to: (0, port.to_string()),
        });
    }
    ShaderGraph { nodes, edges }
}

/// Compiles `graph` under `key`, failing with the source if it is refused.
fn compile(h: &mut Harness, graph: &ShaderGraph, key: &str) -> String {
    let wgsl = bsengine_shadergraph::compile(graph).expect("the graph must compile");
    h.compile_shader(key, &wgsl)
        .unwrap_or_else(|e| panic!("the lit shader was refused: {e}\n{wgsl}"));
    key.to_string()
}

/// `LitOutput` with nothing connected.
fn as_material(h: &mut Harness, key: &str) -> String {
    compile(h, &lit_graph(&[]), key)
}

/// A 6 x 6 quad at y = 0, facing +y, white, `u` along +x: the normal map test's,
/// whose generated tangent is +x.
fn quad(h: &mut Harness) -> u64 {
    let corner = |x: f32, z: f32| bsengine_rhi_wgpu::Vertex {
        position: [x, 0.0, z],
        color: [1.0; 3],
        normal: [0.0, 1.0, 0.0],
        uv: [x / 6.0 + 0.5, z / 6.0 + 0.5],
        uv2: [0.5; 2],
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

fn map(h: &mut Harness, rgba: [u8; 4]) -> u64 {
    h.texture_with(linear(), 1, 1, &rgba)
}

/// A one-texel normal map for the tangent-space normal `n`.
fn normal_map(h: &mut Harness, n: Vec3) -> u64 {
    let enc = |c: f32| ((c * 0.5 + 0.5) * 255.0).round() as u8;
    let n = n.normalize();
    map(h, [enc(n.x), enc(n.y), enc(n.z), 255])
}

/// Everything a scene here is made of.
struct Stage {
    cube: u64,
    quad: u64,
}

impl Stage {
    fn new(h: &mut Harness) -> Self {
        Self {
            cube: h.cube(),
            quad: quad(h),
        }
    }

    /// Something standing on the quad: it casts a shadow across it, stands in
    /// its reflections and stays the standard material whatever the quad is.
    fn post(&self) -> Draw {
        Draw::new(self.cube, Vec3::new(0.0, 1.0, 0.0))
            .scaled(Vec3::new(1.0, 2.0, 1.0), Vec3::new(0.0, 1.0, 0.0))
            .colour(Vec3::new(0.2, 0.3, 0.9))
            .emissive(Vec3::new(1.5, 0.2, 0.2))
    }

    /// The quad, a sun with a shadow to cast and a point light, seen from above.
    fn scene(&self, subject: Draw) -> Scene {
        Scene {
            draws: vec![subject, self.post()],
            light: Light {
                direction: Vec3::new(-0.5, -0.8, -0.3).normalize(),
                ambient: Vec3::splat(0.1),
                points: vec![PointLight {
                    position: Vec3::new(-1.5, 1.0, 1.5),
                    color: Vec3::new(0.2, 1.0, 0.2),
                    intensity: 2.0,
                    range: 6.0,
                }],
                ..Light::default()
            },
            camera_pos: Vec3::new(0.0, 4.0, 5.0),
            look_at: Vec3::ZERO,
            ..Scene::default()
        }
    }
}

/// `frames` of `scene`, for what reads last frame: the last frame's picture.
fn settled(h: &mut Harness, scene: &Scene, frames: u32) -> Pixels {
    for _ in 1..frames {
        h.render(scene);
    }
    h.render(scene)
}

/// `make(None)` and `make(Some(lit shader))` must be the same picture, and
/// `make` has to be a picture a shader that lights nothing could not fake.
///
/// A harness each: what reads last frame -- reflections, TAA, GI -- would
/// otherwise start the second render from the first one's history.
fn assert_is_the_standard_material(
    what: &str,
    frames: u32,
    premise: impl Fn(&mut Harness, &Stage) -> (Pixels, &'static str),
    make: impl Fn(&mut Harness, &Stage, Option<&str>) -> Scene,
) {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let scene = make(&mut h, &stage, None);
    let standard = settled(&mut h, &scene, frames);
    let (without, says) = premise(&mut h, &stage);
    assert!(
        standard.differs_from(&without),
        "premise of {what}: {says}, or this measures nothing"
    );

    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let shader = as_material(&mut h, &format!("lit_{what}"));
    let scene = make(&mut h, &stage, Some(&shader));
    let lit = settled(&mut h, &scene, frames);

    assert_eq!(
        lit.max_channel_diff(&standard),
        0,
        "{what}: a LitOutput with nothing connected must be the standard material, \
         but {} pixels differ",
        lit.differing_pixels(&standard)
    );

    // And the comparison can tell: an unlit shader on the same draw is not this
    // picture.
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let unlit = h.constant_colour_shader([0.4, 0.4, 0.4], &format!("unlit_{what}"));
    let scene = make(&mut h, &stage, Some(&unlit));
    assert!(
        settled(&mut h, &scene, frames).differs_from(&standard),
        "{what}: an unlit shader drew the standard material's picture, so the \
         comparison above cannot tell the two apart"
    );
}

fn shaded(draw: Draw, shader: Option<&str>) -> Draw {
    match shader {
        Some(s) => draw.shader(s),
        None => draw,
    }
}

/// The quad with every map a material has, none of them neutral.
fn fully_mapped(h: &mut Harness, stage: &Stage) -> Draw {
    let albedo = h.two_colour_texture([230, 120, 40, 255], [60, 150, 220, 255]);
    let tilted = normal_map(h, Vec3::new(0.4, 0.2, 0.9));
    let orm = map(h, [120, 150, 110, 255]);
    let glow = h.two_colour_texture([40, 0, 0, 255], [0, 0, 60, 255]);
    Draw::new(stage.quad, Vec3::ZERO)
        .textured(albedo)
        .normal_mapped(tilted, 0.8)
        .orm_mapped(orm, 0.7)
        .emissive_mapped(glow)
        .emissive(Vec3::splat(0.5))
        .metallic(0.6)
        .roughness(0.45)
}

#[test]
fn with_nothing_connected_it_is_the_standard_material() {
    assert_is_the_standard_material(
        "maps_and_lights",
        1,
        |h, stage| {
            // The same material in the dark: the lights had a say.
            let mut scene = stage.scene(fully_mapped(h, stage));
            scene.light.color = Vec3::ZERO;
            scene.light.points.clear();
            (h.render(&scene), "the sun and the point light light it")
        },
        |h, stage, shader| stage.scene(shaded(fully_mapped(h, stage), shader)),
    );
}

#[test]
fn it_takes_the_shadow_the_standard_material_takes() {
    let shadowed = |h: &mut Harness, stage: &Stage, shader: Option<&str>| {
        let _ = h;
        stage.scene(shaded(Draw::new(stage.quad, Vec3::ZERO), shader))
    };
    assert_is_the_standard_material(
        "shadow",
        1,
        |h, stage| {
            // Without the post, whose own pixels change as well: that this
            // leaves a shadow is what the count below is for.
            let mut scene = stage.scene(Draw::new(stage.quad, Vec3::ZERO));
            scene.draws.truncate(1);
            (h.render(&scene), "the post is in the picture")
        },
        shadowed,
    );

    // The hard side, on the lit shader itself: where the sun's shadow lands
    // the quad is darker than the same point with no post.
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let shader = as_material(&mut h, "lit_shadow_hard");
    let scene = shadowed(&mut h, &stage, Some(&shader));
    let with_post = h.render(&scene);
    let mut bare = shadowed(&mut h, &stage, Some(&shader));
    bare.draws.truncate(1);
    let without_post = h.render(&bare);
    let darkened = (0..with_post.height)
        .flat_map(|y| (0..with_post.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let (a, b) = (with_post.at(x, y), without_post.at(x, y));
            a[0] as i32 + a[1] as i32 + a[2] as i32 + 30 < b[0] as i32 + b[1] as i32 + b[2] as i32
        })
        .count();
    assert!(
        darkened > 100,
        "no shadow fell on the lit quad: {darkened} darkened pixels"
    );
}

#[test]
fn it_reads_the_sky_the_standard_material_reads() {
    let scene = |stage: &Stage, shader: Option<&str>| {
        let mut s = stage.scene(shaded(
            Draw::new(stage.quad, Vec3::ZERO)
                .metallic(0.8)
                .roughness(0.3),
            shader,
        ));
        s.light.color = Vec3::ZERO;
        s.light.points.clear();
        s
    };
    assert_is_the_standard_material(
        "sky",
        1,
        |h, stage| {
            h.clear_test_skybox();
            (h.render(&scene(stage, None)), "the sky lights it")
        },
        |h, stage, shader| {
            h.set_test_skybox([200, 170, 90, 255]);
            scene(stage, shader)
        },
    );
}

#[test]
fn it_reflects_what_the_standard_material_reflects() {
    let mirror = |stage: &Stage, shader: Option<&str>, ssr: Option<ScreenSpaceReflections>| {
        let mut s = stage.scene(shaded(
            Draw::new(stage.quad, Vec3::ZERO)
                .roughness(0.02)
                .colour(Vec3::splat(0.02)),
            shader,
        ));
        s.camera_pos = Vec3::new(0.0, 1.2, 6.0);
        s.look_at = Vec3::new(0.0, 0.6, 0.0);
        s.ssr = ssr;
        s
    };
    assert_is_the_standard_material(
        "reflection",
        3,
        |h, stage| {
            (
                settled(h, &mirror(stage, None, None), 3),
                "the reflections put something on the floor",
            )
        },
        |_, stage, shader| mirror(stage, shader, Some(ScreenSpaceReflections::default())),
    );
}

#[test]
fn the_reflections_start_from_the_ports_not_the_material() {
    // The prepass's own pipeline, the one thing the opaque pass does not show:
    // the draw's material is too rough to reflect at all, so only a roughness
    // port that reached `fs_ssr_normal` makes it a mirror.
    let floor = |stage: &Stage, roughness: f32, shader: Option<&str>| {
        let mut s = stage.scene(shaded(
            Draw::new(stage.quad, Vec3::ZERO)
                .roughness(roughness)
                .colour(Vec3::splat(0.02)),
            shader,
        ));
        s.camera_pos = Vec3::new(0.0, 1.2, 6.0);
        s.look_at = Vec3::new(0.0, 0.6, 0.0);
        s.ssr = Some(ScreenSpaceReflections::default());
        s
    };
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let mirror = settled(&mut h, &floor(&stage, 0.02, None), 3);

    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let rough = settled(&mut h, &floor(&stage, 1.0, None), 3);
    assert!(
        mirror.differs_from(&rough),
        "premise: a rough floor does not reflect"
    );

    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let shader = compile(
        &mut h,
        &lit_graph(&[("roughness", NodeKind::Constant(0.02))]),
        "lit_port_roughness",
    );
    let lit = settled(&mut h, &floor(&stage, 1.0, Some(&shader)), 3);
    assert_eq!(
        lit.max_channel_diff(&mirror),
        0,
        "a roughness port must be the roughness the reflections trace from"
    );
}

#[test]
fn it_receives_screen_space_gi_as_the_standard_material_does() {
    let scene = |stage: &Stage, shader: Option<&str>, on: bool| {
        let mut s = stage.scene(shaded(Draw::new(stage.quad, Vec3::ZERO), shader));
        s.ssgi = on.then(ScreenSpaceGlobalIllumination::default);
        s
    };
    assert_is_the_standard_material(
        "ssgi",
        3,
        |h, stage| {
            (
                settled(h, &scene(stage, None, false), 3),
                "the GI changes the picture",
            )
        },
        |_, stage, shader| scene(stage, shader, true),
    );
}

#[test]
fn it_resolves_under_msaa_as_the_standard_material_does() {
    let scene = |stage: &Stage, shader: Option<&str>, msaa: u32| {
        let mut s = stage.scene(shaded(
            Draw::new(stage.cube, Vec3::new(0.0, 0.5, 2.0)).rotated_z(0.4),
            shader,
        ));
        s.msaa = msaa;
        s
    };
    assert_is_the_standard_material(
        "msaa",
        1,
        |h, stage| {
            (
                h.render(&scene(stage, None, 1)),
                "the samples soften an edge",
            )
        },
        |_, stage, shader| scene(stage, shader, 4),
    );
}

#[test]
fn a_decal_lands_on_it_as_it_lands_on_the_standard_material() {
    let scene = |stage: &Stage, tex: Option<u64>, shader: Option<&str>| {
        let mut s = stage.scene(shaded(Draw::new(stage.quad, Vec3::ZERO), shader));
        s.decals = tex
            .map(|texture| bsengine_rhi_wgpu::decals::DecalDraw {
                model: Mat4::from_translation(Vec3::new(1.5, 0.0, 1.5))
                    * Mat4::from_scale(Vec3::new(2.0, 2.0, 2.0)),
                opacity: 1.0,
                normal_fade: 0.0,
                texture: Some(texture),
                normal_texture: None,
            })
            .into_iter()
            .collect();
        s
    };
    assert_is_the_standard_material(
        "decal",
        1,
        |h, stage| {
            (
                h.render(&scene(stage, None, None)),
                "the decal is in the picture",
            )
        },
        |h, stage, shader| {
            let red = h.two_colour_texture([255, 0, 0, 255], [255, 0, 0, 255]);
            scene(stage, Some(red), shader)
        },
    );
}

#[test]
fn a_transparent_one_blends_as_the_standard_material_does() {
    // Over the post, so what is behind it is not one colour: an opaque draw,
    // or a pipeline blending some other way, shows on it.
    let scene = |stage: &Stage, shader: Option<&str>, opacity: f32| {
        let pane = shaded(
            Draw::new(stage.cube, Vec3::new(0.0, 1.0, 2.0))
                .scaled(Vec3::new(3.0, 3.0, 0.2), Vec3::new(0.0, 1.0, 2.0))
                .colour(Vec3::new(0.9, 0.9, 0.2))
                .opacity(opacity),
            shader,
        );
        let mut s = stage.scene(Draw::new(stage.quad, Vec3::ZERO));
        s.draws.push(pane);
        s
    };
    assert_is_the_standard_material(
        "transparent",
        1,
        |h, stage| {
            (
                h.render(&scene(stage, None, 1.0)),
                "what is behind the pane shows through",
            )
        },
        |_, stage, shader| scene(stage, shader, 0.5),
    );
}

/// The pipeline has to be the multisampled one here as well: a draw's sample
/// count not matching its pass is not a wrong picture but a validation error.
#[test]
fn a_transparent_one_blends_under_msaa_too() {
    let scene = |stage: &Stage, shader: Option<&str>, msaa: u32| {
        let pane = shaded(
            Draw::new(stage.cube, Vec3::new(0.0, 1.0, 2.0))
                .scaled(Vec3::new(3.0, 3.0, 0.2), Vec3::new(0.0, 1.0, 2.0))
                .rotated_z(0.3)
                .opacity(0.5),
            shader,
        );
        let mut s = stage.scene(Draw::new(stage.quad, Vec3::ZERO));
        s.draws.push(pane);
        s.msaa = msaa;
        s
    };
    assert_is_the_standard_material(
        "transparent_msaa",
        1,
        |h, stage| {
            (
                h.render(&scene(stage, None, 1)),
                "the samples soften the pane's edge",
            )
        },
        |_, stage, shader| scene(stage, shader, 4),
    );
}

/// The mirror floor the two tests below look at, through a lit shader.
fn lit_mirror(port: &str, value: NodeKind, key: &str) -> Pixels {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let shader = compile(&mut h, &lit_graph(&[(port, value)]), key);
    let mut s = stage.scene(
        Draw::new(stage.quad, Vec3::ZERO)
            .roughness(0.02)
            .colour(Vec3::splat(0.02))
            .shader(&shader),
    );
    s.camera_pos = Vec3::new(0.0, 1.2, 6.0);
    s.look_at = Vec3::new(0.0, 0.6, 0.0);
    s.ssr = Some(ScreenSpaceReflections::default());
    settled(&mut h, &s, 3)
}

/// A graph's normal is whatever its nodes left it, and the reflections' prepass
/// stores what it is given. The trace takes a pixel whose stored normal is
/// shorter than a half for one with no surface at all, so unnormalised, a short
/// normal loses its reflections. (A long one would not show: the trace
/// normalises what it reads.)
#[test]
fn a_normal_port_of_any_length_is_the_same_normal() {
    let unit = lit_mirror(
        "normal",
        NodeKind::ConstantVec3([0.0, 1.0, 0.0]),
        "normal_unit",
    );
    for (length, key) in [(0.3, "normal_short"), (3.0, "normal_long")] {
        let other = lit_mirror("normal", NodeKind::ConstantVec3([0.0, length, 0.0]), key);
        assert_eq!(
            other.max_channel_diff(&unit),
            0,
            "a normal of length {length} reflected differently"
        );
    }
}

/// The floor the shading keeps roughness at holds for the reflections too: they
/// fade with roughness, so a port's 0 unfloored would reflect more strongly
/// than the 0.04 the surface is shaded with.
#[test]
fn a_roughness_port_of_zero_reflects_as_the_floor_does() {
    let zero = lit_mirror("roughness", NodeKind::Constant(0.0), "roughness_zero");
    let floor = lit_mirror("roughness", NodeKind::Constant(0.04), "roughness_floor");
    assert_eq!(zero.max_channel_diff(&floor), 0);
}

#[test]
fn it_is_lit_by_its_lightmap_as_the_standard_material_is() {
    let scene = |stage: &Stage, lightmap: Option<u64>, shader: Option<&str>| {
        let mut draw = Draw::new(stage.quad, Vec3::ZERO);
        if let Some(l) = lightmap {
            draw = draw.lightmapped(l, [1.0, 1.0, 0.0, 0.0]);
        }
        stage.scene(shaded(draw, shader))
    };
    assert_is_the_standard_material(
        "lightmap",
        1,
        |h, stage| {
            (
                h.render(&scene(stage, None, None)),
                "the lightmap lights it",
            )
        },
        |h, stage, shader| {
            let baked = map(h, [255, 60, 60, 255]);
            scene(stage, Some(baked), shader)
        },
    );
}

/// The velocity is what a lit shader has that an unlit one masks off, and it is
/// read directly: motion blur alone could hide a velocity of the wrong size.
#[test]
fn it_writes_the_velocity_the_standard_material_writes() {
    let moving = |stage: &Stage, shader: Option<&str>| {
        let mut s = stage.scene(shaded(
            Draw::new(stage.quad, Vec3::ZERO)
                .moved_from(Mat4::from_translation(Vec3::new(-0.3, 0.0, 0.0))),
            shader,
        ));
        s.motion_blur = Some(MotionBlur::default());
        s
    };
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let standard = h.render(&moving(&stage, None));
    let standard_velocity = h.velocity();
    assert!(
        standard_velocity
            .iter()
            .any(|v| v[0].abs() > 0.001 && v[0].abs() < 1.0),
        "premise: the moving quad has a velocity"
    );

    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let shader = as_material(&mut h, "lit_velocity");
    let lit = h.render(&moving(&stage, Some(&shader)));
    assert_eq!(
        h.velocity(),
        standard_velocity,
        "a lit shader must write the standard material's velocity"
    );
    assert_eq!(lit.max_channel_diff(&standard), 0);
}

/// What each port does, named by the material field that does the same: a draw
/// whose own material says something else, through a port saying `value`, must
/// look like the standard material saying `value` itself.
#[test]
fn each_port_means_what_the_material_field_means() {
    let base = |stage: &Stage| {
        Draw::new(stage.quad, Vec3::ZERO)
            .colour(Vec3::new(0.3, 0.6, 0.5))
            .metallic(0.1)
            .roughness(0.9)
    };
    type Change = fn(Draw) -> Draw;
    let cases: [(&str, NodeKind, Change); 4] = [
        ("base_color", NodeKind::ConstantVec3([0.8, 0.2, 0.1]), |d| {
            d.colour(Vec3::new(0.8, 0.2, 0.1))
        }),
        ("metallic", NodeKind::Constant(0.9), |d| d.metallic(0.9)),
        ("roughness", NodeKind::Constant(0.2), |d| d.roughness(0.2)),
        ("emissive", NodeKind::ConstantVec3([0.0, 0.3, 0.6]), |d| {
            d.emissive(Vec3::new(0.0, 0.3, 0.6))
        }),
    ];
    for (port, value, change) in cases {
        let mut h = Harness::new();
        let stage = Stage::new(&mut h);
        let unchanged = h.render(&stage.scene(base(&stage)));
        let standard = h.render(&stage.scene(change(base(&stage))));
        assert!(
            standard.differs_from(&unchanged),
            "premise: {port} shows in this picture"
        );
        let shader = compile(
            &mut h,
            &lit_graph(&[(port, value)]),
            &format!("port_{port}"),
        );
        let lit = h.render(&stage.scene(base(&stage).shader(&shader)));
        assert_eq!(
            lit.max_channel_diff(&standard),
            0,
            "the {port} port is not the material's {port}"
        );
    }
}

#[test]
fn the_occlusion_port_darkens_the_indirect_light_and_not_the_direct() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let orm = map(&mut h, [64, 255, 255, 255]);
    let scene = |draw: Draw, sun: Vec3, ambient: f32| {
        let mut s = stage.scene(draw);
        s.draws.truncate(1);
        s.light.points.clear();
        s.light.color = sun;
        s.light.ambient = Vec3::splat(ambient);
        s
    };
    let plain = || Draw::new(stage.quad, Vec3::ZERO);
    let shader = compile(
        &mut h,
        &lit_graph(&[("occlusion", NodeKind::Constant(64.0 / 255.0))]),
        "port_occlusion",
    );

    for (what, sun, ambient) in [("indirect", Vec3::ZERO, 0.8), ("direct", Vec3::ONE, 0.0)] {
        let unoccluded = h.render(&scene(plain(), sun, ambient));
        let standard = h.render(&scene(plain().orm_mapped(orm, 1.0), sun, ambient));
        let lit = h.render(&scene(plain().shader(&shader), sun, ambient));
        assert_eq!(
            lit.max_channel_diff(&standard),
            0,
            "{what}: the occlusion port is not the ORM map's occlusion"
        );
        assert_eq!(
            standard.differs_from(&unoccluded),
            what == "indirect",
            "premise: occlusion darkens the {what} light {}",
            if what == "indirect" { "" } else { "not at all" }
        );
    }
}

/// World space, as the `Normal` node hands it out: a port tilted toward +x is a
/// normal map tilted toward +u, on a quad whose +u is +x.
#[test]
fn the_normal_port_is_a_world_space_normal() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let world = Vec3::new(0.5, 0.7, 0.2).normalize();
    // Tangent space: x along +u (+x), z up the normal (+y), y along +v -- which
    // this quad's generated handedness puts along +z.
    let mapped = normal_map(&mut h, Vec3::new(world.x, world.z, world.y));
    let shader = compile(
        &mut h,
        &lit_graph(&[("normal", NodeKind::ConstantVec3(world.to_array()))]),
        "port_normal",
    );
    let flat = h.render(&stage.scene(Draw::new(stage.quad, Vec3::ZERO)));
    let standard =
        h.render(&stage.scene(Draw::new(stage.quad, Vec3::ZERO).normal_mapped(mapped, 1.0)));
    let lit = h.render(&stage.scene(Draw::new(stage.quad, Vec3::ZERO).shader(&shader)));
    assert!(standard.differs_from(&flat), "premise: the tilt shows");
    // Not 0: a one-texel map holds the normal in eight bits.
    assert!(
        lit.max_channel_diff(&standard) <= 3,
        "the normal port is not the world normal the map produces: off by {}",
        lit.max_channel_diff(&standard)
    );
}

/// What is unconnected follows the entity, which is what lets one graph serve
/// many materials: one shader, two entities' roughness.
#[test]
fn what_is_unconnected_follows_each_entitys_material() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let shader = compile(
        &mut h,
        &lit_graph(&[("emissive", NodeKind::ConstantVec3([0.0, 0.0, 0.2]))]),
        "follows_material",
    );
    for roughness in [0.15, 0.8] {
        let draw = || Draw::new(stage.quad, Vec3::ZERO).roughness(roughness);
        let standard = h.render(&stage.scene(draw().emissive(Vec3::new(0.0, 0.0, 0.2))));
        let lit = h.render(&stage.scene(draw().shader(&shader)));
        assert_eq!(
            lit.max_channel_diff(&standard),
            0,
            "at roughness {roughness}"
        );
    }
}

/// A graph's numbers are whatever a `Sin` or an `Add` left them.
#[test]
fn numbers_out_of_range_are_brought_into_it() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let cases = [
        (
            "base_color",
            NodeKind::ConstantVec3([2.0, -1.0, 0.5]),
            NodeKind::ConstantVec3([1.0, 0.0, 0.5]),
        ),
        ("metallic", NodeKind::Constant(3.0), NodeKind::Constant(1.0)),
        // Above, not below: below is the floor `shade` keeps it at either way.
        (
            "roughness",
            NodeKind::Constant(3.0),
            NodeKind::Constant(1.0),
        ),
        (
            "occlusion",
            NodeKind::Constant(4.0),
            NodeKind::Constant(1.0),
        ),
        (
            "emissive",
            NodeKind::ConstantVec3([-1.0, -1.0, 0.2]),
            NodeKind::ConstantVec3([0.0, 0.0, 0.2]),
        ),
    ];
    for (port, wild, tame) in cases {
        let wild_shader = compile(&mut h, &lit_graph(&[(port, wild)]), &format!("wild_{port}"));
        let tame_shader = compile(&mut h, &lit_graph(&[(port, tame)]), &format!("tame_{port}"));
        let draw = |s: &str| Draw::new(stage.quad, Vec3::ZERO).metallic(0.2).shader(s);
        let a = h.render(&stage.scene(draw(&wild_shader)));
        let b = h.render(&stage.scene(draw(&tame_shader)));
        assert_eq!(
            a.max_channel_diff(&b),
            0,
            "{port} was not brought into range"
        );
    }
}

/// The nodes that read the frame, which a lit graph reaches through the lit
/// shader's bindings and not the unlit preamble's.
#[test]
fn a_lit_graph_reads_what_an_unlit_graph_reads() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let tex = h.two_colour_texture([255, 40, 0, 255], [0, 40, 255, 255]);
    let textured = {
        let mut g = lit_graph(&[("base_color", NodeKind::TextureSample)]);
        g.nodes.push(GraphNode {
            id: 9,
            kind: NodeKind::Uv,
            position: [0.0, 0.0],
        });
        g.edges.push(Edge {
            from: (9, "out".to_string()),
            to: (1, "uv".to_string()),
        });
        compile(&mut h, &g, "reads_texture")
    };
    let draw = || Draw::new(stage.quad, Vec3::ZERO).textured(tex);
    let standard = h.render(&stage.scene(draw()));
    let lit = h.render(&stage.scene(draw().shader(&textured)));
    assert_eq!(lit.max_channel_diff(&standard), 0, "TextureSample and Uv");

    let bumped = normal_map(&mut h, Vec3::new(0.5, 0.0, 0.8));
    let normal = compile(
        &mut h,
        &lit_graph(&[("normal", NodeKind::Normal)]),
        "reads_normal",
    );
    let draw = || Draw::new(stage.quad, Vec3::ZERO).normal_mapped(bumped, 1.0);
    let standard = h.render(&stage.scene(draw()));
    let lit = h.render(&stage.scene(draw().shader(&normal)));
    assert_eq!(lit.max_channel_diff(&standard), 0, "Normal");

    let timed = {
        let mut g = lit_graph(&[("metallic", NodeKind::Sin)]);
        g.nodes.push(GraphNode {
            id: 9,
            kind: NodeKind::Time,
            position: [0.0, 0.0],
        });
        g.edges.push(Edge {
            from: (9, "out".to_string()),
            to: (1, "x".to_string()),
        });
        compile(&mut h, &g, "reads_time")
    };
    let scene = stage.scene(Draw::new(stage.quad, Vec3::ZERO).shader(&timed));
    let early = h.render_at_time(&scene, 0.0);
    let later = h.render_at_time(&scene, 1.5);
    assert!(later.differs_from(&early), "Time");
}

const HAND_WRITTEN: &str = "// bsengine: lit
fn surface(in: VertOut) -> Surface {
    var s = material_surface(in);
    s.albedo = vec3<f32>(0.8, 0.2, 0.1);
    s.roughness = 0.25;
    return s;
}
";

#[test]
fn a_hand_written_surface_is_lit_like_a_graph() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    h.compile_shader("hand_written", HAND_WRITTEN)
        .expect("a hand-written surface must compile");
    let lit = h.render(&stage.scene(Draw::new(stage.quad, Vec3::ZERO).shader("hand_written")));
    let standard = h.render(
        &stage.scene(
            Draw::new(stage.quad, Vec3::ZERO)
                .colour(Vec3::new(0.8, 0.2, 0.1))
                .roughness(0.25),
        ),
    );
    assert_eq!(lit.max_channel_diff(&standard), 0);
}

#[test]
fn a_mistake_is_reported_at_the_authors_own_line() {
    // Line 3 of the author's file. The renderer's half follows the author's
    // text rather than preceding it, or this would say line nine hundred.
    let broken = "// bsengine: lit\nfn surface(in: VertOut) -> Surface {\n    return not_a_function(in);\n}\n";
    let mut h = Harness::new();
    let e = h
        .compile_shader("broken_surface", broken)
        .expect_err("a call to nothing must be refused");
    assert!(
        e.contains("not_a_function") && e.contains(":3:"),
        "the message must point at the author's line 3: {e}"
    );
}

/// The directive with no `surface` is refused rather than drawn as something.
#[test]
fn a_lit_shader_without_a_surface_is_refused() {
    let mut h = Harness::new();
    assert!(h
        .compile_shader("no_surface", "// bsengine: lit\nconst x: f32 = 1.0;\n")
        .is_err());
}

/// The reflections' prepass has no lighting group to bind (`LitCustomPipelines`):
/// refused while compiling, not in the middle of a frame, and the shader that
/// was working goes on working.
#[test]
fn a_surface_that_reads_the_lights_is_refused_and_the_old_one_stays() {
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let key = as_material(&mut h, "reads_lights");
    let scene = stage.scene(Draw::new(stage.quad, Vec3::ZERO).shader(&key));
    let before = h.render(&scene);
    let bad = "// bsengine: lit
fn surface(in: VertOut) -> Surface {
    var s = material_surface(in);
    s.albedo = light.color;
    return s;
}
";
    assert!(
        h.compile_shader(&key, bad).is_err(),
        "a surface reading group 2 cannot be drawn in the reflections' prepass"
    );
    assert_eq!(h.render(&scene).max_channel_diff(&before), 0);
}

/// A shader edited from lit back to unlit, under the same path: nothing of the
/// lit one may stay behind, in the two passes only a lit one has.
#[test]
fn a_shader_edited_back_to_unlit_leaves_nothing_lit_behind() {
    let scene = |stage: &Stage, shader: &str| {
        let mut s = stage.scene(
            Draw::new(stage.quad, Vec3::ZERO)
                .roughness(0.02)
                .shader(shader),
        );
        s.camera_pos = Vec3::new(0.0, 1.2, 6.0);
        s.look_at = Vec3::new(0.0, 0.6, 0.0);
        s.ssr = Some(ScreenSpaceReflections::default());
        // The transparent pass, which picks by the same path.
        s.draws.push(
            Draw::new(stage.cube, Vec3::new(1.5, 0.5, 2.0))
                .opacity(0.5)
                .shader(shader),
        );
        s
    };
    let unlit = "@vertex fn vs_main(@location(0) pos: vec3<f32>) -> @builtin(position) vec4<f32> {
    return vec4<f32>(pos.x * 0.1, pos.y * 0.1, 0.5, 1.0);
}
@fragment fn fs_main() -> @location(0) vec4<f32> { return vec4<f32>(0.1, 0.1, 0.1, 1.0); }
";
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let key = as_material(&mut h, "edited");
    let lit_picture = settled(&mut h, &scene(&stage, &key), 3);
    h.compile_shader(&key, unlit)
        .expect("the unlit edit must compile");
    let edited = settled(&mut h, &scene(&stage, &key), 3);

    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    h.compile_shader("always_unlit", unlit)
        .expect("the unlit shader must compile");
    let always_unlit = settled(&mut h, &scene(&stage, "always_unlit"), 3);

    assert!(lit_picture.differs_from(&always_unlit), "premise");
    assert_eq!(
        edited.max_channel_diff(&always_unlit),
        0,
        "the edit left a lit pipeline drawing"
    );
}

/// The hard side of keeping `Output` as it was: it still ignores the light.
#[test]
fn an_unlit_output_still_ignores_the_light() {
    let node = |id: u32, kind| GraphNode {
        id,
        kind,
        position: [0.0, 0.0],
    };
    let unlit = ShaderGraph {
        nodes: vec![
            node(0, NodeKind::ConstantVec3([0.4, 0.4, 0.4])),
            node(1, NodeKind::Output),
        ],
        edges: vec![Edge {
            from: (0, "out".to_string()),
            to: (1, "color".to_string()),
        }],
    };
    let mut h = Harness::new();
    let stage = Stage::new(&mut h);
    let unlit = compile(&mut h, &unlit, "still_unlit");
    let lit = as_material(&mut h, "contrast_lit");
    let pictures = |h: &mut Harness, shader: &str| {
        let draw = || Draw::new(stage.quad, Vec3::ZERO).shader(shader);
        let mut bright = stage.scene(draw());
        bright.draws.truncate(1);
        let mut dark = stage.scene(draw());
        dark.draws.truncate(1);
        dark.light.color = Vec3::ZERO;
        dark.light.points.clear();
        (h.render(&bright), h.render(&dark))
    };
    let (bright, dark) = pictures(&mut h, &unlit);
    assert_eq!(bright.max_channel_diff(&dark), 0, "Output must stay unlit");
    let (bright, dark) = pictures(&mut h, &lit);
    assert!(bright.differs_from(&dark), "LitOutput must not");
}
