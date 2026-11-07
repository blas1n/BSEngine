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
        scrollable: false,
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

/// A scroll container must not let its children paint outside it.
///
/// The pure tests fix which rectangle a child is clipped to; this one proves
/// the renderer honours it. Without the clip the third button would paint below
/// the box, which is exactly what an unclipped container does today and why
/// clipping had to be opt-in.
#[test]
fn a_scroll_container_clips_what_overflows_it() {
    let mut h = Harness::new();
    let blank = render_settled(&mut h, &Scene::default());

    // A 60-tall box holding 3 x 30 of buttons: one and a bit fit.
    let scene_with = |scrollable: bool| {
        let mut ui = UiState::default();
        ui.set_widget(UiWidget::Container {
            id: "list".into(),
            x: 20.0,
            y: 20.0,
            width: 100.0,
            height: 60.0,
            anchor: UiAnchor::TOP_LEFT,
            direction: UiDirection::Vertical,
            spacing: 0.0,
            padding: 0.0,
            align: bsengine_core::UiAlign::Stretch,
            scrollable,
        });
        for id in ["a", "b", "c"] {
            ui.set_widget(button(id, 100.0, 30.0));
            ui.set_parent(id, "list");
        }
        Scene {
            ui,
            ..Scene::default()
        }
    };

    let clipped = render_settled(&mut h, &scene_with(true));
    let unclipped = render_settled(&mut h, &scene_with(false));

    // Below the box (y >= 80) the unclipped version still paints; the clipped
    // one must not.
    let below = |f: &common::Pixels| {
        (82..f.height.min(110)).any(|y| (22..118).any(|x| f.at(x, y) != blank.at(x, y)))
    };
    assert!(
        below(&unclipped),
        "the unclipped container must paint below its own box -- if it does not, \
         this fixture does not overflow and proves nothing"
    );
    assert!(
        !below(&clipped),
        "a scroll container must not paint below its own box"
    );
    // And it must still draw *inside* the box, so the clip is a clip and not a
    // way of drawing nothing at all.
    let inside = (22..118).any(|x| (22..78).any(|y| clipped.at(x, y) != blank.at(x, y)));
    assert!(
        inside,
        "the clipped container must still draw within its box"
    );
}

/// The font the UI font tests load (`tests/fonts/make_test_font.py`): '한'
/// and '글', which none of egui's built-in fonts has, as three horizontal
/// and three vertical bars; 'A' as a solid block; nothing else.
const TEST_FONT: &[u8] = include_bytes!("fonts/test_glyphs.ttf");

fn test_font() -> bsengine_core::UiFont {
    bsengine_core::UiFont {
        path: "assets/fonts/test_glyphs.ttf".into(),
        bytes: TEST_FONT.to_vec(),
    }
}

fn hud(h: &mut Harness, text: &str) -> common::Pixels {
    h.render(&Scene {
        hud: HashMap::from([("0".to_string(), text.to_string())]),
        ..Scene::default()
    })
}

/// The reason for the feature: without a font that has them, two different
/// Hangul syllables draw as the same box, so a Korean string table is a row
/// of boxes. With the project's font they are two different glyphs.
#[test]
fn a_project_font_draws_what_the_built_in_fonts_cannot() {
    let mut h = Harness::new();
    let blank = h.render(&Scene::default());
    let han_boxed = hud(&mut h, "한");
    let geul_boxed = hud(&mut h, "글");
    // The premise, both halves: something is drawn, and it is the same thing
    // for both characters -- a replacement, not the characters.
    assert!(
        han_boxed.differs_from(&blank),
        "premise: the HUD draws at all"
    );
    assert!(
        !han_boxed.differs_from(&geul_boxed),
        "premise: without the font, '한' and '글' are the same replacement box"
    );

    let installed = h.surface_mut().set_ui_fonts(&[test_font()]);
    assert_eq!(installed, ["assets/fonts/test_glyphs.ttf"]);
    let han = hud(&mut h, "한");
    let geul = hud(&mut h, "글");
    assert!(
        han.differs_from(&geul),
        "with the font, '한' and '글' are different glyphs"
    );
    assert!(
        han.differs_from(&han_boxed),
        "and '한' is no longer the replacement box"
    );
}

