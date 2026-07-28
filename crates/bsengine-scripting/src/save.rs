use std::collections::HashMap;
use std::fs;
use std::path::Path;

use bevy_ecs::prelude::{Entity, World};
use bsengine_core::{Name, SaveData, Transform};
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// Root JSON structure of a save-game file: a format version plus the
/// serialized state of every named, transform-bearing entity.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GameSaveFile {
    /// Save-file format version, for future migration compatibility.
    pub version: String,
    /// Serialized entities captured at save time.
    pub entities: Vec<EntitySave>,
}

/// Serialized state of a single entity: its identifying name, transform,
/// and any script-defined `SaveData` fields.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EntitySave {
    /// The entity's `Name` component value, used to match it on load.
    pub name: String,
    /// The entity's position, rotation, and scale at save time.
    pub transform: TransformSave,
    /// Script-defined key/value fields from the entity's `SaveData`, stored as UTF-8 strings.
    #[serde(default)]
    pub fields: HashMap<String, String>,
}

/// Flat, JSON-friendly encoding of a `Transform` (translation, quaternion
/// rotation, and scale) for save files.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct TransformSave {
    /// Translation X component.
    pub x: f32,
    /// Translation Y component.
    pub y: f32,
    /// Translation Z component.
    pub z: f32,
    /// Rotation quaternion X component.
    pub rx: f32,
    /// Rotation quaternion Y component.
    pub ry: f32,
    /// Rotation quaternion Z component.
    pub rz: f32,
    /// Rotation quaternion W component.
    pub rw: f32,
    /// Scale X component.
    pub sx: f32,
    /// Scale Y component.
    pub sy: f32,
    /// Scale Z component.
    pub sz: f32,
}

/// Serialize all named entities (with Transform) to a JSON file.
/// Entities without Transform are skipped. SaveData fields are included when present.
pub fn save_world(world: &mut World, path: &str) -> Result<(), String> {
    let mut q = world.query::<(&Name, &Transform, Option<&SaveData>)>();
    let entity_saves: Vec<EntitySave> = q
        .iter(world)
        .map(|(name, transform, save_data)| {
            let fields: HashMap<String, String> = save_data
                .map(|sd| {
                    sd.fields
                        .iter()
                        .filter_map(|(k, v)| {
                            String::from_utf8(v.clone()).ok().map(|s| (k.clone(), s))
                        })
                        .collect()
                })
                .unwrap_or_default();
            EntitySave {
                name: name.0.clone(),
                transform: TransformSave {
                    x: transform.translation.x,
                    y: transform.translation.y,
                    z: transform.translation.z,
                    rx: transform.rotation.x,
                    ry: transform.rotation.y,
                    rz: transform.rotation.z,
                    rw: transform.rotation.w,
                    sx: transform.scale.x,
                    sy: transform.scale.y,
                    sz: transform.scale.z,
                },
                fields,
            }
        })
        .collect();

    let save_file = GameSaveFile {
        version: "0.1.0".to_string(),
        entities: entity_saves,
    };

    let json = serde_json::to_string_pretty(&save_file).map_err(|e| format!("serialize: {e}"))?;

    if let Some(parent) = Path::new(path).parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
        }
    }

    fs::write(path, json).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

