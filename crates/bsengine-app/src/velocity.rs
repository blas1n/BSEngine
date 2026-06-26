use bevy_app::{App, Plugin, Update};
use bsengine_core::{Time, Transform, Velocity};
use bsengine_ecs::{Query, Res};

pub struct VelocityPlugin;

impl Plugin for VelocityPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_velocity);
    }
}

fn apply_velocity(mut query: Query<(&Velocity, &mut Transform)>, time: Res<Time>) {
    for (vel, mut transform) in query.iter_mut() {
        transform.translation += vel.linear * time.delta_seconds;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Time, Transform, Velocity};
    use glam::Vec3;

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(VelocityPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn velocity_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(VelocityPlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn entity_moves_by_velocity_times_delta() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Transform::default(), Velocity::new(3.0, 0.0, 0.0)))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.translation.x - 3.0).abs() < 0.001);
    }

    #[test]
    fn velocity_scales_with_delta() {
        let mut app = make_app_with_delta(0.5);
        let entity = app
            .world_mut()
            .spawn((Transform::default(), Velocity::new(0.0, 4.0, 0.0)))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.translation.y - 2.0).abs() < 0.001);
    }

    #[test]
    fn zero_velocity_does_not_move() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Transform::default(), Velocity::default()))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::ZERO);
    }

    #[test]
    fn entity_without_velocity_not_moved() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn(Transform::from_translation(Vec3::new(5.0, 0.0, 0.0)))
            .id();

        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert_eq!(transform.translation, Vec3::new(5.0, 0.0, 0.0));
    }

    #[test]
    fn velocity_accumulates_over_frames() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Transform::default(), Velocity::new(1.0, 0.0, 0.0)))
            .id();

        app.update();
        app.update();
        app.update();

        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!((transform.translation.x - 3.0).abs() < 0.001);
    }
}
