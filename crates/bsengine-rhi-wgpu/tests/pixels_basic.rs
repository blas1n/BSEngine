//! The renderer's most basic propositions.
//!
//! The first test here passing is the first time this repository has checked
//! what the renderer actually produced.

mod common;

use common::{Harness, Scene};

#[test]
fn an_empty_frame_is_the_clear_colour() {
    let mut h = Harness::new();
    let pixels = h.render(&Scene::default());

    // 85 is measured, not derived. The main pass clears a linear 0.08 grey into
    // the HDR buffer and the frame then goes through post-processing before it
    // reaches this texture, so working the number out as a plain sRGB encode of
    // 0.08 gives the wrong answer -- that route predicts 79, and the renderer
    // produces 85.
    //
    // The tolerance covers adapters that round differently. What this test
    // defends is "a frame with nothing in it is one uniform colour", not a
    // particular byte, and the mutation that proves it is changing the clear
    // colour: 0.5 red comes back as 206.
    assert!(
        pixels.is_uniformly([85, 85, 85], 6),
        "expected a uniform clear colour, saw {}",
        pixels.describe()
    );
}
