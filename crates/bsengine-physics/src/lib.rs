pub mod components;
pub mod plugin;
pub mod world;

pub use components::{
    Collider, ColliderShape, CollisionEvent, PhysicsInput, PhysicsTransform, RaycastHit, RigidBody,
    RigidBodyType,
};
pub use plugin::PhysicsPlugin;
pub use world::PhysicsWorld;

#[cfg(test)]
mod tests {
    use bevy_app::prelude::*;
    use glam::Vec3;

    use super::*;

    fn new_app() -> App {
        let mut app = App::new();
        app.add_plugins(PhysicsPlugin);
        app
    }

    #[test]
    fn dynamic_body_falls_under_gravity() {
        let mut app = new_app();

        let entity = app
            .world_mut()
            .spawn((
                RigidBody::dynamic(),
                Collider::ball(0.5),
                PhysicsInput {
                    translation: Vec3::new(0.0, 10.0, 0.0),
                    rotation: Default::default(),
                },
            ))
            .id();

        for _ in 0..30 {
            app.update();
        }

        let transform = app.world().get::<PhysicsTransform>(entity).unwrap();
        assert!(
            transform.translation.y < 9.0,
            "expected body to fall, got y={}",
            transform.translation.y
        );
    }

    #[test]
    fn static_body_does_not_move() {
        let mut app = new_app();

        let entity = app
            .world_mut()
            .spawn((
                RigidBody::fixed(),
                Collider::cuboid(1.0, 1.0, 1.0),
                PhysicsInput {
                    translation: Vec3::new(0.0, 0.0, 0.0),
                    rotation: Default::default(),
                },
            ))
            .id();

        for _ in 0..30 {
            app.update();
        }

        let transform = app.world().get::<PhysicsTransform>(entity).unwrap();
        assert!(
            transform.translation.y.abs() < 0.01,
            "static body should not move, got y={}",
            transform.translation.y
        );
    }

    #[test]
    fn body_lands_on_floor() {
        let mut app = new_app();

        app.world_mut().spawn((
            RigidBody::fixed(),
            Collider::cuboid(10.0, 0.1, 10.0),
            PhysicsInput {
                translation: Vec3::new(0.0, 0.0, 0.0),
                rotation: Default::default(),
            },
        ));

        let ball = app
            .world_mut()
            .spawn((
                RigidBody::dynamic(),
                Collider::ball(0.5),
                PhysicsInput {
                    translation: Vec3::new(0.0, 5.0, 0.0),
                    rotation: Default::default(),
                },
            ))
            .id();

        for _ in 0..200 {
            app.update();
        }

        let transform = app.world().get::<PhysicsTransform>(ball).unwrap();
        assert!(
            transform.translation.y < 2.0,
            "ball should land on floor, got y={}",
            transform.translation.y
        );
        assert!(
            transform.translation.y > 0.0,
            "ball should rest above floor, got y={}",
            transform.translation.y
        );
    }

    #[test]
    fn collision_event_fires_when_bodies_touch() {
        use bevy_ecs::event::Events;

        let mut app = new_app();

        app.world_mut().spawn((
            RigidBody::fixed(),
            Collider::cuboid(10.0, 0.1, 10.0),
            PhysicsInput {
                translation: Vec3::ZERO,
                rotation: Default::default(),
            },
        ));

        app.world_mut().spawn((
            RigidBody::dynamic(),
            Collider::ball(0.5),
            PhysicsInput {
                translation: Vec3::new(0.0, 1.0, 0.0),
                rotation: Default::default(),
            },
        ));

        let mut received = false;
        for _ in 0..200 {
            app.update();
            let events = app.world().resource::<Events<CollisionEvent>>();
            let mut reader = events.get_reader();
            if reader.read(events).next().is_some() {
                received = true;
                break;
            }
        }
        assert!(
            received,
            "expected collision event when ball lands on floor"
        );
    }

    #[test]
    fn physics_world_gravity_default() {
        let world = PhysicsWorld::default();
        assert!((world.gravity() - 9.81).abs() < 0.001);
    }

    #[test]
    fn physics_world_set_gravity() {
        let mut world = PhysicsWorld::new(9.81);
        world.set_gravity(1.62);
        assert!((world.gravity() - 1.62).abs() < 0.001);
    }
}
