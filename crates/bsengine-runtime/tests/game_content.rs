//! Guards for claims a game's scene files make that nothing else checks.
//!
//! The E2E replays drive a whole game, but they can only ask the queries the
//! test harness exposes — position, visibility, HUD text, asset status. Nothing
//! there can see a material, so "the Pickup is made of glass" is a statement no
//! replay can confirm or deny. This file reads the scene and checks it.

use std::path::{Path, PathBuf};

use bsengine_scene::SceneDescriptor;

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("workspace root is two levels above crates/bsengine-runtime")
        .to_path_buf()
}

fn load(relative: &str) -> SceneDescriptor {
    let path = workspace_root().join(relative);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("could not read {}: {e}", path.display()));
    ron::from_str(&text).unwrap_or_else(|e| panic!("could not parse {}: {e}", path.display()))
}

#[test]
fn mini_arenas_pickup_is_transparent() {
    let scene = load("games/mini-arena/assets/scenes/main.ron");
    let pickup = scene
        .entities
        .iter()
        .find(|e| e.name == "Pickup")
        .expect("mini-arena should still have a Pickup");

    let opacity = pickup
        .opacity
        .expect("the Pickup is the scene's demonstration of transparency and must author opacity");
    assert!(
        opacity > 0.0 && opacity < 1.0,
        "a glass orb needs an opacity strictly between invisible and solid, got {opacity}"
    );
}

#[test]
fn mini_arenas_floor_is_textured() {
    // No replay can check this. The headless test app deliberately runs without
    // RenderPlugin, so nothing there ever requests a texture and
    // `get_asset_status` correctly answers "unknown" for every image -- the
    // absence is documented in test_mode.rs, not an oversight. That leaves the
    // scene file itself as the only place to assert the claim.
    let scene = load("games/mini-arena/assets/scenes/main.ron");
    let ground = scene
        .entities
        .iter()
        .find(|e| e.name == "Ground")
        .expect("mini-arena should still have a Ground");

    let texture = ground
        .texture
        .as_ref()
        .expect("the floor is this scene's demonstration of textures and must name one");
    assert!(
        texture.path().ends_with(".png"),
        "expected an image path, got {:?}",
        texture.path()
    );
    assert!(
        std::path::Path::new(
            &workspace_root()
                .join("games/mini-arena")
                .join(texture.path())
        )
        .exists(),
        "the named texture has to exist on disk: {}",
        texture.path()
    );
}

#[test]
fn every_other_mini_arena_entity_stays_solid() {
    // The counterpart that makes the test above mean something: transparency is
    // one deliberate object, not a setting that leaked across the scene.
    let scene = load("games/mini-arena/assets/scenes/main.ron");
    for entity in &scene.entities {
        if entity.name == "Pickup" {
            continue;
        }
        assert!(
            entity.opacity.is_none() || entity.opacity == Some(1.0),
            "{} should be solid, but authors opacity {:?}",
            entity.name,
            entity.opacity
        );
    }
}
