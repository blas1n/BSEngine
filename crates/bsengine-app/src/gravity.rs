use bevy_app::{App, Plugin, Update};
use bsengine_core::{Gravity, GravityScale, Time, Velocity};
use bsengine_ecs::{Query, Res};

pub struct GravityPlugin;

impl Plugin for GravityPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Gravity::default())
            .add_systems(Update, apply_gravity);
    }
}

fn apply_gravity(
    mut query: Query<(&mut Velocity, Option<&GravityScale>)>,
    gravity: Res<Gravity>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds;
    for (mut vel, scale) in query.iter_mut() {
        let s = scale.map(|gs| gs.value()).unwrap_or(1.0);
        vel.linear += gravity.acceleration * s * dt;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Gravity, GravityScale, Time, Velocity};
    use glam::Vec3;

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(GravityPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn gravity_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(GravityPlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn applies_downward_acceleration() {
        let mut app = make_app_with_delta(1.0);
        let entity = app.world_mut().spawn(Velocity::default()).id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        assert!(vel.linear.y < 0.0, "gravity should pull downward");
        assert!((vel.linear.y - (-9.81)).abs() < 0.01);
    }

    #[test]
    fn gravity_scales_with_delta() {
        let mut app = make_app_with_delta(0.5);
        let entity = app.world_mut().spawn(Velocity::default()).id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        assert!((vel.linear.y - (-4.905)).abs() < 0.01);
    }

    #[test]
    fn gravity_scale_component_multiplies() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Velocity::default(), GravityScale::new(2.0)))
            .id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        assert!((vel.linear.y - (-19.62)).abs() < 0.01);
    }

    #[test]
    fn gravity_scale_zero_is_immune() {
        let mut app = make_app_with_delta(1.0);
        let entity = app
            .world_mut()
            .spawn((Velocity::default(), GravityScale::new(0.0)))
            .id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        assert_eq!(vel.linear, Vec3::ZERO);
    }

    #[test]
    fn custom_gravity_resource() {
        let mut app = make_app_with_delta(1.0);
        app.insert_resource(Gravity::new(Vec3::new(0.0, -20.0, 0.0)));
        let entity = app.world_mut().spawn(Velocity::default()).id();

        app.update();

        let vel = app.world().get::<Velocity>(entity).unwrap();
        assert!((vel.linear.y - (-20.0)).abs() < 0.01);
    }

    #[test]
    fn entity_without_velocity_not_affected() {
        let mut app = make_app_with_delta(1.0);
        // Just a plain entity with no Velocity — should not crash
        app.world_mut().spawn(());
        app.update();
    }
}
