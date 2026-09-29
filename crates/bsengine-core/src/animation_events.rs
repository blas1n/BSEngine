//! Animation events: names fired when a clip's playhead crosses a time.

use bevy_ecs::prelude::{Component, Entity, Event, ReflectComponent, SystemSet};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// One named moment in a clip -- a footstep, the frame a sword connects.
#[derive(Debug, Clone, PartialEq, Default, Reflect)]
#[reflect(Default)]
pub struct AnimationEvent {
    /// The clip it belongs to; empty for "whatever clip is playing".
    pub clip: String,
    /// Seconds into the clip.
    pub time: f32,
    /// What is reported when it fires.
    pub name: String,
}

/// The animation events an entity's [`crate::AnimationPlayer`] fires.
///
/// Unity's Animation Events, Unreal's Anim Notifies and Godot's method
/// tracks all come down to this: a name at a time in a clip, delivered when
/// playback crosses it. They live on the entity rather than in the clip
/// because a clip here is whatever the glTF file carried, and glTF has no
/// events -- the same clip can be given different events on different
/// characters.
///
/// An event fires once each time the playhead passes over its time: moving
/// forward from before it to at-or-past it, or backward from after it to
/// at-or-before it, including across a loop's wrap and over several laps in
/// one long frame. An event at time 0 fires as playback leaves 0 -- on the
/// first frame of play, and again after each wrap. A paused player fires
/// nothing, and neither does a clip that is not the event's.
#[derive(Component, Debug, Clone, PartialEq, Default, Reflect)]
#[reflect(Component, Default)]
pub struct AnimationEvents {
    /// Every event, in any order; each frame's firings come out in playback
    /// order regardless.
    pub events: Vec<AnimationEvent>,
}

/// Sent when an entity's animation crosses one of its [`AnimationEvents`].
/// Scripts receive it through `Bsengine.onAnimationEvent`.
#[derive(Event, Debug, Clone, PartialEq)]
pub struct AnimationEventFired {
    /// The entity whose player crossed the event.
    pub entity: Entity,
    /// The clip that was playing.
    pub clip: String,
    /// The event's name.
    pub name: String,
}

/// The system set advancing every `AnimationPlayer` and sending
/// [`AnimationEventFired`]. Anything reading those events in the same frame
/// orders itself after this set, so the frame an event is seen in does not
/// depend on how the scheduler happened to sort two unrelated systems -- a
/// difference a replay would see.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub struct AnimationSystems;
