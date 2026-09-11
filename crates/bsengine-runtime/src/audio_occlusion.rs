//! Deciding whether something solid stands between an emitter and the ears.
//!
//! This is the physics half of audio occlusion. The other half — what a muffled
//! sound actually sounds like — lives in `bsengine-audio`, behind
//! [`AudioWorld::set_occlusion`]. The two are split because
//! `bsengine-audio` deliberately depends on neither physics nor a GPU, which is
//! what lets its logic be tested on a machine with no audio device at all.
//!
//! This crate is where they meet: it is the only one that already depends on
//! both.

use bevy_ecs::prelude::*;
use bsengine_audio::{AudioEmitter, AudioListener, AudioOcclusion, AudioWorld};
use bsengine_core::Transform;
use bsengine_physics::PhysicsWorld;

/// How far short of the listener the ray stops, in metres.
///
/// A small margin so a source standing exactly on the listener does not report
/// a hit on nothing. It is *not* what keeps the listener's own collider out of
/// the way — `cast_ray_excluding_pair` does that, because a clearance large
/// enough would have to exceed a half-extent this code cannot know.
const LISTENER_CLEARANCE: f32 = 0.05;

/// Casts one ray per occluding emitter and tells the audio world what it found.
///
/// Runs for entities carrying **both** [`AudioEmitter`] and [`AudioOcclusion`];
/// an emitter without the latter is never ray-cast and never muffled, which is
/// how Unreal ships occlusion too — off unless asked for. That also keeps the
/// cost proportional to the number of emitters that opted in rather than to the
/// number of emitters.
///
/// The result is binary per frame — blocked or not — and the smoothing happens
/// in `set_occlusion`, which tweens over the component's `interpolation_ms`.
/// That is deliberately where it belongs: a per-frame boolean that flickers on
/// a grazing edge becomes an audible click only if something applies it
/// instantly.
pub fn update_audio_occlusion(
    mut audio: ResMut<AudioWorld>,
    physics: Option<Res<PhysicsWorld>>,
    listener: Query<(Entity, &Transform), With<AudioListener>>,
    emitters: Query<(Entity, &Transform, &AudioOcclusion), With<AudioEmitter>>,
) {
    let Some(physics) = physics else {
        return;
    };
    // No ears, nothing to be occluded from. Not an error: a scene is allowed to
    // have no listener, and every emitter simply stays as it was.
    let Some((listener_entity, listener_transform)) = listener.iter().next() else {
        return;
    };
    let ears = listener_transform.position.0;

    for (entity, transform, occlusion) in emitters.iter() {
        let source = transform.position.0;
        let to_ears = ears - source;
        let distance = to_ears.length();
        // Coincident with the listener: nothing can be between them.
        if distance <= LISTENER_CLEARANCE {
            audio.set_occlusion(entity, 0.0, occlusion.interpolation_ms);
            continue;
        }
        let direction = to_ears / distance;
        // Both ends excluded, and that is not belt-and-braces: a source with a
        // collider occludes itself the instant it is given one, and a listener
        // with a collider occludes *everything*. Either way every sound is
        // muffled always, which sounds like a broken filter rather than a
        // broken ray and is correspondingly hard to attribute.
        //
        // Excluding only one end and stopping the ray short of the other does
        // not work: how short would have to exceed that body's half-extent,
        // which is not known here. This was found by test, not by reasoning —
        // the clearance below was 0.05 against a listener whose collider spans
        // 0.5, and `a_source_with_nothing_in_the_way_is_not_occluded` failed.
        let blocked = physics
            .cast_ray_excluding_pair(
                source,
                direction,
                distance - LISTENER_CLEARANCE,
                entity,
                listener_entity,
            )
            .is_some();
        audio.set_occlusion(
            entity,
            if blocked { 1.0 } else { 0.0 },
            occlusion.interpolation_ms,
        );
    }
}
