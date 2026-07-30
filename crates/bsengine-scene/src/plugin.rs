use crate::types::{EntityDescriptor, PhysicsBodyDesc, PrimitiveMesh, SceneDescriptor, ScriptPath};
use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Component, World};
use bsengine_core::{
    Camera, DirectionalLight, GlobalTransform, Material, PointLight, SkyboxPath, SpotLight,
    Transform,
};
use bsengine_gltf::GltfAsset;
use glam::{Quat, Vec3};

/// Human-readable name assigned to a spawned scene entity, taken from `EntityDescriptor::name`.
#[derive(Component, Debug, Clone)]
pub struct Name(pub String);

/// Bevy plugin that loads a scene file at startup and spawns its entities into the world.
pub struct ScenePlugin {
    path: String,
}

impl ScenePlugin {
    /// Creates a plugin that will load and spawn the scene at `path` when added to an `App`.
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
    // Cloned once up front (cheap Arc clone) rather than re-fetched per
    // entity, since `world.spawn(..)` below holds an exclusive sub-borrow of
    // `world` for the rest of the loop body and a `Res`/`world.resource()`
    // call can't be interleaved with it.
    let app_registry = world
        .get_resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .cloned();
    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();

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
            builder.insert(GltfAsset::new(bsengine_core::resolve_project_path(
                project_dir.as_ref(),
                path,
            )));
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

        if !entity.components.is_empty() {
            if let Some(app_registry) = &app_registry {
                let registry = app_registry.read();
                for (type_path, value_ron) in &entity.components {
                    let Some(registration) = registry.get_with_type_path(type_path) else {
                        tracing::warn!(
                            "scene: entity '{}' references unknown reflected type path '{type_path}'",
                            entity.name
                        );
                        continue;
                    };
                    let Some(reflect_component) =
                        registration.data::<bevy_ecs::reflect::ReflectComponent>()
                    else {
                        tracing::warn!(
                            "scene: entity '{}' type path '{type_path}' is not a registered Component",
                            entity.name
                        );
                        continue;
                    };
                    let de =
                        bevy_reflect::serde::TypedReflectDeserializer::new(registration, &registry);
                    match ron::de::Deserializer::from_str(value_ron) {
                        Ok(mut deserializer) => {
                            match serde::de::DeserializeSeed::deserialize(de, &mut deserializer) {
                                Ok(value) => {
                                    reflect_component.apply_or_insert(
                                        &mut builder,
                                        value.as_ref(),
                                        &registry,
                                    );
                                }
                                Err(e) => tracing::warn!(
                                    "scene: entity '{}' component '{type_path}' RON value doesn't match its shape: {e}",
                                    entity.name
                                ),
                            }
                        }
                        Err(e) => tracing::warn!(
                            "scene: entity '{}' component '{type_path}' RON parse error: {e}",
                            entity.name
                        ),
                    }
                }
            }
        }
    }
}

