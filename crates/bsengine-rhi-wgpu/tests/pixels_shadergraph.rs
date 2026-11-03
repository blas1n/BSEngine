//! The shader graph, end to end: a `.shadergraph.ron` on disk, compiled to a
//! `.wgsl` on disk, loaded as a `CustomShader`, rasterised, read back.
//!
//! `bsengine-shadergraph`'s own unit tests can prove the emitted text says what
//! it should. They cannot prove it *runs here*: passing naga and matching this
//! pipeline's bind group layout are different questions, and only the second
//! one is answered by creating the pipeline against the real
//! `pipeline_layout`. That is what this file does.
//!
//! Note what is deliberately absent: nothing downstream of the compiler is
//! touched. The generated shader travels the same `compile_and_store_shader`
//! path a hand-written one does, which is why `pixels_material`'s custom-shader
//! test and `games/mini-arena/assets/shaders/glow.wgsl` keep working unchanged
//! -- the graph route is an addition to the WGSL text path, not a replacement.

mod common;

use common::{Draw, Harness, Light, Pixels, Scene};
use glam::Vec3;

/// Straight down at the floor plane. The plane is the one mesh with white
/// vertex colours and a full 0..1 UV square, which is what makes a scroll
/// legible across the frame.
const OVERHEAD: Vec3 = Vec3::new(0.0, 6.0, 0.01);

/// Flat, bright light. Nothing here reads it -- the generated shader ignores
/// lighting entirely -- but the reference render this is compared against does.
fn even_light() -> Light {
    Light {
        direction: Vec3::new(0.0, -1.0, 0.0),
        ambient: Vec3::splat(0.2),
        ..Light::default()
    }
}

/// The authored demo graph, read from the game's asset directory.
///
/// Read from the real file rather than rebuilt in Rust here on purpose: the RON
/// is the artifact a user authors, and a graph that only exists as a literal in
/// a test proves nothing about the one on disk.
fn demo_graph_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../games/mini-arena/assets/shaders/scroll.shadergraph.ron")
}

/// Parses the demo graph, compiles it, writes the `.wgsl` out, reads that file
/// back, and builds a pipeline from what the *file* says.
///
/// The disk round-trip is the point: the user decision this cycle implements is
/// compilation to a `.wgsl` file that `CustomShader.path` names, so compiling
/// straight into a string and handing that to the GPU would skip the half of
/// the contract that involves a file at all.
///
/// `stem` names the output file, and every caller passes its own test's name:
/// cargo runs the tests in one binary on parallel threads, and a shared path
/// means one test truncating the file while another reads it back. That is not
/// hypothetical -- it is what this file did on its first full-suite run.
///
/// Returns the key a [`Draw::shader`] refers to the pipeline by.
fn compiled_scroll_shader(h: &mut Harness, stem: &str) -> String {
    let path = demo_graph_path();
    let ron_text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));

    let graph: bsengine_shadergraph::ShaderGraph = ron::from_str(&ron_text)
        .unwrap_or_else(|e| panic!("{} is not a valid ShaderGraph: {e}", path.display()));

    let wgsl = bsengine_shadergraph::compile(&graph)
        .unwrap_or_else(|e| panic!("the demo graph failed to compile: {e}"));

    let out = std::path::Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("scroll.{stem}.wgsl"));
    std::fs::write(&out, &wgsl)
        .unwrap_or_else(|e| panic!("could not write {}: {e}", out.display()));
    let from_disk = std::fs::read_to_string(&out)
        .unwrap_or_else(|e| panic!("could not read back {}: {e}", out.display()));
    assert_eq!(
        from_disk, wgsl,
        "the .wgsl on disk must be exactly what the compiler emitted"
    );

    let key = out.to_string_lossy().into_owned();
    if let Err(e) = h.compile_shader(&key, &from_disk) {
        // Printing the source is the difference between a fixable failure and
        // a guess: a bind-group or uniform-layout mismatch is reported by wgpu
        // against the pipeline, with nothing in the message about which field
        // of which struct drifted. Compare what follows against
        // `games/mini-arena/assets/shaders/glow.wgsl` field by field.
        panic!(
            "the generated WGSL did not survive pipeline creation: {e}\n\
             ----- generated shader -----\n{from_disk}\n----- end -----"
        );
    }
    key
}

