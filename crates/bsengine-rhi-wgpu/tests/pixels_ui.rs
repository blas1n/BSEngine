//! HUD text. The only way the games put words on screen, and until now nothing
//! checked that any of it was drawn.

mod common;

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