/// Registers every `bsengine_core` component type an `EntityDescriptor`'s
/// `components:` field (arbitrary reflected components, e.g. `Shield`,
/// `SaveData`, `AnimationStateMachine`, `NavMeshAgent`, `Bloom`, `ToneMap`)
/// may reference, so [`spawn_scene_entities`]'s reflection-based
/// deserialization above can actually find and attach them.
///
/// Previously these registrations lived only inline in `EditorPlugin::build`
/// (`bsengine-editor`), which the headless test runtime
/// (`bsengine-runtime --test`'s `build_test_app`) never adds -- it can't,
/// since `EditorPlugin` requires the render/window stack a headless app
/// doesn't have. That meant every reflected `components:` entry was silently
/// dropped in headless test mode (logged only as a `tracing::warn!`
/// "unknown reflected type path", easy to miss), so any gameplay gated on
/// one of these components -- e.g. a `Shield`-based death check -- was
/// unexercisable via `bsengine-runtime --test`, even though the identical
/// scene loaded and played correctly in the windowed editor runtime. Call
/// this from both `EditorPlugin::build` and the headless test app's setup so
/// the two stay in parity.
pub fn register_gameplay_reflect_types(app: &mut bevy_app::App) {
    app.register_type::<bsengine_core::Camera>();
    app.register_type::<bsengine_core::PointLight>();
    app.register_type::<bsengine_core::DirectionalLight>();
    app.register_type::<bsengine_core::SpotLight>();
    app.register_type::<bsengine_core::Material>();
    app.register_type::<bsengine_core::AmbientOcclusion>();
    app.register_type::<bsengine_core::AnimationPlayer>();
    app.register_type::<bsengine_core::Bloom>();
    app.register_type::<bsengine_core::CustomShader>();
    app.register_type::<bsengine_core::Damping>();
    app.register_type::<bsengine_core::GravityScale>();
    app.register_type::<bsengine_core::Lifetime>();
    app.register_type::<bsengine_core::Mass>();
    app.register_type::<bsengine_core::NetworkId>();
    app.register_type::<bsengine_core::SaveData>();
    app.register_type::<bsengine_core::Shield>();
    app.register_type::<bsengine_core::Skybox>();
    app.register_type::<bsengine_core::Timer>();
    app.register_type::<bsengine_core::ToneMap>();
    app.register_type::<bsengine_core::Visible>();
    app.register_type::<bsengine_core::AngularVelocity>();
    app.register_type::<bsengine_core::ExternalImpulse>();
    app.register_type::<bsengine_core::Follow>();
    app.register_type::<bsengine_core::LookAt>();
    app.register_type::<bsengine_core::NavMeshAgent>();
    app.register_type::<bsengine_core::Velocity>();
    app.register_type::<bsengine_core::Transform>();
    app.register_type::<bsengine_core::GlobalTransform>();
    app.register_type::<bsengine_core::Parent>();
    app.register_type::<bsengine_core::AnimationStateMachine>();
    // AnimationStateMachine::triggers is a HashSet<String>; unlike Map/List/Struct
    // fields, HashSet isn't structurally recursed by TypedReflectDeserializer and
    // needs its value-kind ReflectDeserialize registered explicitly, or JSON/RON
    // authoring fails with "doesn't have ReflectDeserialize" even for an empty
    // `[]`. Same story for ReflectSerialize on the save side. See
    // `bsengine-editor`'s prior inline copy of this registration for the fuller
    // history (design-research probe that found this).
    app.register_type_data::<std::collections::HashSet<String>, bevy_reflect::ReflectDeserialize>();
    app.register_type_data::<std::collections::HashSet<String>, bevy_reflect::ReflectSerialize>();
    app.register_type::<bsengine_core::Tween>();
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
    fn scene_plugin_applies_reflected_component_from_ron_value() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Enemy",
                components: [
                    ("bsengine_core::nav_mesh_agent::NavMeshAgent", "(destination: None, speed: 3.5, angular_speed: 2.0, acceleration: 8.0, stopping_distance: 0.1, radius: 0.3, height: 1.8, state: Idle, enabled: true)"),
                ],
            )
        ])"#;
        let path = write_temp_scene("test_reflected_component.ron", ron);

        let mut app = new_app();
        app.register_type::<bsengine_core::NavMeshAgent>();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::NavMeshAgent)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "Enemy");
        assert!((results[0].1.speed - 3.5).abs() < 1e-5);
        assert!(results[0].1.enabled);
    }

    #[test]
    fn scene_plugin_unknown_reflected_type_path_is_skipped_not_fatal() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Ghost",
                components: [("not::a::real::Type", "()")],
            )
        ])"#;
        let path = write_temp_scene("test_unknown_type.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert!(names.contains(&"Ghost".to_string()));
    }

    #[test]
    fn scene_plugin_gltf_path_resolves_against_project_dir() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Player", gltf: Some("models/hero.glb")),
        ])"#;
        let path = write_temp_scene("test_gltf_project_dir.ron", ron);

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "Player");
        assert_eq!(results[0].1.path, "games/demo/models/hero.glb");
    }

    #[test]
    fn scene_plugin_gltf_path_unchanged_without_project_dir() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Player", gltf: Some("models/hero.glb")),
        ])"#;
        let path = write_temp_scene("test_gltf_no_project_dir.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.path, "models/hero.glb");
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
