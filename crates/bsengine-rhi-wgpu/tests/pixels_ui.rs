//! HUD text. The only way the games put words on screen, and until now nothing
//! checked that any of it was drawn.

mod common;

use bsengine_core::{UiAnchor, UiDirection, UiState, UiWidget};
use common::{Harness, Scene};
use std::collections::HashMap;

#[test]
fn hud_text_reaches_the_framebuffer() {
    let mut h = Harness::new();
    let blank = h.render(&Scene::default());

    let with_text = h.render(&Scene {
        hud: HashMap::from([("0".to_string(), "SCORE 1234".to_string())]),
        ..Scene::default()
    });
    assert!(
        with_text.differs_from(&blank),
        "HUD text should change the frame"
    );

    // An empty string must draw nothing. Without this the assertion above says
    // only "putting a key in the HUD map changes something", which a renderer
    // that drew an empty box would also satisfy.
    let with_empty = h.render(&Scene {
        hud: HashMap::from([("0".to_string(), String::new())]),
        ..Scene::default()
    });
    assert!(
        !with_empty.differs_from(&blank),
        "an empty HUD string should draw nothing at all"
    );
}

/// A button with no authored position, so only a container can place it.
fn button(id: &str, width: f32, height: f32) -> UiWidget {
    UiWidget::Button {
        id: id.into(),
        label: "OK".into(),
        x: 0.0,
        y: 0.0,
        width,
        height,
        anchor: UiAnchor::TOP_LEFT,
    }
}

/// Whether any pixel inside the given box differs between two frames.
fn differs_in(a: &common::Pixels, b: &common::Pixels, x0: u32, y0: u32, x1: u32, y1: u32) -> bool {
    (y0..y1.min(a.height)).any(|y| (x0..x1.min(a.width)).any(|x| a.at(x, y) != b.at(x, y)))
}

/// The end-to-end claim: a container puts its children somewhere the child's
/// own coordinates do not.
///
/// The pure layout tests in `bsengine-core` prove the arithmetic; this proves
/// the renderer asks for it. Until this branch the harness built its own
/// `UiState::default()`, so the draw path could have ignored containers
/// entirely -- or ignored anchors, for that matter -- and every test here would
/// still have passed.
#[test]
fn a_container_moves_its_child_away_from_the_childs_own_coordinates() {
    let mut h = Harness::new();
    let blank = h.render(&Scene::default());

    // Loose: the button sits at its own (0, 0), the top-left corner.
    let mut loose = UiState::default();
    loose.set_widget(button("b", 80.0, 40.0));
    let loose_frame = h.render(&Scene {
        ui: loose,
        ..Scene::default()
    });

    // Contained: the identical button, inside a row anchored far from the
    // origin. Nothing about the button itself changed.
    let mut boxed = UiState::default();
    boxed.set_widget(UiWidget::Container {
        id: "row".into(),
        // Inside the harness's 200x150 framebuffer, and far enough from the
        // origin that "drew at its own coordinates" and "drew where its
        // container says" cannot both be true of the same pixels.
        x: 100.0,
        y: 80.0,
        width: 90.0,
        height: 50.0,
        anchor: UiAnchor::TOP_LEFT,
        direction: UiDirection::Horizontal,
        spacing: 0.0,
        padding: 0.0,
        align: bsengine_core::UiAlign::Stretch,
    });
    boxed.set_widget(button("b", 80.0, 40.0));
    boxed.set_parent("b", "row");
    let boxed_frame = h.render(&Scene {
        ui: boxed,
        ..Scene::default()
    });

    assert!(
        differs_in(&loose_frame, &blank, 2, 2, 60, 30),
        "the loose button must actually draw at the top-left; if it does not, \
         the rest of this test is comparing two blank frames"
    );
    assert!(
        !differs_in(&boxed_frame, &blank, 2, 2, 60, 30),
        "the contained button must NOT draw at its own (0, 0) -- if it does, \
         the renderer used the widget's coordinates and ignored its container"
    );
    assert!(
        differs_in(&boxed_frame, &blank, 105, 85, 185, 128),
        "the contained button must draw inside its container at (100, 80)"
    );
}