/// The demo scene: a floor plane carrying a two-texel red|blue texture, drawn
/// by the compiled graph.
///
/// Two colours rather than one because a flat texture reads identically whether
/// or not the UVs are right -- and UV pass-through through the generated
/// `VertOut` is precisely what this file has to prove.
fn scroll_scene(h: &mut Harness, stem: &str) -> Scene {
    let plane = h.plane();
    let tex = h.two_colour_texture([255, 0, 0, 255], [0, 0, 255, 255]);
    let shader = compiled_scroll_shader(h, stem);

    Scene {
        draws: vec![Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(6.0, 1.0, 6.0), Vec3::ZERO)
            .textured(tex)
            .shader(&shader)],
        light: even_light(),
        camera_pos: OVERHEAD,
        ..Scene::default()
    }
}

/// Two samples well inside the plane, one either side of the texture's seam.
fn left_and_right(p: &Pixels) -> ([u8; 4], [u8; 4]) {
    (
        p.at(p.width / 4, p.height / 2),
        p.at(p.width * 3 / 4, p.height / 2),
    )
}

#[test]
fn a_compiled_graph_renders_through_custom_shader() {
    let mut h = Harness::new();
    let scene = scroll_scene(&mut h, "renders");
    let textured = h.render(&scene);

    // The same draw through the same generated pipeline, with no texture bound.
    // The renderer then falls back to its 1x1 white default, so this is exactly
    // "the untextured default" -- and it isolates the texture as the only
    // difference, which comparing against the *standard* pipeline would not.
    let plane = scene.draws[0].mesh;
    let shader = scene.draws[0]
        .custom_shader
        .clone()
        .expect("scroll_scene attaches the compiled shader");
    let untextured = h.render(&Scene {
        draws: vec![Draw::new(plane, Vec3::ZERO)
            .scaled(Vec3::new(6.0, 1.0, 6.0), Vec3::ZERO)
            .shader(&shader)],
        light: even_light(),
        camera_pos: OVERHEAD,
        ..Scene::default()
    });

    assert!(
        textured.differs_from(&untextured),
        "the compiled graph drew the same thing with and without a texture, so \
         either the sample or the UV pass-through is dead. textured: {}, \
         untextured: {}",
        textured.describe(),
        untextured.describe()
    );

    let ([lr, _, lb, _], [rr, _, rb, _]) = left_and_right(&textured);
    assert!(
        lr > lb + 20 && rb > rr + 20,
        "the graph must sample the red half on the left and the blue half on \
         the right -- anything else means `in.uv` never reached the fragment \
         stage. left {:?}, right {:?}",
        [lr, lb],
        [rr, rb]
    );
}

#[test]
fn the_scroll_graph_actually_scrolls_over_time() {
    let mut h = Harness::new();
    let scene = scroll_scene(&mut h, "scrolls");

    // 5 seconds at the graph's 0.1 units/second is exactly half a UV, which on
    // a two-texel texture swaps the halves. A quarter-second offset would also
    // "differ", but only this makes the direction and the rate legible.
    let early = h.render_at_time(&scene, 0.0);
    let late = h.render_at_time(&scene, 5.0);

    let (early_left, early_right) = left_and_right(&early);
    let (late_left, late_right) = left_and_right(&late);
    println!("t=0.0  left {early_left:?}  right {early_right:?}");
    println!("t=5.0  left {late_left:?}  right {late_right:?}");

    assert!(
        early.differs_from(&late),
        "camera.time drove nothing: the frames at t=0 and t=5 are identical. \
         t=0 {}, t=5 {}",
        early.describe(),
        late.describe()
    );
    assert!(
        early_left[0] > early_left[2] && late_left[2] > late_left[0],
        "half a UV of scroll must turn the red left side blue, saw {early_left:?} -> {late_left:?}"
    );
    assert!(
        early_right[2] > early_right[0] && late_right[0] > late_right[2],
        "and the blue right side red, saw {early_right:?} -> {late_right:?}"
    );
}

