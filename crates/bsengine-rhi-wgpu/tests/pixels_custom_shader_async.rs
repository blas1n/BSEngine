//! Custom shaders compile off the frame: `compile_and_store_shader` checks the
//! source with naga, hands the device's half -- the driver compiling the module
//! and every pipeline made from it -- to a thread of its own, and returns. A
//! later frame picks the verdict up. On the main thread that half held a
//! Radeon iGPU's frame for ~0.8 s per lit shader.
//!
//! The harness's `compile_shader` waits for the verdict (`finish_custom_shaders`),
//! as every other pixel test needs; this file goes through the surface directly
//! to see the waiting itself.

mod common;

use common::{Draw, Harness, Pixels, Scene};
use glam::Vec3;

/// A lit shader no driver has compiled before, with `albedo`.
///
/// Fresh each time because drivers cache compiled shaders by source: the same
/// lit WGSL compiled a second time took ~15 ms rather than ~0.8 s, which would
/// leave a timing comparison nothing to compare.
fn fresh_lit(albedo: Vec3) -> String {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("clock after 1970")
        .as_nanos();
    format!(
        "// bsengine: lit
// {nonce}
fn surface(in: VertOut) -> Surface {{
    var s = material_surface(in);
    s.albedo = vec3<f32>({:?}, {:?}, {:?});
    return s;
}}
",
        albedo.x, albedo.y, albedo.z
    )
}

const ALBEDO: Vec3 = Vec3::new(0.8, 0.2, 0.1);

/// A white 6 x 6 quad at y = 0 seen from above. White, not the harness cube:
/// the cube's faces are tinted by vertex colour, which the standard material
/// multiplies in and a `surface` setting `albedo` does not.
fn scene(h: &mut Harness, draw: impl FnOnce(Draw) -> Draw) -> Scene {
    let corner = |x: f32, z: f32| bsengine_rhi_wgpu::Vertex {
        position: [x, 0.0, z],
        color: [1.0; 3],
        normal: [0.0, 1.0, 0.0],
        uv: [x / 6.0 + 0.5, z / 6.0 + 0.5],
        uv2: [0.5; 2],
        tangent: [0.0; 4],
    };
    let quad = h.mesh(
        &[
            corner(-3.0, -3.0),
            corner(3.0, -3.0),
            corner(3.0, 3.0),
            corner(-3.0, 3.0),
        ],
        &[0, 2, 1, 0, 3, 2],
    );
    Scene {
        draws: vec![draw(Draw::new(quad, Vec3::ZERO))],
        camera_pos: Vec3::new(0.0, 6.0, 0.01),
        ..Scene::default()
    }
}

/// The standard material in `albedo`: what a lit shader setting only that
/// draws, byte for byte (`a_hand_written_surface_is_lit_like_a_graph`).
fn standard(h: &mut Harness, albedo: Vec3) -> Pixels {
    let s = scene(h, |d| d.colour(albedo));
    h.render(&s)
}

/// The verdicts `finish_custom_shaders` gave for `key`.
fn verdicts_for(h: &mut Harness, key: &str) -> Vec<Result<(), String>> {
    h.surface_mut()
        .finish_custom_shaders()
        .into_iter()
        .filter(|(path, _)| path == key)
        .map(|(_, v)| v)
        .collect()
}

#[test]
fn a_compile_returns_before_the_device_has_finished() {
    let mut h = Harness::new();
    let key = "returns_early";
    let as_standard = standard(&mut h, ALBEDO);
    let white = standard(&mut h, Vec3::ONE);
    let source = fresh_lit(ALBEDO);

    let start = std::time::Instant::now();
    h.surface_mut()
        .compile_and_store_shader(key, &source)
        .expect("valid WGSL is handed over");
    let handed_over = start.elapsed();
    // Deterministic, unlike the times: only a frame or a wait stores a
    // verdict, and neither has happened. Compiling in the call and storing
    // there fails here.
    assert!(
        !h.surface().has_custom_shader(key),
        "the pipeline was stored before any frame or wait"
    );

    let start = std::time::Instant::now();
    assert_eq!(verdicts_for(&mut h, key), vec![Ok(())]);
    let still_compiling = start.elapsed();
    assert!(h.surface().has_custom_shader(key));

    // The device's half, still running after the call returned, took longer
    // than the whole call did. It includes wgpu's own naga pass over the same
    // source -- at least the call's check again -- before the backend's
    // translation and the driver's compile of five pipelines. Compiling in
    // the call and handing back a verdict already set fails here: the call
    // then takes all of it and the wait none.
    assert!(
        handed_over < still_compiling,
        "the call took {handed_over:?}; the device then took {still_compiling:?} more"
    );

    let s = scene(&mut h, |d| d.colour(Vec3::ONE).shader(key));
    let drawn = h.render(&s);
    assert!(as_standard.differs_from(&white), "premise");
    assert_eq!(
        drawn.max_channel_diff(&as_standard),
        0,
        "the compiled shader is not what is drawn"
    );
}

