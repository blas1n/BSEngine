use bevy_app::{App, Plugin, Update};
use bsengine_core::{AnimationPlayer, Time};
use bsengine_ecs::Query;
use bsengine_ecs::Res;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, advance_animations);
    }
}

fn advance_animations(mut query: Query<&mut AnimationPlayer>, time: Res<Time>) {
    let dt = time.delta_seconds;
    for mut player in query.iter_mut() {
        player.tick(dt);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{AnimationPlayer, Time};

    fn make_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(AnimationPlugin);
        let mut t = Time::default();
        t.set_delta_for_test(0.1);
        app.insert_resource(t);
        app
    }

    #[test]
    fn animation_advances_each_frame() {
        let mut app = make_app();
        app.world_mut()
            .spawn(AnimationPlayer::new("walk").with_duration(2.0));
        app.update();

        let time = app
            .world_mut()
            .query::<&AnimationPlayer>()
            .iter(app.world())
            .next()
            .map(|p| p.time)
            .unwrap();
        assert!((time - 0.1).abs() < 0.001);
    }

    #[test]
    fn paused_animation_does_not_advance() {
        let mut app = make_app();
        app.world_mut()
            .spawn(AnimationPlayer::new("idle").with_duration(2.0).paused());
        app.update();

        let time = app
            .world_mut()
            .query::<&AnimationPlayer>()
            .iter(app.world())
            .next()
            .map(|p| p.time)
            .unwrap();
        assert_eq!(time, 0.0);
    }

    #[test]
    fn non_looping_animation_stops() {
        let mut app = make_app();
        // Set dt = 0.1, clip duration = 0.05 → should stop after first frame
        app.world_mut().spawn(
            AnimationPlayer::new("die")
                .with_duration(0.05)
                .with_looping(false),
        );
        app.update();

        let player = app
            .world_mut()
            .query::<&AnimationPlayer>()
            .iter(app.world())
            .next()
            .unwrap()
            .clone();
        assert!(player.is_finished());
        assert!(!player.playing);
    }
}