/// A 6 x 6 quad at y = 0 facing +y, `u` along +x and `v` along +z -- the
/// fixture `pixels_normal_map.rs` reads its tangent frame from. Its tangents
/// are generated at registration: +x, handedness -1 (`cross(+y, +x)` is -z
/// while `v` runs along +z), so a map's green reaches +z only if the
/// handedness is honoured.
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

/// A one-texel normal map holding the tangent-space normal `n`, linear.
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

/// Compiles `graph` and builds its pipeline, under `key`.
fn compiled(h: &mut Harness, graph: &bsengine_shadergraph::ShaderGraph, key: &str) -> String {
    let wgsl = bsengine_shadergraph::compile(graph)
        .unwrap_or_else(|e| panic!("the graph failed to compile: {e}"));
    if let Err(e) = h.compile_shader(key, &wgsl) {
        panic!(
            "the generated WGSL did not survive pipeline creation: {e}\n\
             ----- generated shader -----\n{wgsl}\n----- end -----"
        );
    }
    key.to_string()
}

/// `Normal * 0.25 + 0.25` into the colour: the world normal, as a normal map
/// encodes one but at half the brightness, so +x reads red, +y green and +z
/// blue. Half, because the HDR output is tonemapped: at `* 0.5 + 0.5` the
/// colours sit in the curve's shoulder, where 0.5 and 1.0 came out 211 and
/// 234 -- too close together to tell a tilt from noise.
fn normal_as_colour() -> bsengine_shadergraph::ShaderGraph {
    use bsengine_shadergraph::{Edge, GraphNode, NodeKind};
    let node = |id, kind| GraphNode {
        id,
        kind,
        position: [0.0, 0.0],
    };
    let edge = |from: u32, to: u32, port: &str| Edge {
        from: (from, "out".to_string()),
        to: (to, port.to_string()),
    };
    bsengine_shadergraph::ShaderGraph {
        nodes: vec![
            node(0, NodeKind::Normal),
            node(1, NodeKind::Constant(0.25)),
            node(2, NodeKind::Multiply),
            node(3, NodeKind::Add),
            node(4, NodeKind::Output),
        ],
        edges: vec![
            edge(0, 2, "a"),
            edge(1, 2, "b"),
            edge(2, 3, "a"),
            edge(1, 3, "b"),
            edge(3, 4, "color"),
        ],
    }
}

/// The centre pixel of the quad drawn by `shader`, from overhead.
fn centre(h: &mut Harness, draw: Draw) -> [u8; 4] {
    h.render(&Scene {
        draws: vec![draw],
        light: even_light(),
        camera_pos: OVERHEAD,
        ..Scene::default()
    })
    .centre()
}

