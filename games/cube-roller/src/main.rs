use bevy_ecs::system::Local;
use bsengine_app::{new_app, Startup, Update};
use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Time, Transform, Velocity};
use bsengine_ecs::{Commands, Component, Entity, IntoSystemConfigs, Query, Res, ResMut, With};
use bsengine_input::{Input, InputPlugin, KeyCode};
use bsengine_render::{MeshRenderer, RenderPlugin};
use bsengine_rhi_wgpu::{cube_vertices, GpuMeshRegistry, WgpuRHIPlugin};
use bsengine_window::{WindowDescriptor, WindowPlugin};
use glam::{Quat, Vec2, Vec3};

const FLOOR_Y: f32 = 0.5;
const ACCEL: f32 = 20.0;
const MAX_SPEED: f32 = 8.0;
const DAMPING: f32 = 0.85;
const GRAVITY: f32 = 20.0;
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
        .add_plugins(RenderPlugin)
        .add_systems(Startup, setup)
        .add_systems(Update, (player_control, collect_items, respawn).chain())
        .run();
}

fn setup(mut commands: Commands, registry: Option<ResMut<GpuMeshRegistry>>) {
    commands.spawn((
        Camera::perspective(60.0, 16.0 / 9.0),
        Transform::from_translation(Vec3::new(0.0, 8.0, 12.0)),
    ));

    commands.spawn(DirectionalLight::default());

    let Some(mut reg) = registry else { return };
    let (verts, indices) = cube_vertices();
    let cube_id = reg.register(&verts, &indices);

    // Floor
    commands.spawn((
        MeshRenderer { mesh_id: cube_id },
        Transform {
            translation: Vec3::new(0.0, -0.5, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::new(20.0, 0.2, 20.0),
        },
        GlobalTransform::default(),
    ));

    // Player
    commands.spawn((
        Player,
        MeshRenderer { mesh_id: cube_id },
        Transform {
            translation: Vec3::new(0.0, FLOOR_Y, 0.0),
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        },
        GlobalTransform::default(),
        Velocity::default(),
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
                translation: pos,
                rotation: Quat::IDENTITY,
                scale: Vec3::splat(0.4),
            },
            GlobalTransform::default(),
        ));
    }
}

fn player_control(
    keys: Res<Input<KeyCode>>,
    mut query: Query<(&mut Velocity, &mut Transform), With<Player>>,
    time: Res<Time>,
) {
    let Ok((mut vel, mut transform)) = query.get_single_mut() else {
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
        vel.linear.x += accel.x;
        vel.linear.z += accel.z;
    }

    let hspeed = Vec2::new(vel.linear.x, vel.linear.z).length();
    if hspeed > MAX_SPEED {
        let scale = MAX_SPEED / hspeed;
        vel.linear.x *= scale;
        vel.linear.z *= scale;
    }

    vel.linear.x *= DAMPING;
    vel.linear.z *= DAMPING;
    vel.linear.y -= GRAVITY * dt;

    transform.translation += vel.linear * dt;

    if transform.translation.y < FLOOR_Y {
        transform.translation.y = FLOOR_Y;
        if vel.linear.y < 0.0 {
            vel.linear.y = 0.0;
        }
    }
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
        if (player_t.translation - item_t.translation).length() < COLLECT_DIST {
            commands.entity(entity).despawn();
            *score += 1;
            println!("Score: {}", *score);
        }
    }
}

fn respawn(mut query: Query<(&mut Transform, &mut Velocity), With<Player>>) {
    let Ok((mut t, mut vel)) = query.get_single_mut() else {
        return;
    };
    if t.translation.y < RESPAWN_Y {
        t.translation = Vec3::new(0.0, FLOOR_Y, 0.0);
        vel.linear = Vec3::ZERO;
        println!("Respawned!");
    }
}
