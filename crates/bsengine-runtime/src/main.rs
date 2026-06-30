use std::env;

use bevy_app::{PostStartup, Update};
use bevy_ecs::prelude::{IntoSystemConfigs, World};
use bsengine_app::new_app;
use bsengine_audio::AudioPlugin;
use bsengine_core::{HudTexts, Transform};
use bsengine_ecs::{Added, Commands, Entity, Query, ResMut};
use bsengine_input::InputPlugin;
use bsengine_physics::{Collider, PhysicsInput, PhysicsPlugin, RigidBody};
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{
    capsule_vertices, cube_vertices, plane_vertices, sphere_vertices, GpuMeshRegistry,
    WgpuRHIPlugin,
};
use bsengine_scene::{
    spawn_scene_entities, ColliderShapeDesc, Name, PendingSceneLoad, PhysicsBodyDesc, Primitive,
    PrimitiveMesh, RigidBodyDesc, SceneDescriptor, ScenePlugin,
};
use bsengine_scripting::{
    load_scripts, ScriptRuntime, ScriptRuntimeResource, ScriptingPlugin, SoundHandles,
};
use bsengine_window::{WindowDescriptor, WindowPlugin};
use serde::Deserialize;

#[derive(Deserialize)]
struct ProjectManifest {
    project: ProjectSection,
    #[serde(default)]
    window: WindowSection,
}

#[derive(Deserialize)]
struct ProjectSection {
    name: String,
    entry_scene: String,
}

#[derive(Deserialize, Default)]
struct WindowSection {
    #[serde(default)]
    title: Option<String>,
    #[serde(default = "default_width")]
    width: u32,
    #[serde(default = "default_height")]
    height: u32,
    #[serde(default = "default_true")]
    resizable: bool,
}

fn default_width() -> u32 {
    1280
}
fn default_height() -> u32 {
    720
}
fn default_true() -> bool {
    true
}

fn main() {
    let project_dir = env::args().nth(1).unwrap_or_else(|| ".".to_string());
    let manifest_path = format!("{}/project.toml", project_dir);

    let manifest_str = std::fs::read_to_string(&manifest_path)
        .unwrap_or_else(|e| panic!("Cannot read {manifest_path}: {e}"));
    let manifest: ProjectManifest = toml::from_str(&manifest_str)
        .unwrap_or_else(|e| panic!("Cannot parse {manifest_path}: {e}"));

    let scene_path = format!("{}/{}", project_dir, manifest.project.entry_scene);
    let title = manifest
        .window
        .title
        .unwrap_or_else(|| manifest.project.name.clone());

    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title,
                width: manifest.window.width,
                height: manifest.window.height,
                resizable: manifest.window.resizable,
            },
        })
        .add_plugins(InputPlugin)
        .add_plugins(AudioPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(RenderPlugin)
        .add_plugins(ScenePlugin::from_file(&scene_path))
        .add_plugins(ScriptingPlugin {
            project_dir: project_dir.clone(),
        })
        // PostStartup: resolve primitive meshes, then physics bodies
        .add_systems(PostStartup, (resolve_primitives, resolve_physics_bodies).chain())
        // Update: handle scene transitions first, then re-resolve newly spawned entities
        .add_systems(Update, handle_scene_load)
        .add_systems(Update, resolve_primitives.after(handle_scene_load))
        .add_systems(Update, resolve_physics_bodies.after(resolve_primitives))
        .run();
}

