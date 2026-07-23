//! Scene/project loading systems shared by the real-time runtime (`main.rs`)
//! and the headless test runtime (`test_mode.rs`), so both run identical
//! scene-load and physics-resolution behavior.

use bevy_app::{App, PostStartup, Update};
use bevy_ecs::prelude::{IntoSystemConfigs, World};
use bsengine_core::{HudTexts, Transform};
use bsengine_ecs::{Added, Commands, Entity, Query, ResMut};
use bsengine_physics::{Collider, PhysicsInput, RigidBody};
use bsengine_rhi_wgpu::{
    capsule_vertices, cube_vertices, plane_vertices, sphere_vertices, GpuMeshRegistry,
};
use bsengine_scene::{
    spawn_scene_entities, ColliderShapeDesc, Name, PendingSceneLoad, PhysicsBodyDesc, Primitive,
    PrimitiveMesh, RigidBodyDesc, SceneDescriptor,
};
use bsengine_scripting::{load_scripts, SoundHandles};
use serde::Deserialize;

#[derive(Deserialize)]
pub struct ProjectManifest {
    pub project: ProjectSection,
    #[serde(default)]
    pub window: WindowSection,
}

#[derive(Deserialize)]
pub struct ProjectSection {
    pub name: String,
    pub entry_scene: String,
}

#[derive(Deserialize, Default)]
pub struct WindowSection {
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_true")]
    pub resizable: bool,
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

/// Registers the scene-load/physics-resolution systems shared by every
/// runtime entry point (windowed and headless).
pub fn register_scene_systems(app: &mut App) {
    app.add_systems(
        PostStartup,
        (resolve_primitives, resolve_physics_bodies).chain(),
    )
    .add_systems(Update, handle_scene_load)
    .add_systems(Update, resolve_primitives.after(handle_scene_load))
    .add_systems(Update, resolve_physics_bodies.after(resolve_primitives));
}

pub fn resolve_primitives(
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
        commands
            .entity(entity)
            .insert(bsengine_render::MeshRenderer { mesh_id });
    }
}

pub fn resolve_physics_bodies(
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
            ColliderShapeDesc::Capsule {
                half_height,
                radius,
            } => Collider::capsule(*half_height, *radius),
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
                translation: t.translation.0,
                rotation: t.rotation.0,
            },
        ));
    }
}

pub fn handle_scene_load(world: &mut World) {
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
            handle.stop(kira::Tween::default());
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

    // Script state (Bsengine._scripts, timers, collision/message handlers,
    // ...) is reset below by re-running BOOTSTRAP_JS + each entity's script
    // via load_scripts, which replaces the whole `Bsengine` object. This
    // deliberately reuses the existing ScriptRuntime/V8 isolate rather than
    // constructing a new one: creating a second V8 isolate while
    // EditorPlugin's stack is active corrupts V8's isolate state (crashes
    // with "Cannot create a handle without a HandleScope" the moment a
    // script next runs) — see BOOTSTRAP_JS's `var Bsengine` comment for the
    // JS-side half of this fix.

    // Spawn scene and resolve physics inline (Added<> won't fire for same-frame spawns)
    spawn_scene_entities(world, &scene.entities);
    resolve_physics_bodies_world(world);
    load_scripts(world);
}

pub fn resolve_physics_bodies_world(world: &mut World) {
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
            ColliderShapeDesc::Capsule {
                half_height,
                radius,
            } => Collider::capsule(half_height, radius),
        };
        let col = col_base
            .with_restitution(desc.collider.restitution)
            .with_friction(desc.collider.friction)
            .with_sensor(desc.collider.sensor);
        let t = world.get::<Transform>(entity).cloned().unwrap_or_default();
        world.entity_mut(entity).insert((
            rb,
            col,
            PhysicsInput {
                translation: t.translation.0,
                rotation: t.rotation.0,
            },
        ));
    }
}
