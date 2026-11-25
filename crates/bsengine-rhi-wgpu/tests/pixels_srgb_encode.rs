//! A target that cannot encode sRGB gets the frame any other target gets.
//!
//! An Android GLES surface offers `Rgba8Unorm` and nothing else -- no sRGB
//! format, and no sRGB view of this one -- so the encode an `…Srgb` target
//! does on every store is done by a pass of the renderer's own
//! (`srgb_encode.rs`). Before it, the emulator's frame came out with linear
//! values stored as they were: the same scene as on Vulkan, much darker.
//!
//! The harness's `Rgba8Unorm` target stands in for that surface on any
//! adapter: `Output::needs_srgb_encode` decides by the target's format, the
//! same rule for both. What is compared is the whole frame, the scene, the
//! post-process chain and the UI, against the usual `Rgba8UnormSrgb` target's.

mod common;

use common::{Draw, Harness, Scene, HEIGHT, WIDTH};
use glam::Vec3;
use std::collections::HashMap;

/// A lit cube with a gradient across its faces and a line of HUD text: dark,
/// mid and light values, and egui's pass as well as the scene's.
fn scene(cube: u64) -> Scene {
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO).colour(Vec3::new(0.8, 0.5, 0.2))],
        hud: HashMap::from([("0".to_string(), "SCORE 1234".to_string())]),
        ..Scene::default()
    }
}

#[test]
fn an_rgba8unorm_target_shows_the_frame_an_srgb_target_does() {
    let mut usual = Harness::new();
    let mut unorm = Harness::new_in_format(wgpu::TextureFormat::Rgba8Unorm);
    // The premise: the one renderer takes the path, the other does not. A
    // renderer that drew into the `Rgba8Unorm` target directly would fail
    // the comparison below too, but this says why.
    assert!(
        unorm.surface().encodes_srgb_itself(),
        "an Rgba8Unorm target must be drawn through the sRGB encode pass"
    );
    assert!(
        !usual.surface().encodes_srgb_itself(),
        "an sRGB target encodes on its own; the pass would encode twice"
    );

    let cube = usual.cube();
    let expected = usual.render(&scene(cube));
    let cube = unorm.cube();
    let got = unorm.render(&scene(cube));

    // The premise of the comparison: the frame has the values a missing
    // encode changes most. The clear colour is 85 (linear 0.09), which
    // unencoded is ~23, and the cube spans more than that.
    let distinct: std::collections::HashSet<[u8; 4]> = expected
        .data
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect();
    assert!(
        distinct.len() > 50,
        "the scene must have a spread of colours to compare, saw {} ({})",
        distinct.len(),
        expected.describe()
    );
    let without_hud = usual.render(&Scene {
        hud: HashMap::new(),
        ..scene(cube)
    });
    assert!(
        expected.differs_from(&without_hud),
        "the HUD text must be in the frame, or the UI's pass is not compared"
    );

    // The shader's curve against the fixed-function one: a step of rounding
    // apart at most, where the frame was a sum of several of them.
    let worst = got.max_channel_diff(&expected);
    assert!(
        worst <= 2,
        "the Rgba8Unorm target's frame differs by up to {worst} from the sRGB \
         target's: got {}, expected {} ({} pixels differ)",
        got.describe(),
        expected.describe(),
        got.differing_pixels(&expected)
    );
}

/// The frame texture is screen-sized: after a resize the pass must read the
/// new one, or it encodes a texture no pass draws into any more.
#[test]
fn after_a_resize_the_encode_reads_the_new_frame() {
    let mut usual = Harness::new();
    let mut unorm = Harness::new_in_format(wgpu::TextureFormat::Rgba8Unorm);
    let cube = usual.cube();
    let empty = usual.render(&Scene::default());
    let expected = usual.render(&scene(cube));

    // The frame before the resize is an empty one: a pass still reading the
    // old texture would show it, not the cube.
    unorm.render(&Scene::default());
    unorm.recreate_targets();
    let cube = unorm.cube();
    let got = unorm.render(&scene(cube));

    assert!(
        expected.differs_from(&empty),
        "the cube must change the frame, or a stale frame would pass"
    );
    let worst = got.max_channel_diff(&expected);
    assert!(
        worst <= 2,
        "after a resize the frame differs by up to {worst} from the sRGB target's \
         ({WIDTH}x{HEIGHT}): got {}, expected {}",
        got.describe(),
        expected.describe()
    );
}
