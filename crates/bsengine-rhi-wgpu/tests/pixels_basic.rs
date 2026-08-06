//! The renderer's most basic propositions.
//!
//! The first test here passing is the first time this repository has checked
//! what the renderer actually produced.

mod common;

use common::{Draw, Harness, Light, Scene};
use glam::Vec3;

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

#[test]
fn a_red_cube_in_front_of_the_camera_makes_the_centre_red() {
    let mut h = Harness::new();
    let cube = h.cube();
    let pixels = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO).colour(Vec3::new(1.0, 0.0, 0.0))],
        ..Scene::default()
    });

    let [r, g, b, _] = pixels.centre();
    assert!(
        r > g + 40 && r > b + 40,
        "expected the centre pixel to be dominated by red, saw {}",
        pixels.describe()
    );
}

#[test]
fn an_object_outside_the_view_changes_nothing() {
    let mut h = Harness::new();
    let cube = h.cube();

    let empty = h.render(&Scene::default());
    let behind_the_far_plane = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::new(0.0, 0.0, -500.0))],
        ..Scene::default()
    });
    assert!(
        !behind_the_far_plane.differs_from(&empty),
        "a cube far beyond the far plane should not appear, saw {}",
        behind_the_far_plane.describe()
    );

    // The positive control. Without it the assertion above cannot be told apart
    // from "the renderer draws nothing at all".
    let in_view = h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        ..Scene::default()
    });
    assert!(
        in_view.differs_from(&empty),
        "the same cube in view must change the frame, or the assertion above proves nothing"
    );
}

#[test]
fn a_near_cube_hides_a_far_one() {
    let mut h = Harness::new();
    let cube = h.cube();

    // The far cube is drawn *last* on purpose. With the depth test disabled the
    // later draw wins, the colours swap, and this test fails -- which is what
    // makes it a test of the depth buffer. Drawing the near cube last would
    // give the same answer either way.
    //
    // The light points away from the camera so the faces we are looking at are
    // the lit ones. Under the default sun both cubes' front faces are dim
    // enough that the two colours differ by about twenty, which is a margin
    // thin enough to be noise rather than an answer.
    let pixels = h.render(&Scene {
        draws: vec![
            Draw::new(cube, Vec3::ZERO).colour(Vec3::new(0.0, 0.0, 1.0)),
            Draw::new(cube, Vec3::new(0.0, 0.0, -6.0)).colour(Vec3::new(1.0, 0.0, 0.0)),
        ],
        light: Light {
            direction: Vec3::new(0.0, 0.0, -1.0),
            ..Light::default()
        },
        ..Scene::default()
    });

    let [r, _, b, _] = pixels.centre();
    assert!(
        b > r + 40,
        "the near blue cube should occlude the far red one, saw {}",
        pixels.describe()
    );
}
