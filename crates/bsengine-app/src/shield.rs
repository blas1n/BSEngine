use bevy_app::{App, Plugin, Update};
use bsengine_core::{Shield, Time};
use bsengine_ecs::Query;
use bsengine_ecs::Res;

pub struct ShieldPlugin;

impl Plugin for ShieldPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, recharge_shields);
    }
}

fn recharge_shields(mut query: Query<&mut Shield>, time: Res<Time>) {
    let dt = time.delta_seconds;
    for mut shield in query.iter_mut() {
        shield.tick(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Shield, Time};

    fn make_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(ShieldPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(1.0);
        app.insert_resource(t);
        app
    }

    #[test]
    fn shield_recharges_each_frame() {
        let mut app = make_app();
        let mut shield = Shield::new(100.0).with_recharge(10.0, 0.0);
        shield.absorb(50.0); // current = 50, cooldown = 0
        app.world_mut().spawn(shield);
        app.update(); // +10 HP

        let current = app
            .world_mut()
            .query::<&Shield>()
            .iter(app.world())
            .next()
            .map(|s| s.current)
            .unwrap();
        assert!((current - 60.0).abs() < 0.01);
    }

    #[test]
    fn shield_waits_for_cooldown() {
        let mut app = make_app(); // dt = 1.0
        let mut shield = Shield::new(100.0).with_recharge(10.0, 2.0);
        shield.absorb(50.0); // cooldown = 2.0
        app.world_mut().spawn(shield);
        app.update(); // cooldown becomes 1.0, no recharge

        let current = app
            .world_mut()
            .query::<&Shield>()
            .iter(app.world())
            .next()
            .map(|s| s.current)
            .unwrap();
        assert!((current - 50.0).abs() < 0.01);
    }
}