/// No one waits in a game: a frame stores what has arrived, and until then
/// the draw is the standard material's.
#[test]
fn a_frame_picks_the_verdict_up_by_itself() {
    let mut h = Harness::new();
    let key = "picked_up";
    let as_standard = standard(&mut h, ALBEDO);
    let white = standard(&mut h, Vec3::ONE);
    assert!(as_standard.differs_from(&white), "premise");
    let s = scene(&mut h, |d| d.colour(Vec3::ONE).shader(key));
    h.surface_mut()
        .compile_and_store_shader(key, &fresh_lit(ALBEDO))
        .expect("valid WGSL is handed over");

    // A wall clock rather than a frame count: how long the driver takes is
    // the driver's business (WARP's first compile of a shader is far slower
    // than a GPU's).
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    let mut frames = 0;
    let drawn = loop {
        let frame = h.render(&s);
        frames += 1;
        if h.surface().has_custom_shader(key) {
            // The frame that settles it draws with it.
            break frame;
        }
        assert_eq!(
            frame.max_channel_diff(&white),
            0,
            "while compiling, the draw is the standard material's"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "no frame stored the shader in 300 s ({frames} frames)"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    };
    // Premise: some frame was drawn while it compiled, or the check inside
    // the loop never ran. Measured 73 frames on Vulkan, 8 on Radeon D3D12,
    // 5 on WARP (debug builds): a lit compile is several of them long.
    assert!(frames > 1, "the shader was ready by the first frame");
    assert_eq!(drawn.max_channel_diff(&as_standard), 0);
}

/// A save while the last save is still compiling: the newer source is the one
/// that lands, and the older one never does -- it would otherwise be stored
/// on whichever frame its thread happened to finish, over the newer.
#[test]
fn a_newer_source_supersedes_one_still_waiting() {
    let mut h = Harness::new();
    let key = "superseded";
    let newer = Vec3::new(0.1, 0.7, 0.2);
    let as_newer = standard(&mut h, newer);
    let as_older = standard(&mut h, ALBEDO);
    assert!(as_newer.differs_from(&as_older), "premise");
    h.surface_mut()
        .compile_and_store_shader(key, &fresh_lit(ALBEDO))
        .expect("valid WGSL is handed over");
    h.surface_mut()
        .compile_and_store_shader(key, &fresh_lit(newer))
        .expect("valid WGSL is handed over");

    assert_eq!(
        verdicts_for(&mut h, key),
        vec![Ok(())],
        "only the newer source is waited for"
    );
    let s = scene(&mut h, |d| d.colour(Vec3::ONE).shader(key));
    assert_eq!(h.render(&s).max_channel_diff(&as_newer), 0);
}

/// What only the device can refuse -- a `surface` reading the lights, which the
/// reflections' prepass has no group for -- is no longer the call's to report:
/// naga accepts it, so the call says it was handed over, and the refusal
/// arrives with the verdict.
#[test]
fn a_device_rejection_arrives_with_the_verdict() {
    let mut h = Harness::new();
    let key = "rejected_later";
    let reads_lights = "// bsengine: lit
fn surface(in: VertOut) -> Surface {
    var s = material_surface(in);
    s.albedo = light.color;
    return s;
}
";
    h.surface_mut()
        .compile_and_store_shader(key, reads_lights)
        .expect("naga accepts it; only the device can refuse it");
    let verdicts = verdicts_for(&mut h, key);
    assert!(
        matches!(verdicts.as_slice(), [Err(_)]),
        "the device must refuse it: {verdicts:?}"
    );
    assert!(!h.surface().has_custom_shader(key));
}