/// The `Normal` node sees the material's normal map, in the mesh's tangent
/// frame: a quad facing +y reads green-dominant with no map, and a map tilted
/// 45 degrees toward +u (the tangent, +x) or +v (+z, through the handedness)
/// tips the colour toward red or toward blue. A node that ignored the map --
/// the vertex normal alone, which is what a graph could see before -- reads
/// the same colour all three times; one with the bitangent's sign fixed sends
/// +v to -z, which reads less blue than flat rather than more.
#[test]
fn the_normal_node_turns_with_the_material_normal_map() {
    let mut h = Harness::new();
    let q = quad(&mut h);
    let shader = compiled(&mut h, &normal_as_colour(), "normal_as_colour.wgsl");
    let tilt = |degrees: f32, along_v: bool| {
        let (s, c) = degrees.to_radians().sin_cos();
        if along_v {
            Vec3::new(0.0, s, c)
        } else {
            Vec3::new(s, 0.0, c)
        }
    };
    let toward_u = normal_map(&mut h, tilt(45.0, false));
    let toward_v = normal_map(&mut h, tilt(45.0, true));

    let flat = centre(&mut h, Draw::new(q, Vec3::ZERO).shader(&shader));
    let along_u = centre(
        &mut h,
        Draw::new(q, Vec3::ZERO)
            .normal_mapped(toward_u, 1.0)
            .shader(&shader),
    );
    let along_v = centre(
        &mut h,
        Draw::new(q, Vec3::ZERO)
            .normal_mapped(toward_v, 1.0)
            .shader(&shader),
    );
    println!("flat {flat:?}  +u {along_u:?}  +v {along_v:?}");

    // The premise: the flat quad reads its +y normal -- green over an equal
    // red and blue -- so the shader drew, and drew the normal.
    let [fr, fg, fb, _] = flat;
    assert!(
        fg > fr + 30 && fr.abs_diff(fb) <= 2,
        "the flat quad must read +y (green over equal red and blue), saw {flat:?}"
    );
    // (sin 45, cos 45, 0): red up to green's level, blue unchanged.
    let [ur, _, ub, _] = along_u;
    assert!(
        ur > fr + 30 && ub.abs_diff(fb) <= 2,
        "a map tilted toward +u must turn the normal toward +x (red), saw \
         flat {flat:?} -> {along_u:?}"
    );
    // (0, cos 45, sin 45): blue up, red unchanged.
    let [vr, _, vb, _] = along_v;
    assert!(
        vb > fb + 30 && vr.abs_diff(fr) <= 2,
        "a map tilted toward +v must turn the normal toward +z (blue) -- the \
         tangent's handedness -- saw flat {flat:?} -> {along_v:?}"
    );
}

/// A hand-written custom shader can declare the tangent: the custom pipeline
/// offers every vertex attribute the mesh pipeline does. Its copy of the
/// layout used to stop at location 3, so declaring `@location(5)` failed
/// pipeline creation and no hand-written shader could read a normal map.
/// The colour is the tangent itself, so it also proves the data arrives: the
/// quad's is +x, red; an attribute bound to nothing would read 0, grey.
#[test]
fn a_hand_written_custom_shader_reads_the_vertex_tangent() {
    const TANGENT_WGSL: &str = r#"
struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
struct ModelUniform {
    model: mat4x4<f32>,
};
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(5) tangent: vec4<f32>,
}
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) tangent: vec4<f32>,
}

@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    out.clip_pos = camera.view_proj * model_data.model * vec4<f32>(in.pos, 1.0);
    out.tangent = in.tangent;
    return out;
}

@fragment
fn fs_main(in: VertOut) -> @location(0) vec4<f32> {
    // Half brightness, out of the tonemapper's shoulder (see normal_as_colour).
    return vec4<f32>(in.tangent.xyz * 0.25 + vec3<f32>(0.25), 1.0);
}
"#;
    let mut h = Harness::new();
    let q = quad(&mut h);
    h.compile_shader("tangent.wgsl", TANGENT_WGSL)
        .unwrap_or_else(|e| panic!("a shader declaring the tangent must compile: {e}"));

    let [r, g, b, _] = centre(&mut h, Draw::new(q, Vec3::ZERO).shader("tangent.wgsl"));
    assert!(
        r > g + 30 && r > b + 30,
        "the quad's tangent is +x, so the shader must read red, saw {:?}",
        [r, g, b]
    );
}

#[test]
fn holding_time_still_leaves_consecutive_frames_identical() {
    let mut h = Harness::new();
    let scene = scroll_scene(&mut h, "still");

    // The opposite direction, and the reason the scroll test means anything:
    // if consecutive frames differed on their own -- dither, jitter, an
    // uninitialised history buffer -- then "the pixels differ" would be true
    // whether or not time drove the shader, and that test would certify
    // nothing at all.
    let first = h.render_at_time(&scene, 3.0);
    let second = h.render_at_time(&scene, 3.0);

    assert_eq!(
        first.differing_pixels(&second),
        0,
        "two frames at the same time must be identical, else the scroll test's \
         difference could be noise rather than the clock. first {}, second {}",
        first.describe(),
        second.describe()
    );
}
