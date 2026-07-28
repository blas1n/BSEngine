use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// A single named animation state within an [`AnimationStateMachine`], describing
/// which clip plays and how, while that state is active.
#[derive(Debug, Clone, Reflect)]
pub struct AsmState {
    /// Name/identifier of the animation clip this state plays.
    pub clip: String,
    /// Whether the clip wraps back to the start after reaching `duration`.
    pub looping: bool,
    /// Playback rate multiplier (1.0 = normal speed).
    pub speed: f32,
    /// Length of the clip, in seconds.
    pub duration: f32,
}

impl AsmState {
    /// Creates a state for the given clip, looping at normal speed with zero duration.
    pub fn new(clip: impl Into<String>) -> Self {
        Self {
            clip: clip.into(),
            looping: true,
            speed: 1.0,
            duration: 0.0,
        }
    }

    /// Sets whether the clip loops when it reaches the end.
    pub fn with_looping(mut self, looping: bool) -> Self {
        self.looping = looping;
        self
    }

    /// Sets the playback rate multiplier.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed;
        self
    }

    /// Sets the clip duration, clamped to be non-negative.
    pub fn with_duration(mut self, duration: f32) -> Self {
        self.duration = duration.max(0.0);
        self
    }
}

/// Predicate evaluated against an [`AnimationStateMachine`]'s parameters to decide
/// whether an [`AsmTransition`] should fire.
#[derive(Debug, Clone, PartialEq, Reflect)]
pub enum TransitionCondition {
    /// Fires once when the named trigger parameter has been set, then clears it.
    Trigger(String),
    /// Fires while the named float parameter is greater than `threshold`.
    FloatGreater {
        /// Name of the float parameter to compare.
        param: String,
        /// Value the parameter must exceed.
        threshold: f32,
    },
    /// Fires while the named float parameter is less than `threshold`.
    FloatLess {
        /// Name of the float parameter to compare.
        param: String,
        /// Value the parameter must fall below.
        threshold: f32,
    },
    /// Fires while the named bool parameter is `true`.
    BoolTrue(String),
    /// Fires while the named bool parameter is `false`.
    BoolFalse(String),
    /// Fires once the current state's clip has finished playing.
    Finished,
}

/// A directed edge in the state machine graph: when `condition` holds while
/// in state `from`, playback crossfades to state `to` over `blend_duration`.
#[derive(Debug, Clone, Reflect)]
pub struct AsmTransition {
    /// Source state name, or `"*"` to match any state.
    pub from: String,
    /// Destination state name to transition into.
    pub to: String,
    /// Predicate that must hold for this transition to fire.
    pub condition: TransitionCondition,
    /// Crossfade duration, in seconds, when this transition fires.
    pub blend_duration: f32,
}

/// Component that drives an `AnimationPlayer` through a named-state graph.
///
/// Each frame the ECS system evaluates transitions in order and fires the first
/// matching one, consuming Trigger parameters in the process.  Crossfade blend
/// weight is exposed via `blend_weight` so renderers can lerp bone poses.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct AnimationStateMachine {
    /// All named states in this machine's graph.
    pub states: HashMap<String, AsmState>,
    /// Edges evaluated, in order, to decide when to change state.
    pub transitions: Vec<AsmTransition>,
    /// Name of the state currently driving playback.
    pub current_state: String,
    /// Float parameters read by `FloatGreater`/`FloatLess` conditions.
    pub params_float: HashMap<String, f32>,
    /// Bool parameters read by `BoolTrue`/`BoolFalse` conditions.
    pub params_bool: HashMap<String, bool>,
    /// Trigger parameters pending consumption by a `Trigger` condition.
    pub triggers: HashSet<String>,
    /// The state being blended *out* during a crossfade (None when not blending).
    pub blend_from: Option<String>,
    /// 0.0 = fully `blend_from`, 1.0 = fully `current_state`.
    pub blend_weight: f32,
    /// Total duration of the in-progress crossfade, in seconds.
    pub blend_duration: f32,
    /// Time elapsed since the in-progress crossfade began, in seconds.
    pub blend_elapsed: f32,
}

