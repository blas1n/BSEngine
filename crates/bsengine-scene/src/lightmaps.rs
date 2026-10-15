//! Which baked lightmap each entity of a scene reads.
//!
//! A bake writes this beside the scene rather than into it, as Godot keeps a
//! `LightmapGIData` resource and Unity a `LightingData` asset: the scene file
//! is what a person wrote, and a re-bake should not rewrite it (or conflict
//! with someone else's edit of it). Loading the scene reads it back and gives
//! each named entity its [`Lightmap`](bsengine_core::Lightmap).
//!
//! The extension is `.lightmaps`, not `.ron`, although the contents are RON:
//! every `.ron` under a project's assets is a scene or a prefab to the cook
//! and to the identity fixup, and this is neither.
//!
//! Entities are found by name, the key scene files already use (a prefab
//! refuses duplicate names; the bake refuses them among its static entities).

use serde::{Deserialize, Serialize};

/// The data file's extension.
pub const EXTENSION: &str = "lightmaps";

/// One scene's lightmap assignments.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightmapData {
    /// [`fingerprint`] of the scene file the bake read. A scene edited since
    /// still loads its lightmaps, with a warning: they show the scene as it
    /// was, which is usually close, and never worse than none.
    pub scene_fingerprint: String,
    /// One per lightmapped entity.
    pub entries: Vec<LightmapEntry>,
}

/// One entity's lightmap.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightmapEntry {
    /// The entity's `Name`.
    pub entity: String,
    /// The atlas layer's texture, project-relative as scenes write paths.
    pub texture: String,
    /// Its rectangle of the layer (`Lightmap::scale_offset`).
    pub scale_offset: [f32; 4],
}

/// Where a scene's lightmap data lives: beside it, the scene's extension
/// replaced -- `assets/scenes/main.ron` reads `assets/scenes/main.lightmaps`.
pub fn data_path(scene_path: &str) -> String {
    let p = std::path::Path::new(scene_path);
    p.with_extension(EXTENSION)
        .to_string_lossy()
        .replace('\\', "/")
}

/// A stable digest of a scene file's text (FNV-1a, 64-bit). Line endings are
/// normalised first, so a checkout that converts them does not make every
/// bake look stale.
pub fn fingerprint(scene_text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in scene_text.bytes().filter(|&b| b != b'\r') {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    format!("fnv1a64:{h:016x}")
}

/// Gives each entity named in `scene_path`'s lightmap data its `Lightmap`,
/// if the scene has been baked. Returns how many entities got one.
///
/// Silent when there is no data file -- most scenes are never baked. Warns,
/// and still applies, when the scene changed since the bake; warns about a
/// name the data has and the world does not (an entity renamed or deleted
/// since), which is what a stale bake most often looks like.
pub fn apply(world: &mut bevy_ecs::world::World, scene_path: &str, scene_text: &str) -> usize {
    let path = data_path(scene_path);
    let Ok(text) = bsengine_asset::pak_source::read_to_string(&path) else {
        return 0;
    };
    let data: LightmapData = match ron::from_str(&text) {
        Ok(d) => d,
        Err(e) => {
            tracing::warn!("lightmaps: cannot read {path}: {e}; the scene loads unlit by them");
            return 0;
        }
    };
    if data.scene_fingerprint != fingerprint(scene_text) {
        tracing::warn!(
            "lightmaps: {scene_path} has changed since it was baked; its lightmaps show it as \
             it was until it is baked again"
        );
    }
    let mut by_name = std::collections::HashMap::new();
    for entity in world.iter_entities() {
        if let Some(name) = entity.get::<crate::Name>() {
            by_name.insert(name.0.clone(), entity.id());
        }
    }
    let mut applied = 0;
    for entry in &data.entries {
        match by_name.get(&entry.entity) {
            Some(&e) => {
                world.entity_mut(e).insert(bsengine_core::Lightmap {
                    texture_path: entry.texture.clone(),
                    scale_offset: entry.scale_offset,
                });
                applied += 1;
            }
            None => tracing::warn!(
                "lightmaps: {path} names '{}', which {scene_path} no longer has; bake again",
                entry.entity
            ),
        }
    }
    applied
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_data_lives_beside_the_scene_with_its_own_extension() {
        assert_eq!(
            data_path("assets/scenes/main.ron"),
            "assets/scenes/main.lightmaps"
        );
        assert_eq!(data_path("level"), "level.lightmaps");
    }

    #[test]
    fn the_fingerprint_ignores_line_endings_and_sees_edits() {
        assert_eq!(fingerprint("a\r\nb"), fingerprint("a\nb"));
        assert_ne!(fingerprint("a\nb"), fingerprint("a\nc"));
    }

    /// Named entities get their lightmap -- texture and rectangle as the data
    /// says -- and an entity the data does not name gets none.
    #[test]
    fn named_entities_get_their_lightmap() {
        let dir = std::env::temp_dir().join(format!("bsengine_lightmaps_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let scene = dir.join("level.ron");
        let scene_path = scene.to_string_lossy().replace('\\', "/");
        let scene_text = "SceneDescriptor(entities: [])";
        let data = LightmapData {
            scene_fingerprint: fingerprint(scene_text),
            entries: vec![LightmapEntry {
                entity: "Floor".into(),
                texture: "assets/lightmaps/level/lightmap_0.png".into(),
                scale_offset: [0.5, 0.5, 0.25, 0.0],
            }],
        };
        std::fs::write(
            data_path(&scene_path),
            ron::ser::to_string_pretty(&data, Default::default()).unwrap(),
        )
        .unwrap();

        let mut world = bevy_ecs::world::World::new();
        let floor = world.spawn(crate::Name("Floor".into())).id();
        let wall = world.spawn(crate::Name("Wall".into())).id();
        assert_eq!(apply(&mut world, &scene_path, scene_text), 1);
        let lm = world
            .get::<bsengine_core::Lightmap>(floor)
            .expect("Floor has one");
        assert_eq!(lm.texture_path, "assets/lightmaps/level/lightmap_0.png");
        assert_eq!(lm.scale_offset, [0.5, 0.5, 0.25, 0.0]);
        assert!(world.get::<bsengine_core::Lightmap>(wall).is_none());

        // A scene that was never baked: nothing, and no error.
        let mut other = bevy_ecs::world::World::new();
        other.spawn(crate::Name("Floor".into()));
        assert_eq!(apply(&mut other, "nowhere/unbaked.ron", scene_text), 0);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