/// Load entities from a JSON save file. Existing entities (matched by name) have
/// their Transform and SaveData updated. Unknown entities are spawned fresh.
pub fn load_world(world: &mut World, path: &str) -> Result<(), String> {
    let json = fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let save_file: GameSaveFile =
        serde_json::from_str(&json).map_err(|e| format!("deserialize: {e}"))?;

    for es in &save_file.entities {
        let entity: Option<Entity> = {
            let mut q = world.query::<(Entity, &Name)>();
            q.iter(world).find(|(_, n)| n.0 == es.name).map(|(e, _)| e)
        };

        let ts = &es.transform;

        if let Some(entity) = entity {
            if let Some(mut t) = world.get_mut::<Transform>(entity) {
                t.translation = Vec3::new(ts.x, ts.y, ts.z).into();
                t.rotation = Quat::from_xyzw(ts.rx, ts.ry, ts.rz, ts.rw).into();
                t.scale = Vec3::new(ts.sx, ts.sy, ts.sz).into();
            }
            if !es.fields.is_empty() {
                if let Some(mut sd) = world.get_mut::<SaveData>(entity) {
                    for (k, v) in &es.fields {
                        sd.set(k.clone(), v.clone().into_bytes());
                    }
                }
            }
        } else {
            let mut spawned = world.spawn((
                Name(es.name.clone()),
                Transform {
                    translation: Vec3::new(ts.x, ts.y, ts.z).into(),
                    rotation: Quat::from_xyzw(ts.rx, ts.ry, ts.rz, ts.rw).into(),
                    scale: Vec3::new(ts.sx, ts.sy, ts.sz).into(),
                },
            ));
            if !es.fields.is_empty() {
                let mut sd = SaveData::new(0);
                for (k, v) in &es.fields {
                    sd.set(k.clone(), v.clone().into_bytes());
                }
                spawned.insert(sd);
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Name, SaveData, Transform};
    use glam::Vec3;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("bsengine_save_test_{name}.json"))
    }

    #[test]
    fn save_round_trips_transform() {
        let mut world = World::new();
        world.spawn((
            Name("player".to_string()),
            Transform::from_translation(Vec3::new(1.0, 2.0, 3.0)),
        ));

        let path = temp_path("round_trip");
        save_world(&mut world, path.to_str().unwrap()).unwrap();

        let mut world2 = World::new();
        load_world(&mut world2, path.to_str().unwrap()).unwrap();

        let mut q = world2.query::<(&Name, &Transform)>();
        let (name, t) = q
            .iter(&world2)
            .next()
            .expect("entity must exist after load");
        assert_eq!(name.0, "player");
        assert!((t.translation.x - 1.0).abs() < 1e-5);
        assert!((t.translation.y - 2.0).abs() < 1e-5);
        assert!((t.translation.z - 3.0).abs() < 1e-5);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_skips_entities_without_transform() {
        let mut world = World::new();
        world.spawn(Name("nameless".to_string()));
        world.spawn((
            Name("located".to_string()),
            Transform::from_translation(Vec3::X),
        ));

        let path = temp_path("skip_notransform");
        save_world(&mut world, path.to_str().unwrap()).unwrap();

        let json = fs::read_to_string(&path).unwrap();
        let sf: GameSaveFile = serde_json::from_str(&json).unwrap();
        assert_eq!(sf.entities.len(), 1);
        assert_eq!(sf.entities[0].name, "located");

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_updates_existing_entity() {
        let mut world = World::new();
        let entity = world
            .spawn((
                Name("hero".to_string()),
                Transform::from_translation(Vec3::ZERO),
            ))
            .id();

        let path = temp_path("update_existing");
        let save_file = GameSaveFile {
            version: "0.1.0".to_string(),
            entities: vec![EntitySave {
                name: "hero".to_string(),
                transform: TransformSave {
                    x: 5.0,
                    y: 0.0,
                    z: 0.0,
                    rx: 0.0,
                    ry: 0.0,
                    rz: 0.0,
                    rw: 1.0,
                    sx: 1.0,
                    sy: 1.0,
                    sz: 1.0,
                },
                fields: HashMap::new(),
            }],
        };
        fs::write(&path, serde_json::to_string_pretty(&save_file).unwrap()).unwrap();

        load_world(&mut world, path.to_str().unwrap()).unwrap();

        let t = world.get::<Transform>(entity).unwrap();
        assert!(
            (t.translation.x - 5.0).abs() < 1e-5,
            "x should be updated to 5"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_spawns_missing_entity() {
        let mut world = World::new();

        let path = temp_path("spawn_missing");
        let save_file = GameSaveFile {
            version: "0.1.0".to_string(),
            entities: vec![EntitySave {
                name: "new_entity".to_string(),
                transform: TransformSave {
                    x: 0.0,
                    y: 10.0,
                    z: 0.0,
                    rx: 0.0,
                    ry: 0.0,
                    rz: 0.0,
                    rw: 1.0,
                    sx: 1.0,
                    sy: 1.0,
                    sz: 1.0,
                },
                fields: HashMap::new(),
            }],
        };
        fs::write(&path, serde_json::to_string_pretty(&save_file).unwrap()).unwrap();

        load_world(&mut world, path.to_str().unwrap()).unwrap();

        let mut q = world.query::<(&Name, &Transform)>();
        let (name, t) = q.iter(&world).next().expect("entity must be spawned");
        assert_eq!(name.0, "new_entity");
        assert!((t.translation.y - 10.0).abs() < 1e-5);

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn save_includes_save_data_fields() {
        let mut world = World::new();
        let mut sd = SaveData::new(0);
        sd.set("score", b"42".to_vec());
        world.spawn((Name("player".to_string()), Transform::default(), sd));

        let path = temp_path("fields_save");
        save_world(&mut world, path.to_str().unwrap()).unwrap();

        let json = fs::read_to_string(&path).unwrap();
        let sf: GameSaveFile = serde_json::from_str(&json).unwrap();
        assert_eq!(
            sf.entities[0].fields.get("score").map(|s| s.as_str()),
            Some("42")
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn load_restores_save_data_fields() {
        let mut world = World::new();
        world.spawn((
            Name("player".to_string()),
            Transform::default(),
            SaveData::new(0),
        ));

        let path = temp_path("fields_load");
        let mut fields = HashMap::new();
        fields.insert("lives".to_string(), "3".to_string());
        let save_file = GameSaveFile {
            version: "0.1.0".to_string(),
            entities: vec![EntitySave {
                name: "player".to_string(),
                transform: TransformSave {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                    rx: 0.0,
                    ry: 0.0,
                    rz: 0.0,
                    rw: 1.0,
                    sx: 1.0,
                    sy: 1.0,
                    sz: 1.0,
                },
                fields,
            }],
        };
        fs::write(&path, serde_json::to_string_pretty(&save_file).unwrap()).unwrap();

        load_world(&mut world, path.to_str().unwrap()).unwrap();

        let mut q = world.query::<(&Name, &SaveData)>();
        let (_, sd) = q
            .iter(&world)
            .next()
            .expect("entity with SaveData must exist");
        assert_eq!(
            sd.get("lives"),
            Some(b"3".as_ref()),
            "lives field must be restored"
        );

        let _ = fs::remove_file(&path);
    }
}