impl Default for AnimationStateMachine {
    fn default() -> Self {
        Self::new("")
    }
}

impl AnimationStateMachine {
    /// Creates a state machine with no states or transitions, starting in `initial_state`.
    pub fn new(initial_state: impl Into<String>) -> Self {
        Self {
            states: HashMap::new(),
            transitions: Vec::new(),
            current_state: initial_state.into(),
            params_float: HashMap::new(),
            params_bool: HashMap::new(),
            triggers: HashSet::new(),
            blend_from: None,
            blend_weight: 1.0,
            blend_duration: 0.0,
            blend_elapsed: 0.0,
        }
    }

    /// Registers a named state in the graph, replacing any existing state of the same name.
    pub fn add_state(&mut self, name: impl Into<String>, state: AsmState) -> &mut Self {
        self.states.insert(name.into(), state);
        self
    }

    /// Registers a transition edge from `from` to `to`, evaluated when `condition` holds.
    pub fn add_transition(
        &mut self,
        from: impl Into<String>,
        to: impl Into<String>,
        condition: TransitionCondition,
        blend_duration: f32,
    ) -> &mut Self {
        self.transitions.push(AsmTransition {
            from: from.into(),
            to: to.into(),
            condition,
            blend_duration: blend_duration.max(0.0),
        });
        self
    }

    /// Arms the named trigger parameter so a `Trigger` condition can consume it.
    pub fn set_trigger(&mut self, name: impl Into<String>) {
        self.triggers.insert(name.into());
    }

    /// Sets the named float parameter, read by `FloatGreater`/`FloatLess` conditions.
    pub fn set_float(&mut self, name: impl Into<String>, value: f32) {
        self.params_float.insert(name.into(), value);
    }

    /// Sets the named bool parameter, read by `BoolTrue`/`BoolFalse` conditions.
    pub fn set_bool(&mut self, name: impl Into<String>, value: bool) {
        self.params_bool.insert(name.into(), value);
    }

    /// Returns true while a crossfade between two states is in progress.
    pub fn is_blending(&self) -> bool {
        self.blend_from.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_asm_starts_in_initial_state() {
        let asm = AnimationStateMachine::new("idle");
        assert_eq!(asm.current_state, "idle");
        assert!(!asm.is_blending());
        assert_eq!(asm.blend_weight, 1.0);
    }

    #[test]
    fn add_state_stores_state() {
        let mut asm = AnimationStateMachine::new("idle");
        asm.add_state("idle", AsmState::new("idle_clip").with_duration(1.0));
        assert!(asm.states.contains_key("idle"));
        assert_eq!(asm.states["idle"].clip, "idle_clip");
    }

    #[test]
    fn set_trigger_inserts() {
        let mut asm = AnimationStateMachine::new("idle");
        asm.set_trigger("move");
        assert!(asm.triggers.contains("move"));
    }

    #[test]
    fn set_float_and_bool() {
        let mut asm = AnimationStateMachine::new("idle");
        asm.set_float("speed", 1.5);
        asm.set_bool("grounded", true);
        assert!((asm.params_float["speed"] - 1.5).abs() < 1e-6);
        assert!(asm.params_bool["grounded"]);
    }

    #[test]
    fn asm_state_builder() {
        let s = AsmState::new("run")
            .with_looping(false)
            .with_speed(2.0)
            .with_duration(0.8);
        assert_eq!(s.clip, "run");
        assert!(!s.looping);
        assert!((s.speed - 2.0).abs() < 1e-6);
        assert!((s.duration - 0.8).abs() < 1e-6);
    }

    #[test]
    fn transition_condition_eq() {
        assert_eq!(TransitionCondition::Finished, TransitionCondition::Finished);
        assert_ne!(
            TransitionCondition::Trigger("a".into()),
            TransitionCondition::Trigger("b".into())
        );
    }
}
