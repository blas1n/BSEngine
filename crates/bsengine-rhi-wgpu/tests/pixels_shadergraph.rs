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
