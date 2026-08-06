//! A small physics demo: roll a cube around a floor and collect items.
//!
//! Movement is driven by Rapier through `bsengine-physics`. This used to run on
//! `bsengine-core`'s kinematic `Velocity` component with hand-rolled gravity,
//! damping and a floor clamp; roadmap item 33 removed that stack, and this demo
//! was its only real consumer. Gravity, damping and the floor are now the
//! physics engine's job, so what remains here is input and gameplay.

use bevy_ecs::system::Local;
use bsengine_app::{new_app, Startup, Update};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Time, Transform};
use bsengine_ecs::{Commands, Component, Entity, IntoSystemConfigs, Query, Res, ResMut, With};
use bsengine_input::{Input, InputPlugin, KeyCode};
use bsengine_physics::{
    Collider, PhysicsInput, PhysicsPlugin, PhysicsTransform, PhysicsWorld, RigidBody,
};
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{cube_vertices, GpuMeshRegistry, WgpuRHIPlugin};
use bsengine_window::{WindowDescriptor, WindowPlugin};
use glam::{Quat, Vec2, Vec3};

const FLOOR_Y: f32 = 0.5;
const ACCEL: f32 = 20.0;
const MAX_SPEED: f32 = 8.0;
/// Per-second linear damping handed to Rapier, replacing the old per-frame
/// `velocity *= 0.85`. That factor was frame-rate dependent; this is not.
const LINEAR_DAMPING: f32 = 2.0;
const COLLECT_DIST: f32 = 1.5;
const RESPAWN_Y: f32 = -10.0;

#[derive(Component)]
struct Player;

#[derive(Component)]
struct Item;

fn main() {
    new_app()
        .add_plugins(WgpuRHIPlugin)
        .add_plugins(WindowPlugin {
            descriptor: WindowDescriptor {
                title: "Cube Roller".to_string(),
                width: 1280,
                height: 720,
                resizable: true,
            },
        })
        .add_plugins(InputPlugin)
        .add_plugins(PhysicsPlugin)
        .add_plugins(RenderPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (player_control, collect_items, respawn).chain())
        .run();
}

fn setup(mut commands: Commands, registry: Option<ResMut<GpuMeshRegistry>>) {
    commands.spawn((
        Camera::perspective(60.0, 16.0 / 9.0),
        Transform::from_position(Vec3::new(0.0, 8.0, 12.0)),
    ));

    commands.spawn((
        DirectionalLight::default(),
        Transform {
            rotation: Quat::from_rotation_arc(Vec3::NEG_Z, Vec3::new(-0.4, -0.8, -0.4).normalize())
                .into(),
            ..Default::default()
        },
        GlobalTransform::default(),
    ));

    let Some(mut reg) = registry else { return };
    let (verts, indices) = cube_vertices();
    let cube_id = reg.register(&verts, &indices);

    // Floor. The mesh is a unit cube scaled to 20 x 0.2 x 20, so the collider's
    // half-extents are half of that.
    commands.spawn((
        MeshRenderer { mesh_id: cube_id },
        Transform {
            position: Vec3::new(0.0, -0.5, 0.0).into(),
            rotation: Quat::IDENTITY.into(),
            scale: Vec3::new(20.0, 0.2, 20.0).into(),
        },
        GlobalTransform::default(),
        RigidBody::fixed(),
        Collider::cuboid(10.0, 0.1, 10.0),
        PhysicsInput {
            position: Vec3::new(0.0, -0.5, 0.0).into(),
            rotation: Quat::IDENTITY.into(),
        },
        PhysicsTransform::default(),
    ));

    // Player. Damping is Rapier's now; so are gravity and standing on the floor.
    commands.spawn((
        Player,
        MeshRenderer { mesh_id: cube_id },
        Transform {
            position: Vec3::new(0.0, FLOOR_Y, 0.0).into(),
            rotation: Quat::IDENTITY.into(),
            scale: Vec3::ONE.into(),
        },
        GlobalTransform::default(),
        RigidBody {
            linear_damping: LINEAR_DAMPING,
            ..RigidBody::dynamic()
        },
        Collider::cuboid(0.5, 0.5, 0.5),
        PhysicsInput {
            position: Vec3::new(0.0, FLOOR_Y, 0.0).into(),
            rotation: Quat::IDENTITY.into(),
        },
        PhysicsTransform::default(),
    ));

    // Items
    for pos in [
        Vec3::new(3.0, FLOOR_Y, 0.0),
        Vec3::new(-3.0, FLOOR_Y, 2.0),
        Vec3::new(0.0, FLOOR_Y, -4.0),
        Vec3::new(5.0, FLOOR_Y, -3.0),
        Vec3::new(-5.0, FLOOR_Y, -2.0),
    ] {
        commands.spawn((
            Item,
            MeshRenderer { mesh_id: cube_id },
            Transform {
                position: pos.into(),
                rotation: Quat::IDENTITY.into(),
                scale: Vec3::splat(0.4).into(),
            },
            GlobalTransform::default(),
        ));
    }
}

fn player_control(
    keys: Res<Input<KeyCode>>,
    query: Query<Entity, With<Player>>,
    mut physics: ResMut<PhysicsWorld>,
    time: Res<Time>,
) {
    let Ok(entity) = query.get_single() else {
        return;
    };
    let Some(mut linvel) = physics.get_linvel(entity) else {
        // The body is registered with Rapier a frame after it is spawned.
        return;
    };
    let dt = time.delta_seconds;

    let mut dir = Vec3::ZERO;
    if keys.is_pressed(&KeyCode::W) {
        dir.z -= 1.0;
    }
    if keys.is_pressed(&KeyCode::S) {
        dir.z += 1.0;
    }
    if keys.is_pressed(&KeyCode::A) {
        dir.x -= 1.0;
    }
    if keys.is_pressed(&KeyCode::D) {
        dir.x += 1.0;
    }

    if dir.length_squared() > 0.0 {
        let accel = dir.normalize() * ACCEL * dt;
        linvel.x += accel.x;
        linvel.z += accel.z;
    }

    // Clamp horizontal speed only — the vertical component is gravity's, and
    // clamping it would fight the physics step.
    let hspeed = Vec2::new(linvel.x, linvel.z).length();
    if hspeed > MAX_SPEED {
        let scale = MAX_SPEED / hspeed;
        linvel.x *= scale;
        linvel.z *= scale;
    }

    physics.set_linvel(entity, linvel);
}

fn collect_items(
    player: Query<&Transform, With<Player>>,
    items: Query<(Entity, &Transform), With<Item>>,
    mut commands: Commands,
    mut score: Local<u32>,
) {
    let Ok(player_t) = player.get_single() else {
        return;
    };

    for (entity, item_t) in items.iter() {
        if (player_t.position.0 - item_t.position.0).length() < COLLECT_DIST {
            commands.entity(entity).despawn();
            *score += 1;
            println!("Score: {}", *score);
        }
    }
}

fn respawn(query: Query<(Entity, &Transform), With<Player>>, mut physics: ResMut<PhysicsWorld>) {
    let Ok((entity, t)) = query.get_single() else {
        return;
    };
    if t.position.y < RESPAWN_Y {
        // Teleporting a dynamic body means telling Rapier, not writing
        // `Transform` — the physics step is what drives `Transform` here, so a
        // write to it would be overwritten on the very next frame.
        physics.set_translation(entity, Vec3::new(0.0, FLOOR_Y, 0.0));
        physics.set_linvel(entity, Vec3::ZERO);
        println!("Respawned!");
    }
}
