use bevy_app::{App, Plugin, Update};
use bsengine_core::{Time, Timer};
use bsengine_ecs::{Query, Res};

pub struct TimerPlugin;

impl Plugin for TimerPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, tick_timers);
    }
}

fn tick_timers(mut query: Query<&mut Timer>, time: Res<Time>) {
    for mut timer in query.iter_mut() {
        timer.tick(time.delta_seconds);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{Time, Timer};

    fn make_app_with_delta(delta: f32) -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(TimerPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(delta);
        app.insert_resource(t);
        app
    }

    #[test]
    fn timer_plugin_builds() {
        let mut app = crate::new_app();
        app.add_plugins(TimerPlugin);
        app.insert_resource(Time::default());
    }

    #[test]
    fn plugin_ticks_timer() {
        let mut app = make_app_with_delta(0.5);
        let entity = app.world_mut().spawn(Timer::new(1.0)).id();

        app.update();

        let timer = app.world().get::<Timer>(entity).unwrap();
        assert!((timer.fraction() - 0.5).abs() < 0.001);
    }

    #[test]
    fn timer_fires_via_plugin() {
        let mut app = make_app_with_delta(1.0);
        let entity = app.world_mut().spawn(Timer::new(1.0)).id();

        app.update();

        let timer = app.world().get::<Timer>(entity).unwrap();
        assert!(timer.just_finished());
        assert!(timer.is_finished());
    }

    #[test]
    fn repeating_timer_fires_each_period() {
        let mut app = make_app_with_delta(1.0);
        let entity = app.world_mut().spawn(Timer::repeating(1.0)).id();

        app.update();
        assert!(app.world().get::<Timer>(entity).unwrap().just_finished());

        app.update();
        assert!(app.world().get::<Timer>(entity).unwrap().just_finished());
    }

    #[test]
    fn just_finished_clears_between_frames() {
        let mut app = make_app_with_delta(1.0);
        let entity = app.world_mut().spawn(Timer::new(1.0)).id();

        app.update(); // fires
        app.update(); // should not re-fire

        let timer = app.world().get::<Timer>(entity).unwrap();
        assert!(!timer.just_finished());
    }
}
