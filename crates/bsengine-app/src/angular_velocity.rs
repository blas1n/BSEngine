use bevy_app::{App, Plugin, Update};
use bsengine_core::{AngularVelocity, Time, Transform};
use bsengine_ecs::{Query, Res};
use glam::Quat;

pub struct AngularVelocityPlugin;

impl Plugin for AngularVelocityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_angular_velocity);
    }
}

fn apply_angular_velocity(mut query: Query<(&AngularVelocity, &mut Transform)>, time: Res<Time>) {
    let dt = time.delta_seconds;
    for (av, mut transform) in query.iter_mut() {
        let delta = Quat::from_euler(
            glam::EulerRot::XYZ,
            av.angular.x * dt,
            av.angular.y * dt,
            av.angular.z * dt,
        );
        transform.rotation = (transform.rotation * delta).normalize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{AngularVelocity, Time, Transform};
    use glam::Vec3;
    use std::f32::consts::PI;

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(AngularVelocityPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn angular_velocity_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(AngularVelocityPlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn zero_angular_velocity_does_not_rotate() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Transform::default(), AngularVelocity::default()))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.rotation.w - 1.0).abs() < 0.001);
    }

    #[test]
    fn yaw_rotation_applies() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                AngularVelocity::new(0.0, PI * 0.5, 0.0),
            ))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        let expected = Quat::from_euler(glam::EulerRot::XYZ, 0.0, PI * 0.5, 0.0);
        assert!((transform.rotation.dot(expected)).abs() > 0.999);
    }

    #[test]
    fn rotation_accumulates_over_frames() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((
                Transform::default(),
                AngularVelocity::new(0.0, PI * 0.5, 0.0),
            ))
            .id();

        app.update();
        app.update();

        // After 2 frames at 1s/frame with 0.5π yaw/s → ~π yaw total
        // rotation should be close to 180° around Y
        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!(
            transform.rotation.w.abs() < 0.01,
            "expected ~180 deg rotation"
        );
    }

    #[test]
    fn entity_without_angular_velocity_not_rotated() {
        let mut app = make_app_with_delta(1.0);
        let entity = app.world_mut().spawn(Transform::default()).id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.rotation.w - 1.0).abs() < 0.001);
    }
}
