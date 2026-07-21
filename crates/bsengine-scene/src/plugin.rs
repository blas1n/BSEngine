use crate::types::{EntityDescriptor, PhysicsBodyDesc, PrimitiveMesh, SceneDescriptor, ScriptPath};
use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Component, World};
use bsengine_core::{
    Camera, DirectionalLight, GlobalTransform, Material, PointLight, SkyboxPath, SpotLight,
    Transform,
};
use bsengine_gltf::GltfAsset;
use glam::{Quat, Vec3};

#[derive(Component, Debug, Clone)]
pub struct Name(pub String);

pub struct ScenePlugin {
    path: String,
}

impl ScenePlugin {
    pub fn from_file(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
}

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();
        app.add_systems(Startup, move |world: &mut World| {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read scene {path}: {e}"));
            let scene: SceneDescriptor = ron::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse scene {path}: {e}"));
            spawn_scene_entities(world, &scene.entities);
            if let Some(skybox_rel) = &scene.skybox {
                let scene_dir = std::path::Path::new(&path)
                    .parent()
                    .unwrap_or(std::path::Path::new("."));
                let skybox_full = scene_dir.join(skybox_rel).to_string_lossy().into_owned();
                world.insert_resource(SkyboxPath(Some(skybox_full)));
            }
        });
    }
}

/// Spawn entities from a list of descriptors into the given world.
/// Called at startup by ScenePlugin and at runtime for scene transitions.
pub fn spawn_scene_entities(world: &mut World, entities: &[EntityDescriptor]) {
    for entity in entities {
        let mut builder = world.spawn(Name(entity.name.clone()));

        if let Some(t) = &entity.transform {
            let mut rotation =
                Quat::from_xyzw(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]);
            if entity.camera {
                if let Some(target) = entity.look_at {
                    let pos = Vec3::from(t.translation);
                    let dir = Vec3::from(target) - pos;
                    if dir.length_squared() > 1e-10 {
                        rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize());
                    }
                }
            }
            let transform = Transform {
                translation: Vec3::from(t.translation).into(),
                rotation: rotation.into(),
                scale: Vec3::from(t.scale).into(),
            };
            builder.insert((transform, GlobalTransform::default()));
        }

        if let Some(path) = &entity.gltf {
            builder.insert(GltfAsset::new(path.clone()));
        }

        if entity.camera {
            match entity.camera_fov {
                Some(fov) => {
                    builder.insert(Camera::perspective(fov, 16.0 / 9.0));
                }
                None => {
                    builder.insert(Camera::default());
                }
            }
        }

        if let Some(dl) = &entity.directional_light {
            builder.insert(DirectionalLight {
                color: Vec3::from(dl.color).into(),
                ambient: Vec3::from(dl.ambient).into(),
            });
            // Direction lives on Transform.rotation (rotation * -Z), same as
            // SpotLight; reuse any explicit translation/scale from the scene
            // file's own `transform:` block if one was given.
            let dir = Vec3::from(dl.direction).normalize_or(Vec3::NEG_Z);
            let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
            let (translation, scale) = entity
                .transform
                .as_ref()
                .map(|t| (Vec3::from(t.translation), Vec3::from(t.scale)))
                .unwrap_or((Vec3::ZERO, Vec3::ONE));
            builder.insert((
                Transform {
                    translation: translation.into(),
                    rotation: rotation.into(),
                    scale: scale.into(),
                },
                GlobalTransform::default(),
            ));
        }

        if let Some(pl) = &entity.point_light {
            builder.insert(PointLight {
                color: Vec3::from(pl.color).into(),
                intensity: pl.intensity,
                range: pl.range,
            });
        }

        if let Some(sl) = &entity.spot_light {
            builder.insert(SpotLight {
                color: Vec3::from(sl.color).into(),
                intensity: sl.intensity,
                range: sl.range,
                inner_angle_degrees: sl.inner_angle_degrees.into(),
                outer_angle_degrees: sl.outer_angle_degrees.into(),
            });
        }

        if let Some(prim) = &entity.primitive {
            builder.insert(PrimitiveMesh(prim.clone()));
        }

        if let Some(script) = &entity.script {
            builder.insert(ScriptPath(script.clone()));
        }

        if entity.emissive.is_some() || entity.color.is_some() {
            builder.insert(Material {
                emissive: entity.emissive.map(Vec3::from).unwrap_or(Vec3::ZERO).into(),
                base_color: entity.color.map(Vec3::from).unwrap_or(Vec3::ONE).into(),
                ..Default::default()
            });
        }

        if let (Some(rb), Some(col)) = (&entity.rigidbody, &entity.collider) {
            builder.insert(PhysicsBodyDesc {
                rigidbody: rb.clone(),
                collider: col.clone(),
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Name, ScenePlugin};
    use bsengine_app::new_app;
    use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Transform};
    use glam::Vec3;

    fn write_temp_scene(filename: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(filename);
        std::fs::write(&path, content).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn scene_plugin_spawns_entities() {
        let ron = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Player", components: []), EntityDescriptor(name: "Camera", components: [])])"#;
        let path = write_temp_scene("test_spawn.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert!(
            names.contains(&"Player".to_string()),
            "Player missing: {:?}",
            names
        );
        assert!(
            names.contains(&"Camera".to_string()),
            "Camera missing: {:?}",
            names
        );
    }

    #[test]
    fn scene_plugin_empty_scene() {
        let ron = r#"SceneDescriptor(entities: [])"#;
        let path = write_temp_scene("test_empty.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        assert_eq!(q.iter(app.world()).count(), 0);
    }

    #[test]
    fn scene_plugin_spawns_transform() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Cube",
                transform: Some((translation: (1.0, 2.0, 3.0))),
            )
        ])"#;
        let path = write_temp_scene("test_transform.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &Transform, &GlobalTransform)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        let (name, t, _) = &results[0];
        assert_eq!(name.0, "Cube");
        assert!((t.translation.x - 1.0).abs() < 1e-5);
        assert!((t.translation.y - 2.0).abs() < 1e-5);
        assert!((t.translation.z - 3.0).abs() < 1e-5);
    }

    #[test]
    fn scene_plugin_spawns_camera() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "MainCam", camera: true)
        ])"#;
        let path = write_temp_scene("test_camera.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<(&Name, &Camera)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "MainCam");
    }

    #[test]
    fn scene_plugin_spawns_directional_light() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Sun",
                directional_light: Some((direction: (0.0, -1.0, 0.0))),
            )
        ])"#;
        let path = write_temp_scene("test_light.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &DirectionalLight, &Transform)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        let (name, _light, transform) = &results[0];
        assert_eq!(name.0, "Sun");
        let derived_dir = transform.rotation.0 * Vec3::NEG_Z;
        assert!((derived_dir.y - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn scene_plugin_no_transform_when_not_specified() {
        let ron = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Ghost")])"#;
        let path = write_temp_scene("test_no_transform.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<(&Name, &Transform)>();
        assert_eq!(
            q.iter(app.world()).count(),
            0,
            "entity without transform field should have no Transform component"
        );
    }
}
