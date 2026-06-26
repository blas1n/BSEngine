use bevy_app::{App, Plugin, Update};
use bsengine_core::{AngularVelocity, ExternalImpulse, Mass, Velocity};
use bsengine_ecs::Query;

pub struct ExternalImpulsePlugin;

impl Plugin for ExternalImpulsePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, apply_external_impulses);
    }
}

fn apply_external_impulses(
    mut query: Query<(
        &mut Velocity,
        Option<&mut AngularVelocity>,
        &mut ExternalImpulse,
        Option<&Mass>,
    )>,
) {
    for (mut velocity, angular_velocity, mut impulse, mass) in query.iter_mut() {
        if impulse.is_zero() {
            continue;
        }

        let inv_mass = mass.map(|m| m.inverse()).unwrap_or(1.0);

        velocity.linear += impulse.linear * inv_mass;

        if let Some(mut av) = angular_velocity {
            av.angular += impulse.angular * inv_mass;
        }

        impulse.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{AngularVelocity, ExternalImpulse, Mass, Velocity};
    use glam::Vec3;

    fn make_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(ExternalImpulsePlugin);
        app
    }

    #[test]
    fn impulse_applied_to_velocity_with_mass() {
        let mut app = make_app();
        app.world_mut().spawn((
            Velocity::default(),
            ExternalImpulse::linear(Vec3::new(10.0, 0.0, 0.0)),
            Mass::new(2.0),
        ));
        app.update();

        let vel = app
            .world_mut()
            .query::<&Velocity>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!((vel.linear.x - 5.0).abs() < 0.001); // 10 N·s / 2 kg = 5 m/s
    }

    #[test]
    fn impulse_cleared_after_application() {
        let mut app = make_app();
        app.world_mut()
            .spawn((Velocity::default(), ExternalImpulse::linear(Vec3::X)));
        app.update();

        let impulse = app
            .world_mut()
            .query::<&ExternalImpulse>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(impulse.is_zero());
    }

    #[test]
    fn no_mass_defaults_to_one_kg() {
        let mut app = make_app();
        app.world_mut()
            .spawn((Velocity::default(), ExternalImpulse::linear(Vec3::X * 3.0)));
        app.update();

        let vel = app
            .world_mut()
            .query::<&Velocity>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!((vel.linear.x - 3.0).abs() < 0.001);
    }

    #[test]
    fn angular_impulse_applied_when_component_present() {
        let mut app = make_app();
        app.world_mut().spawn((
            Velocity::default(),
            AngularVelocity::default(),
            ExternalImpulse::angular(Vec3::Y * 2.0),
            Mass::new(1.0),
        ));
        app.update();

        let av = app
            .world_mut()
            .query::<&AngularVelocity>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!((av.angular.y - 2.0).abs() < 0.001);
    }
}
