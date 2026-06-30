use crate::types::{PrimitiveMesh, SceneDescriptor, ScriptPath};
use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Commands, Component};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Material, Transform};
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
        app.add_systems(Startup, move |mut commands: Commands| {
            let content = std::fs::read_to_string(&path)
                .unwrap_or_else(|e| panic!("Failed to read scene {path}: {e}"));
            let scene: SceneDescriptor = ron::from_str(&content)
                .unwrap_or_else(|e| panic!("Failed to parse scene {path}: {e}"));

            for entity in &scene.entities {
                let mut builder = commands.spawn(Name(entity.name.clone()));

                if let Some(t) = &entity.transform {
                    let transform = Transform {
                        translation: Vec3::from(t.translation),
                        rotation: Quat::from_xyzw(
                            t.rotation[0],
                            t.rotation[1],
                            t.rotation[2],
                            t.rotation[3],
                        ),
                        scale: Vec3::from(t.scale),
                    };
                    builder.insert((transform, GlobalTransform::default()));
                }

                if let Some(path) = &entity.gltf {
                    builder.insert(GltfAsset::new(path.clone()));
                }

                if entity.camera {
                    builder.insert(Camera::default());
                }

                if let Some(dl) = &entity.directional_light {
                    builder.insert(DirectionalLight {
                        direction: Vec3::from(dl.direction),
                        color: Vec3::from(dl.color),
                        ambient: Vec3::from(dl.ambient),
                    });
                }

                if let Some(prim) = &entity.primitive {
                    builder.insert(PrimitiveMesh(prim.clone()));
                }

                if let Some(script) = &entity.script {
                    builder.insert(ScriptPath(script.clone()));
                }

                if let Some(emissive) = &entity.emissive {
                    builder.insert(Material {
                        emissive: Vec3::from(*emissive),
                        ..Default::default()
                    });
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{Name, ScenePlugin};
    use bsengine_app::new_app;
    use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Transform};

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

        let mut q = app.world_mut().query::<(&Name, &DirectionalLight)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        let (name, light) = &results[0];
        assert_eq!(name.0, "Sun");
        assert!((light.direction.y - (-1.0)).abs() < 1e-5);
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
