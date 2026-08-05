use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bsengine_core::Transform;

use crate::spatial::{AudioEmitter, AudioListener};
use crate::world::AudioWorld;

/// Registers the [`AudioWorld`] resource and keeps positional audio in step
/// with the scene.
///
/// Playback itself is driven imperatively rather than by an ECS component
/// pair: `bsengine-scripting` holds the queue of requested plays
/// (`SoundLoads`/`PendingSounds`) and the live handles (`SoundHandles`), and
/// calls [`AudioWorld::play`] or [`AudioWorld::play_at`] when a decoded
/// [`AudioSourceAsset`](crate::AudioSourceAsset) becomes available. What the
/// systems here own is *where* things are — the listener's pose and each
/// emitter's position — which is a property of the scene and belongs in the
/// ECS.
pub struct AudioPlugin;

impl Plugin for AudioPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AudioWorld::default());
        // Registered here rather than in `bsengine_scene::register_gameplay_
        // reflect_types` because `bsengine-scene` does not depend on this
        // crate, and `AudioPlugin` is in both the windowed runtime and the
        // headless `--test` app, so the two cannot drift.
        app.register_type::<AudioListener>();
        app.register_type::<AudioEmitter>();
        // Listener first: an emitter's spatial track is created *linked to* a
        // listener, so on the very first frame the ears have to exist before
        // any source can be attached to them.
        app.add_systems(Update, (sync_listener, sync_emitters).chain());
    }
}

/// Pushes the [`AudioListener`] entity's pose into [`AudioWorld`].
///
/// More than one listener has no meaningful answer — you cannot hear a scene
/// from two places — so the first is used and the rest are reported once per
/// frame rather than silently ignored.
fn sync_listener(mut audio: ResMut<AudioWorld>, query: Query<&Transform, With<AudioListener>>) {
    let mut iter = query.iter();
    let Some(transform) = iter.next() else {
        return;
    };
    let extra = iter.count();
    if extra > 0 {
        tracing::warn!(
            "{} extra AudioListener entities ignored — a scene is heard from one place",
            extra
        );
    }
    audio.set_listener_pose(transform.translation.0, transform.rotation.0);
}

/// Pushes every [`AudioEmitter`] entity's position into [`AudioWorld`].
fn sync_emitters(
    mut audio: ResMut<AudioWorld>,
    query: Query<(Entity, &Transform), With<AudioEmitter>>,
) {
    for (entity, transform) in query.iter() {
        audio.set_emitter_position(entity, transform.translation.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_app::new_app;
    use glam::{Quat, Vec3};

    #[test]
    fn the_listener_pose_follows_its_entity() {
        let mut app = new_app();
        app.add_plugins(AudioPlugin);
        let at = Vec3::new(1.0, 2.0, 3.0);
        let entity = app
            .world_mut()
            .spawn((AudioListener, Transform::from_translation(at)))
            .id();

        app.update();
        assert_eq!(
            app.world().resource::<AudioWorld>().last_listener_pose(),
            Some((at, Quat::IDENTITY)),
            "the listener should be wherever its entity is"
        );

        let moved = Vec3::new(-4.0, 0.0, 8.0);
        app.world_mut()
            .get_mut::<Transform>(entity)
            .unwrap()
            .translation = moved.into();
        app.update();

        assert_eq!(
            app.world()
                .resource::<AudioWorld>()
                .last_listener_pose()
                .map(|(p, _)| p),
            Some(moved),
            "and should keep following it, not just be placed once"
        );
    }

    #[test]
    fn an_emitter_position_follows_its_entity() {
        let mut app = new_app();
        app.add_plugins(AudioPlugin);
        app.world_mut()
            .spawn((AudioListener, Transform::from_translation(Vec3::ZERO)));
        let at = Vec3::new(5.0, 0.0, 0.0);
        let emitter = app
            .world_mut()
            .spawn((AudioEmitter::default(), Transform::from_translation(at)))
            .id();

        app.update();
        assert_eq!(
            app.world()
                .resource::<AudioWorld>()
                .last_emitter_position(emitter),
            Some(at)
        );

        let moved = Vec3::new(5.0, 0.0, 9.0);
        app.world_mut()
            .get_mut::<Transform>(emitter)
            .unwrap()
            .translation = moved.into();
        app.update();

        assert_eq!(
            app.world()
                .resource::<AudioWorld>()
                .last_emitter_position(emitter),
            Some(moved)
        );
    }

    #[test]
    fn an_entity_without_the_marker_is_not_an_emitter() {
        // Otherwise every entity in the scene would be a sound source and the
        // component would mean nothing.
        let mut app = new_app();
        app.add_plugins(AudioPlugin);
        app.world_mut()
            .spawn((AudioListener, Transform::from_translation(Vec3::ZERO)));
        let plain = app
            .world_mut()
            .spawn(Transform::from_translation(Vec3::new(3.0, 0.0, 0.0)))
            .id();

        app.update();

        assert_eq!(
            app.world()
                .resource::<AudioWorld>()
                .last_emitter_position(plain),
            None
        );
    }

    #[test]
    fn a_scene_with_no_listener_does_not_panic() {
        // Emitters exist before the camera is spawned during scene load, and a
        // frame in that state must not be fatal.
        let mut app = new_app();
        app.add_plugins(AudioPlugin);
        app.world_mut().spawn((
            AudioEmitter::default(),
            Transform::from_translation(Vec3::ZERO),
        ));
        app.update();
    }
}