/// Order, both ways: the project's font is in front -- its 'A' (a block)
/// replaces the built-in 'A' -- and a character it lacks still comes from
/// the built-in fonts behind it, unchanged.
#[test]
fn the_project_font_comes_first_and_the_built_in_fonts_fill_in() {
    let mut h = Harness::new();
    let a_built_in = hud(&mut h, "A");
    let b_built_in = hud(&mut h, "B");
    assert!(
        a_built_in.differs_from(&b_built_in),
        "premise: the built-in 'A' and 'B' differ"
    );

    h.surface_mut().set_ui_fonts(&[test_font()]);
    let blank = h.render(&Scene::default());
    assert!(
        !same_drawing(&blank, &hud(&mut h, "A"), &a_built_in),
        "a character the project font has is drawn in the project font"
    );
    // The same drawing, not the same frame: the line's ascent and descent are
    // the first font's, so a fallback glyph sits on the project font's
    // baseline -- 2 px higher than on the built-in one's, with this font.
    assert!(
        same_drawing(&blank, &hud(&mut h, "B"), &b_built_in),
        "a character it lacks is the built-in font's glyph, pixel for pixel"
    );
}

/// Whether `a` and `b` draw the same thing over `blank`, wherever each put
/// it: the boxes of what changed are the same size and hold the same pixels.
fn same_drawing(blank: &common::Pixels, a: &common::Pixels, b: &common::Pixels) -> bool {
    let (Some(p), Some(q)) = (changed_bounds(blank, a), changed_bounds(blank, b)) else {
        return false;
    };
    let size = (p.2 - p.0, p.3 - p.1);
    size == (q.2 - q.0, q.3 - q.1)
        && (0..size.1)
            .all(|y| (0..size.0).all(|x| a.at(p.0 + x, p.1 + y) == b.at(q.0 + x, q.1 + y)))
}

/// Between project fonts the manifest's order holds: of two fonts that both
/// have 'A', the first one listed draws it. Each order against the drawing
/// that font makes alone, so a list reversed on the way in fails one way or
/// the other.
#[test]
fn the_first_listed_project_font_draws_a_character_both_have() {
    let mut h = Harness::new();
    let blank = h.render(&Scene::default());
    let alt = bsengine_core::UiFont {
        path: "assets/fonts/test_glyphs_alt.ttf".into(),
        bytes: include_bytes!("fonts/test_glyphs_alt.ttf").to_vec(),
    };
    h.surface_mut().set_ui_fonts(&[test_font()]);
    let a_block = hud(&mut h, "A");
    h.surface_mut().set_ui_fonts(std::slice::from_ref(&alt));
    let a_bars = hud(&mut h, "A");
    assert!(
        !same_drawing(&blank, &a_block, &a_bars),
        "premise: the two fonts draw 'A' differently"
    );

    h.surface_mut().set_ui_fonts(&[test_font(), alt.clone()]);
    assert!(
        same_drawing(&blank, &hud(&mut h, "A"), &a_block),
        "listed first, the block font draws 'A'"
    );
    h.surface_mut().set_ui_fonts(&[alt, test_font()]);
    assert!(
        same_drawing(&blank, &hud(&mut h, "A"), &a_bars),
        "listed first, the bars font draws 'A'"
    );
}

/// A corrupt font costs its own glyphs, not the game: egui panics on a font
/// it cannot parse, so one has to be refused before egui sees it -- and the
/// good font after it still goes in.
#[test]
fn a_file_that_is_not_a_font_is_left_out_and_the_rest_go_in() {
    let mut h = Harness::new();
    let han_boxed = hud(&mut h, "한");
    let not_a_font = bsengine_core::UiFont {
        path: "assets/fonts/broken.ttf".into(),
        bytes: b"this is not a font".to_vec(),
    };
    let installed = h.surface_mut().set_ui_fonts(&[not_a_font, test_font()]);
    assert_eq!(installed, ["assets/fonts/test_glyphs.ttf"]);
    assert!(
        hud(&mut h, "한").differs_from(&han_boxed),
        "the frame is drawn, with the font that did parse"
    );
}

/// An empty list is the built-in fonts alone: fonts set before do not linger.
#[test]
fn an_empty_font_list_is_the_built_in_fonts_again() {
    let mut h = Harness::new();
    let han_boxed = hud(&mut h, "한");
    h.surface_mut().set_ui_fonts(&[test_font()]);
    assert!(
        hud(&mut h, "한").differs_from(&han_boxed),
        "premise: the font took"
    );
    h.surface_mut().set_ui_fonts(&[]);
    assert!(
        !hud(&mut h, "한").differs_from(&han_boxed),
        "back to the replacement box"
    );
}
