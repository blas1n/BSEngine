use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use glam::{Quat, Vec3};
use rapier3d::prelude::*;

use crate::{
    components::{
        Collider, ColliderShape, PhysicsHandles, PhysicsInput, PhysicsTransform, RigidBody,
        RigidBodyType,
    },
    world::PhysicsWorld,
};

pub struct PhysicsPlugin;

impl Plugin for PhysicsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PhysicsWorld::default());
        app.add_systems(
            Update,
            (spawn_bodies, step_world, sync_from_rapier).chain(),
        );
    }
}

// Convert project glam 0.29 Vec3 → rapier math Vec3 (glam 0.33) via raw floats
fn to_rapier_vec(v: Vec3) -> Vector {
    Vector::new(v.x, v.y, v.z)
}

// Convert project glam 0.29 Quat → rapier Rotation (glam 0.33 Quat) via raw floats
fn to_rapier_rot(q: Quat) -> Rotation {
    Rotation::from_xyzw(q.x, q.y, q.z, q.w)
}

// Convert rapier math Vec3 (glam 0.33) → project glam 0.29 Vec3
fn from_rapier_vec(v: Vector) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

// Convert rapier Rotation (glam 0.33 Quat) → project glam 0.29 Quat
fn from_rapier_rot(r: Rotation) -> Quat {
    Quat::from_xyzw(r.x, r.y, r.z, r.w)
}

fn spawn_bodies(
    mut world: ResMut<PhysicsWorld>,
    mut commands: Commands,
    query: Query<
        (Entity, &RigidBody, &Collider, Option<&PhysicsInput>),
        Without<PhysicsHandles>,
    >,
) {
    for (entity, rigid_body, collider, input) in query.iter() {
        let pos = input.map(|i| i.translation).unwrap_or(Vec3::ZERO);
        let rot = input.map(|i| i.rotation).unwrap_or(Quat::IDENTITY);

        let pose = Pose::from_parts(to_rapier_vec(pos), to_rapier_rot(rot));

        let rb = match rigid_body.body_type {
            RigidBodyType::Dynamic => RigidBodyBuilder::dynamic()
                .position(pose)
                .linear_damping(rigid_body.linear_damping)
                .angular_damping(rigid_body.angular_damping)
                .build(),
            RigidBodyType::Static => RigidBodyBuilder::fixed().position(pose).build(),
            RigidBodyType::KinematicPosition => {
                RigidBodyBuilder::kinematic_position_based().position(pose).build()
            }
        };

        let body_handle = world.rigid_body_set.insert(rb);

        let shape = make_shape(&collider.shape);
        let coll = ColliderBuilder::new(shape)
            .restitution(collider.restitution)
            .friction(collider.friction)
            .density(collider.density)
            .build();

        let collider_handle = world.add_collider(coll, body_handle);

        commands.entity(entity).insert((
            PhysicsHandles { body_handle, collider_handle },
            PhysicsTransform { translation: pos, rotation: rot },
        ));
    }
}

fn step_world(
    mut world: ResMut<PhysicsWorld>,
    query: Query<(&PhysicsHandles, &PhysicsInput), With<RigidBody>>,
) {
    for (handles, input) in query.iter() {
        if let Some(body) = world.rigid_body_set.get_mut(handles.body_handle) {
            if body.is_kinematic() {
                body.set_next_kinematic_position(Pose::from_parts(
                    to_rapier_vec(input.translation),
                    to_rapier_rot(input.rotation),
                ));
            }
        }
    }

    world.step();
}

fn sync_from_rapier(
    world: Res<PhysicsWorld>,
    mut query: Query<(&PhysicsHandles, &mut PhysicsTransform)>,
) {
    for (handles, mut transform) in query.iter_mut() {
        if let Some(body) = world.rigid_body_set.get(handles.body_handle) {
            transform.translation = from_rapier_vec(body.translation());
            transform.rotation = from_rapier_rot(*body.rotation());
        }
    }
}

fn make_shape(shape: &ColliderShape) -> SharedShape {
    match shape {
        ColliderShape::Box { half_extents } => {
            SharedShape::cuboid(half_extents.x, half_extents.y, half_extents.z)
        }
        ColliderShape::Sphere { radius } => SharedShape::ball(*radius),
        ColliderShape::Capsule { half_height, radius } => {
            SharedShape::capsule_y(*half_height, *radius)
        }
    }
}
