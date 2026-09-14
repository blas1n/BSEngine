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

/// The bounding box of pixels that differ between two frames, if any do.
///
/// Returns `(x0, y0, x1, y1)`, exclusive on the far edges.
///
/// A bounding box rather than a hand-picked sample rectangle, deliberately.
/// The first version of this test asserted that specific pixels changed, which
/// meant it also encoded my guesses about egui's button metrics, the theme's
/// fill colour and where a label lands inside its frame — none of which this
/// test is about, and none of which I could check on a machine whose GPU can no
/// longer create a device. Asking *where the drawing moved to* needs none of
/// them.
fn changed_bounds(a: &common::Pixels, b: &common::Pixels) -> Option<(u32, u32, u32, u32)> {
    let mut bounds: Option<(u32, u32, u32, u32)> = None;
    for y in 0..a.height.min(b.height) {
        for x in 0..a.width.min(b.width) {
            if a.at(x, y) != b.at(x, y) {
                bounds = Some(match bounds {
                    None => (x, y, x + 1, y + 1),
                    Some((x0, y0, x1, y1)) => (x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)),
                });
            }
        }
    }
    bounds
}

/// The end-to-end claim: a container puts its children somewhere the child's
/// own coordinates do not.
///
/// The pure layout tests in `bsengine-core` prove the arithmetic; this proves
/// the renderer asks for it. Until this branch the harness built its own
/// `UiState::default()`, so no pixel test could drive widgets at all — the draw
/// path could have ignored containers, or anchors entirely, and everything here
/// would still have passed. This is the first test in the repo to render a
/// `UiWidget` and look at the result.
/// Renders a scene twice and returns the second frame.
///
/// egui derives an `Area`'s clip rectangle from the size it had on the
/// previous frame, so the frame on which an area first appears clips its
/// contents away entirely. HUD text does not go through an area — it paints
/// straight into a layer — which is why the HUD test above needs only one
/// frame and this one does not. A game running continuously never notices; a
/// test that renders a single frame sees nothing at all.
fn render_settled(h: &mut Harness, scene: &Scene) -> common::Pixels {
    h.render(scene);
    h.render(scene)
}

#[test]
fn a_container_moves_its_child_away_from_the_childs_own_coordinates() {
    let mut h = Harness::new();
    let blank = render_settled(&mut h, &Scene::default());

    // Loose: the button sits at its own (0, 0), the top-left corner.
    let mut loose = UiState::default();
    loose.set_widget(button("b", 80.0, 40.0));
    let loose_frame = render_settled(
        &mut h,
        &Scene {
            ui: loose,
            ..Scene::default()
        },
    );

    // Contained: the identical button, inside a row placed well away from the
    // origin. Nothing about the button itself changed.
    let mut boxed = UiState::default();
    boxed.set_widget(UiWidget::Container {
        id: "row".into(),
        // Inside the harness's 200x150 framebuffer, and far enough from the
        // origin that "drew at its own coordinates" and "drew where its
        // container says" cannot describe the same pixels.
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
    let boxed_frame = render_settled(
        &mut h,
        &Scene {
            ui: boxed,
            ..Scene::default()
        },
    );

    let loose_at = changed_bounds(&loose_frame, &blank).expect(
        "a button widget must draw something; nothing in the frame changed at all, \
         which means UI widgets do not reach the framebuffer even after the area \
         has settled — not a first-frame clipping artifact",
    );
    let boxed_at =
        changed_bounds(&boxed_frame, &blank).expect("the contained button must draw something");

    // The button is 80 wide and 40 tall at the origin, so its drawing starts
    // near the top-left corner; inside the container it starts near (100, 80).
    assert!(
        loose_at.0 < 40 && loose_at.1 < 30,
        "a button at its own (0, 0) should draw from near the top-left corner, \
         but the changed pixels start at {loose_at:?}"
    );
    assert!(
        boxed_at.0 >= 90 && boxed_at.1 >= 70,
        "the contained button must draw from near its container's (100, 80), but \
         the changed pixels start at {boxed_at:?}. Starting near the origin means \
         the renderer used the widget's own coordinates and ignored its container."
    );
}

/// The Image widget must draw its texture.
///
/// It used to call `allocate_exact_size` and nothing else -- reserving a
/// rectangle and painting no pixels -- while `texture_path` was stored and
/// ignored. With no `setImage` op either, the variant was both unreachable from
/// a game and inert if reached.
///
/// The second half is what makes this about the texture rather than about
/// drawing *something*: the identical widget with no uploaded texture must
/// still draw nothing, so a placeholder rectangle cannot satisfy the first
/// assertion.
#[test]
fn an_image_widget_draws_its_texture_and_nothing_without_one() {
    let mut h = Harness::new();
    let blank = render_settled(&mut h, &Scene::default());
    let tex = h.two_colour_texture([255, 0, 0, 255], [0, 0, 255, 255]);

    let image = |id: &str| UiWidget::Image {
        id: id.into(),
        texture_path: "ui/panel.png".into(),
        x: 40.0,
        y: 30.0,
        width: 80.0,
        height: 60.0,
        anchor: UiAnchor::TOP_LEFT,
    };

    let mut with_texture = UiState::default();
    with_texture.set_widget(image("img"));
    let drawn = render_settled(
        &mut h,
        &Scene {
            ui: with_texture,
            ui_textures: HashMap::from([("ui/panel.png".to_string(), tex)]),
            ..Scene::default()
        },
    );

    let mut unresolved = UiState::default();
    unresolved.set_widget(image("img"));
    let not_drawn = render_settled(
        &mut h,
        &Scene {
            ui: unresolved,
            // No entry: the path has not finished loading.
            ..Scene::default()
        },
    );

    let bounds = changed_bounds(&drawn, &blank)
        .expect("an image widget with an uploaded texture must draw pixels");
    assert!(
        bounds.0 >= 30 && bounds.1 >= 20,
        "the image should draw at its own (40, 30), but the changed pixels start \
         at {bounds:?}"
    );
    assert!(
        changed_bounds(&not_drawn, &blank).is_none(),
        "an image whose texture has not loaded must draw nothing; something was \
         painted, so the first assertion could be satisfied by a placeholder \
         rather than by the texture"
    );
}
