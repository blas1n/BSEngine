use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Entity, EventWriter};
use bsengine_core::{
    AnimationEventFired, AnimationEvents, AnimationPlayer, AnimationSystems, Time,
};
use bsengine_ecs::{IntoSystemConfigs, Query, Res};

/// Advances every `AnimationPlayer`'s clip time each frame by
/// `Time::delta_seconds`, and sends an [`AnimationEventFired`] for each of the
/// entity's [`AnimationEvents`] the playhead crossed.
pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<AnimationEventFired>()
            .add_systems(Update, advance_animations.in_set(AnimationSystems));
    }
}

fn advance_animations(
    mut query: Query<(Entity, &mut AnimationPlayer, Option<&AnimationEvents>)>,
    time: Res<Time>,
    mut fired: EventWriter<AnimationEventFired>,
) {
    let dt = time.delta_seconds;
    for (entity, mut player, events) in query.iter_mut() {
        let spans = player.tick_spans(dt);
        let Some(events) = events else {
            continue;
        };
        // Span by span, and within a span in the direction of travel: the
        // order the playhead actually met them, whatever order they were
        // listed in.
        for span in spans {
            let mut crossed: Vec<_> = events
                .events
                .iter()
                .filter(|e| e.clip.is_empty() || e.clip == player.clip)
                .filter(|e| player.span_crosses(span, e.time))
                .collect();
            crossed.sort_by(|a, b| a.time.total_cmp(&b.time));
            if span.0 > span.1 {
                crossed.reverse();
            }
            for e in crossed {
                fired.send(AnimationEventFired {
                    entity,
                    clip: player.clip.clone(),
                    name: e.name.clone(),
                });
            }
        }
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

    fn event(clip: &str, time: f32, name: &str) -> bsengine_core::AnimationEvent {
        bsengine_core::AnimationEvent {
            clip: clip.to_string(),
            time,
            name: name.to_string(),
        }
    }

    /// Every `AnimationEventFired` sent so far, by name, in order.
    fn fired(app: &mut bevy_app::App) -> Vec<String> {
        let events = app
            .world()
            .resource::<bevy_ecs::event::Events<bsengine_core::AnimationEventFired>>();
        let mut reader = events.get_reader();
        reader.read(events).map(|e| e.name.clone()).collect()
    }

    /// Frame by frame through a one-second loop at 0.1 s a frame: each event
    /// fires once, in the frame the playhead crosses it, and again on the
    /// next lap; an event at 0 fires as playback leaves 0; an event for
    /// another clip never fires, one with no clip does. Events are read each
    /// frame (Bevy keeps them two frames), so what is collected is exactly
    /// what each frame sent.
    #[test]
    fn events_fire_once_per_crossing_in_playback_order() {
        let mut app = make_app();
        app.world_mut().spawn((
            AnimationPlayer::new("walk").with_duration(1.0),
            bsengine_core::AnimationEvents {
                events: vec![
                    event("walk", 0.75, "right_foot"),
                    event("", 0.25, "left_foot"),
                    event("walk", 0.0, "start"),
                    event("run", 0.5, "other_clip"),
                ],
            },
        ));
        let mut per_frame = Vec::new();
        for _ in 0..13 {
            app.update();
            let names = fired(&mut app);
            app.world_mut()
                .resource_mut::<bevy_ecs::event::Events<bsengine_core::AnimationEventFired>>()
                .clear();
            per_frame.push(names);
        }
        let all: Vec<String> = per_frame.iter().flatten().cloned().collect();
        assert_eq!(
            all,
            ["start", "left_foot", "right_foot", "start", "left_foot"],
            "frames: {per_frame:?}"
        );
        assert_eq!(per_frame[0], ["start"], "0 fires as playback leaves it");
        assert_eq!(
            per_frame[2],
            ["left_foot"],
            "0.25 is crossed going 0.2 -> 0.3"
        );
    }

    /// One long frame over two and a half laps fires every event it passes,
    /// lap by lap, in order; a paused player fires nothing; and in reverse
    /// the order reverses.
    #[test]
    fn long_frames_paused_players_and_reverse_play() {
        let events = || bsengine_core::AnimationEvents {
            events: vec![event("", 0.25, "a"), event("", 0.75, "b")],
        };
        let mut app = make_app();
        app.world_mut()
            .resource_mut::<Time>()
            .set_delta_for_test(2.5);
        app.world_mut()
            .spawn((AnimationPlayer::new("x").with_duration(1.0), events()));
        app.world_mut().spawn((
            AnimationPlayer::new("x").with_duration(1.0).paused(),
            events(),
        ));
        app.update();
        assert_eq!(fired(&mut app), ["a", "b", "a", "b", "a"]);

        let mut app = make_app();
        app.world_mut().spawn((
            AnimationPlayer::new("x")
                .with_duration(1.0)
                .with_speed(-1.0),
            events(),
        ));
        app.world_mut()
            .resource_mut::<Time>()
            .set_delta_for_test(0.9);
        app.update();
        // From 0 backwards: wraps to 1.0, then down to 0.1 -- meeting 0.75
        // before 0.25.
        assert_eq!(fired(&mut app), ["b", "a"]);
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
