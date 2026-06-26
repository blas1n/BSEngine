use bevy_app::{App, Plugin, Update};
use bsengine_core::{Lifetime, Time};
use bsengine_ecs::{Commands, Entity, Query, Res};

pub struct LifetimePlugin;

impl Plugin for LifetimePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_lifetimes);
    }
}

fn tick_lifetimes(
    mut commands: Commands,
    mut query: Query<(Entity, &mut Lifetime)>,
    time: Res<Time>,
) {
    for (entity, mut lifetime) in query.iter_mut() {
        lifetime.remaining -= time.delta_seconds;
        if lifetime.remaining <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Lifetime, Time};

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(LifetimePlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn lifetime_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(LifetimePlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn entity_despawns_when_lifetime_expires() {
        let mut app = make_app_with_delta(1.0);
        let entity = app.world_mut().spawn(Lifetime::from_seconds(0.5)).id();

        app.update();

        assert!(app.world().get_entity(entity).is_none());
    }

    #[test]
    fn entity_survives_when_lifetime_not_expired() {
        let mut app = make_app_with_delta(0.1);
        let entity = app.world_mut().spawn(Lifetime::from_seconds(1.0)).id();

        app.update();

        assert!(app.world().get_entity(entity).is_some());
    }

    #[test]
    fn lifetime_decrements_each_frame() {
        let mut app = make_app_with_delta(0.3);
        let entity = app.world_mut().spawn(Lifetime::from_seconds(1.0)).id();

        app.update();

        let lifetime = app.world().get::<Lifetime>(entity).unwrap();
        assert!((lifetime.remaining - 0.7).abs() < 0.01);
    }

    #[test]
    fn entity_despawns_after_multiple_frames() {
        let mut app = make_app_with_delta(0.4);
        let entity = app.world_mut().spawn(Lifetime::from_seconds(1.0)).id();

        // Frame 1: remaining = 0.6
        app.update();
        assert!(app.world().get_entity(entity).is_some());

        // Frame 2: remaining = 0.2
        app.update();
        assert!(app.world().get_entity(entity).is_some());

        // Frame 3: remaining = -0.2 → despawn
        app.update();
        assert!(app.world().get_entity(entity).is_none());
    }
}