fn resolve_primitives(
    query: Query<(Entity, &PrimitiveMesh), Added<PrimitiveMesh>>,
    mut commands: Commands,
    registry: Option<ResMut<GpuMeshRegistry>>,
) {
    let Some(mut registry) = registry else { return };

    let mut cube_id: Option<u64> = None;
    let mut sphere_id: Option<u64> = None;
    let mut plane_id: Option<u64> = None;
    let mut capsule_id: Option<u64> = None;

    for (entity, prim) in query.iter() {
        let mesh_id = match &prim.0 {
            Primitive::Cube => *cube_id.get_or_insert_with(|| {
                let (v, i) = cube_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Sphere => *sphere_id.get_or_insert_with(|| {
                let (v, i) = sphere_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Plane => *plane_id.get_or_insert_with(|| {
                let (v, i) = plane_vertices();
                registry.register(&v, &i)
            }),
            Primitive::Capsule => *capsule_id.get_or_insert_with(|| {
                let (v, i) = capsule_vertices();
                registry.register(&v, &i)
            }),
        };
        commands.entity(entity).insert(MeshRenderer { mesh_id });
    }
}

fn resolve_physics_bodies(
    query: Query<(Entity, &PhysicsBodyDesc), Added<PhysicsBodyDesc>>,
    transforms: Query<&Transform>,
    mut commands: Commands,
) {
    for (entity, desc) in query.iter() {
        let rb = match desc.rigidbody {
            RigidBodyDesc::Dynamic => RigidBody::dynamic(),
            RigidBodyDesc::Static => RigidBody::fixed(),
            RigidBodyDesc::Kinematic => RigidBody::kinematic(),
        };
        let col_base = match &desc.collider.shape {
            ColliderShapeDesc::Box { hx, hy, hz } => Collider::cuboid(*hx, *hy, *hz),
            ColliderShapeDesc::Sphere { radius } => Collider::ball(*radius),
            ColliderShapeDesc::Capsule { half_height, radius } => {
                Collider::capsule(*half_height, *radius)
            }
        };
        let col = col_base
            .with_restitution(desc.collider.restitution)
            .with_friction(desc.collider.friction)
            .with_sensor(desc.collider.sensor);
        let t = transforms.get(entity).cloned().unwrap_or_default();
        commands.entity(entity).insert((
            rb,
            col,
            PhysicsInput {
                translation: t.translation,
                rotation: t.rotation,
            },
        ));
    }
}

fn handle_scene_load(world: &mut World) {
    let pending = world.remove_resource::<PendingSceneLoad>();
    let Some(pending) = pending else { return };

    let content = match std::fs::read_to_string(&pending.path) {
        Ok(c) => c,
        Err(e) => {
            tracing::error!("[scene] failed to read {}: {e}", pending.path);
            return;
        }
    };
    let scene: SceneDescriptor = match ron::from_str(&content) {
        Ok(s) => s,
        Err(e) => {
            tracing::error!("[scene] failed to parse {}: {e}", pending.path);
            return;
        }
    };

    // Stop all sounds and clear handles
    if let Some(mut handles) = world.get_resource_mut::<SoundHandles>() {
        for (_, mut handle) in handles.0.drain() {
            let _ = handle.stop(kira::Tween::default());
        }
    }

    // Despawn all named entities
    let named: Vec<Entity> = {
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world).map(|(e, _)| e).collect()
    };
    for e in named {
        world.despawn(e);
    }

    // Clear HUD
    if let Some(mut hud) = world.get_resource_mut::<HudTexts>() {
        hud.0.clear();
    }

    // Reset script runtime
    world.insert_non_send_resource(ScriptRuntimeResource(ScriptRuntime::new_with_ops()));

    // Spawn scene and resolve physics inline (Added<> won't fire for same-frame spawns)
    spawn_scene_entities(world, &scene.entities);
    resolve_physics_bodies_world(world);
    load_scripts(world);
}

fn resolve_physics_bodies_world(world: &mut World) {
    let entities: Vec<(Entity, PhysicsBodyDesc)> = {
        let mut q = world.query::<(Entity, &PhysicsBodyDesc)>();
        q.iter(world).map(|(e, d)| (e, d.clone())).collect()
    };
    for (entity, desc) in entities {
        let rb = match desc.rigidbody {
            RigidBodyDesc::Dynamic => RigidBody::dynamic(),
            RigidBodyDesc::Static => RigidBody::fixed(),
            RigidBodyDesc::Kinematic => RigidBody::kinematic(),
        };
        let col_base = match desc.collider.shape {
            ColliderShapeDesc::Box { hx, hy, hz } => Collider::cuboid(hx, hy, hz),
            ColliderShapeDesc::Sphere { radius } => Collider::ball(radius),
            ColliderShapeDesc::Capsule { half_height, radius } => {
                Collider::capsule(half_height, radius)
            }
        };
        let col = col_base
            .with_restitution(desc.collider.restitution)
            .with_friction(desc.collider.friction)
            .with_sensor(desc.collider.sensor);
        let t = world
            .get::<Transform>(entity)
            .cloned()
            .unwrap_or_default();
        world.entity_mut(entity).insert((
            rb,
            col,
            PhysicsInput {
                translation: t.translation,
                rotation: t.rotation,
            },
        ));
    }
}
