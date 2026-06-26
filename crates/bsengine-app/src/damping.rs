use bevy_app::{App, Plugin, Update};
use bsengine_core::{Damping, Time, Velocity};
use bsengine_ecs::{Query, Res};

pub struct DampingPlugin;

impl Plugin for DampingPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_damping);
    }
}

fn apply_damping(mut query: Query<(&Damping, &mut Velocity)>, time: Res<Time>) {
    let dt = time.delta_seconds;
    for (damping, mut vel) in query.iter_mut() {
        let factor = (1.0 - damping.linear * dt).max(0.0);
        vel.linear *= factor;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Damping, Time, Velocity};
    use glam::Vec3;

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(DampingPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn damping_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(DampingPlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn zero_damping_does_not_change_velocity() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Velocity::new(10.0, 0.0, 0.0), Damping::new(0.0)))
            .id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        assert!((vel.linear.x - 10.0).abs() < 0.001);
    }

    #[test]
    fn damping_reduces_velocity() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Velocity::new(10.0, 0.0, 0.0), Damping::new(0.5)))
            .id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        // factor = (1.0 - 0.5 * 1.0).max(0.0) = 0.5 → 10.0 * 0.5 = 5.0
        assert!((vel.linear.x - 5.0).abs() < 0.001);
    }

    #[test]
    fn full_damping_stops_velocity() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Velocity::new(10.0, 5.0, 3.0), Damping::new(1.0)))
            .id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        // factor = (1.0 - 1.0 * 1.0).max(0.0) = 0.0
        assert_eq!(vel.linear, Vec3::ZERO);
    }

    #[test]
    fn overdamping_clamps_to_zero() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Velocity::new(10.0, 0.0, 0.0), Damping::new(2.0)))
            .id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        // factor = (1.0 - 2.0 * 1.0).max(0.0) = 0.0 — no sign reversal
        assert_eq!(vel.linear, Vec3::ZERO);
    }

    #[test]
    fn entity_without_velocity_not_affected() {
        let mut app = make_app_with_delta(1.0);
        app.world_mut().spawn(Damping::new(1.0));
        app.update();
    }
}
