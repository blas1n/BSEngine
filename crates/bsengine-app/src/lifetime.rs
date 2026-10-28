use bevy_app::{App, Plugin, Update};
use bsengine_core::{Lifetime, Time};
use bsengine_ecs::{Commands, Entity, Query, Res};

/// Counts down each entity's `Lifetime` and despawns it once the remaining time hits zero.
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
            // `try_`: a child that expires in the same frame as its parent is
            // already gone with it by the time its own command runs.
            commands.entity(entity).try_despawn();
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

        assert!(app.world().get_entity(entity).is_err());
    }

    #[test]
    fn an_expired_entity_takes_its_children_with_it() {
        // A projectile's trail, a pickup's glow: what is parented to a timed
        // entity goes when it does, as Unity's Destroy and Godot's queue_free
        // take the children. A sibling root with no Lifetime stays.
        let mut app = make_app_with_delta(1.0);
        let parent = app.world_mut().spawn(Lifetime::from_seconds(0.5)).id();
        let child = app.world_mut().spawn(bsengine_core::ChildOf(parent)).id();
        let bystander = app.world_mut().spawn_empty().id();

        app.update();

        assert!(
            app.world().get_entity(parent).is_err(),
            "premise: it expired"
        );
        assert!(
            app.world().get_entity(child).is_err(),
            "and its child with it"
        );
        assert!(
            app.world().get_entity(bystander).is_ok(),
            "but not a stranger"
        );
    }

    #[test]
    fn entity_survives_when_lifetime_not_expired() {
        let mut app = make_app_with_delta(0.1);
        let entity = app.world_mut().spawn(Lifetime::from_seconds(1.0)).id();

        app.update();

        assert!(app.world().get_entity(entity).is_ok());
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
        assert!(app.world().get_entity(entity).is_ok());

        // Frame 2: remaining = 0.2
        app.update();
        assert!(app.world().get_entity(entity).is_ok());

        // Frame 3: remaining = -0.2 → despawn
        app.update();
        assert!(app.world().get_entity(entity).is_err());
    }
}
