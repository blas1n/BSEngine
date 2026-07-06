use bevy_app::{App, Plugin, PostUpdate};
use bsengine_core::{AnimationPlayer, AnimationStateMachine, Time, TransitionCondition};
use bsengine_ecs::{Query, Res};

pub struct AnimationStateMachinePlugin;

impl Plugin for AnimationStateMachinePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(PostUpdate, advance_state_machines);
    }
}

fn advance_state_machines(
    mut query: Query<(&mut AnimationStateMachine, &mut AnimationPlayer)>,
    time: Res<Time>,
) {
    let dt = time.delta_seconds;
    for (mut asm, mut player) in query.iter_mut() {
        // Advance crossfade blend weight.
        if asm.blend_from.is_some() {
            asm.blend_elapsed += dt;
            let w = (asm.blend_elapsed / asm.blend_duration.max(f32::EPSILON)).clamp(0.0, 1.0);
            asm.blend_weight = w;
            if w >= 1.0 {
                asm.blend_from = None;
            }
        }

        // Snapshot values needed before borrowing asm mutably.
        let current = asm.current_state.clone();
        let player_finished = player.is_finished();

        // Find the first transition whose condition is satisfied.
        let fired = asm
            .transitions
            .iter()
            .find(|t| {
                let from_ok = t.from == "*" || t.from == current;
                if !from_ok {
                    return false;
                }
                match &t.condition {
                    TransitionCondition::Trigger(name) => asm.triggers.contains(name.as_str()),
                    TransitionCondition::FloatGreater { param, threshold } => {
                        asm.params_float.get(param.as_str()).copied().unwrap_or(0.0) > *threshold
                    }
                    TransitionCondition::FloatLess { param, threshold } => {
                        asm.params_float.get(param.as_str()).copied().unwrap_or(0.0) < *threshold
                    }
                    TransitionCondition::BoolTrue(name) => {
                        asm.params_bool.get(name.as_str()).copied().unwrap_or(false)
                    }
                    TransitionCondition::BoolFalse(name) => {
                        !asm.params_bool.get(name.as_str()).copied().unwrap_or(false)
                    }
                    TransitionCondition::Finished => player_finished,
                }
            })
            .cloned();

        if let Some(t) = fired {
            // Consume trigger so it doesn't re-fire next frame.
            if let TransitionCondition::Trigger(name) = &t.condition {
                asm.triggers.remove(name.as_str());
            }

            // Start crossfade.
            asm.blend_from = Some(current);
            asm.blend_weight = 0.0;
            asm.blend_duration = t.blend_duration.max(f32::EPSILON);
            asm.blend_elapsed = 0.0;
            asm.current_state = t.to.clone();

            // Drive AnimationPlayer to the new state's clip.
            if let Some(state) = asm.states.get(&t.to) {
                player.clip = state.clip.clone();
                player.looping = state.looping;
                player.speed = state.speed;
                player.time = 0.0;
                player.playing = true;
                if state.duration > 0.0 {
                    player.duration = state.duration;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{AsmState, Time, TransitionCondition};

    fn make_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(AnimationStateMachinePlugin);
        let mut t = Time::default();
        t.set_delta_for_test(0.1);
        app.insert_resource(t);
        app
    }

    #[test]
    fn trigger_causes_state_transition() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_state("walk", AsmState::new("walk_clip").with_duration(0.8));
        asm.add_transition(
            "idle",
            "walk",
            TransitionCondition::Trigger("move".into()),
            0.2,
        );
        asm.set_trigger("move");
        let player = AnimationPlayer::new("idle_clip").with_duration(1.0);
        app.world_mut().spawn((asm, player));
        app.update();

        let state = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .current_state
            .clone();
        assert_eq!(state, "walk");
    }

    #[test]
    fn float_greater_causes_transition() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_state("run", AsmState::new("run_clip").with_duration(0.5));
        asm.add_transition(
            "idle",
            "run",
            TransitionCondition::FloatGreater {
                param: "speed".into(),
                threshold: 0.5,
            },
            0.1,
        );
        asm.set_float("speed", 1.0);
        let player = AnimationPlayer::new("idle_clip").with_duration(1.0);
        app.world_mut().spawn((asm, player));
        app.update();

        let state = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .current_state
            .clone();
        assert_eq!(state, "run");
    }

    #[test]
    fn no_transition_without_trigger() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_state("walk", AsmState::new("walk_clip").with_duration(0.8));
        asm.add_transition(
            "idle",
            "walk",
            TransitionCondition::Trigger("move".into()),
            0.2,
        );
        let player = AnimationPlayer::new("idle_clip").with_duration(1.0);
        app.world_mut().spawn((asm, player));
        app.update();

        let state = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .current_state
            .clone();
        assert_eq!(state, "idle");
    }

    #[test]
    fn trigger_consumed_after_transition() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_state("walk", AsmState::new("walk_clip").with_duration(0.8));
        asm.add_transition(
            "idle",
            "walk",
            TransitionCondition::Trigger("move".into()),
            0.2,
        );
        asm.set_trigger("move");
        let player = AnimationPlayer::new("idle_clip").with_duration(1.0);
        app.world_mut().spawn((asm, player));
        app.update();

        let asm = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .clone();
        assert!(!asm.triggers.contains("move"));
    }

    #[test]
    fn crossfade_blend_advances_over_frames() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_state("walk", AsmState::new("walk_clip").with_duration(0.8));
        asm.add_transition(
            "idle",
            "walk",
            TransitionCondition::Trigger("move".into()),
            0.5,
        );
        asm.set_trigger("move");
        let player = AnimationPlayer::new("idle_clip").with_duration(1.0);
        app.world_mut().spawn((asm, player));
        app.update(); // transition fires, blend starts at weight=0.0
        app.update(); // blend_elapsed=0.1, duration=0.5 → weight=0.2

        let asm = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .clone();
        assert!(asm.blend_from.is_some());
        assert!(asm.blend_weight > 0.0 && asm.blend_weight < 1.0);
    }

    #[test]
    fn finished_condition_triggers_transition() {
        // This test also needs AnimationPlugin (Update) to tick the player before
        // the ASM (PostUpdate) evaluates the Finished condition.
        let mut app = crate::new_app();
        app.add_plugins(crate::AnimationPlugin);
        app.add_plugins(AnimationStateMachinePlugin);
        let mut t = Time::default();
        t.set_delta_for_test(0.1);
        app.insert_resource(t);

        let mut asm = AnimationStateMachine::new("attack");
        asm.add_state(
            "attack",
            AsmState::new("attack_clip")
                .with_duration(0.05)
                .with_looping(false),
        );
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_transition("attack", "idle", TransitionCondition::Finished, 0.1);
        let player = AnimationPlayer::new("attack_clip")
            .with_duration(0.05)
            .with_looping(false);
        app.world_mut().spawn((asm, player));
        // Update: AnimationPlugin ticks player (dt=0.1 > duration=0.05 → finished).
        //         ASM (PostUpdate) sees finished → transitions to "idle".
        app.update();

        let state = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .current_state
            .clone();
        assert_eq!(state, "idle");
    }

    #[test]
    fn player_clip_updates_on_transition() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_state("run", AsmState::new("run_clip").with_duration(0.6));
        asm.add_transition(
            "idle",
            "run",
            TransitionCondition::Trigger("go".into()),
            0.0,
        );
        asm.set_trigger("go");
        let player = AnimationPlayer::new("idle_clip").with_duration(1.0);
        app.world_mut().spawn((asm, player));
        app.update();

        let clip = app
            .world_mut()
            .query::<&AnimationPlayer>()
            .iter(app.world())
            .next()
            .unwrap()
            .clip
            .clone();
        assert_eq!(clip, "run_clip");
    }

    #[test]
    fn wildcard_from_matches_any_state() {
        let mut app = make_app();
        let mut asm = AnimationStateMachine::new("run");
        asm.add_state("run", AsmState::new("run_clip").with_duration(0.5));
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        asm.add_transition(
            "*",
            "idle",
            TransitionCondition::BoolTrue("stop".into()),
            0.1,
        );
        asm.set_bool("stop", true);
        let player = AnimationPlayer::new("run_clip").with_duration(0.5);
        app.world_mut().spawn((asm, player));
        app.update();

        let state = app
            .world_mut()
            .query::<&AnimationStateMachine>()
            .iter(app.world())
            .next()
            .unwrap()
            .current_state
            .clone();
        assert_eq!(state, "idle");
    }
}
