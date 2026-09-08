//! Applies the editor Timeline panel's cutscene preview to the world, and
//! undoes it when the preview ends.
//!
//! Its own module rather than another addition to `plugin.rs`, which is
//! already ninety thousand lines.

use bsengine_core::{AnimationPlayer, InspectorState};

/// Applies the Timeline panel's animation preview, and puts back what it
/// replaced when the preview ends.
///
/// The runtime deliberately restores nothing when a timeline stops (see
/// `bsengine_app::timeline_playback`): a cutscene that exists to move the
/// player somewhere should leave them there. An authoring tool is the
/// opposite case — looking at a cutscene must not be a way to edit the scene
/// — so this is the one place the two rules differ on purpose.
pub fn apply_timeline_preview_animation(
    inspector: Option<bevy_ecs::system::ResMut<InspectorState>>,
    mut saved: bevy_ecs::system::Local<Vec<(bevy_ecs::entity::Entity, AnimationPlayer)>>,
    mut players: bevy_ecs::system::Query<(
        bevy_ecs::entity::Entity,
        &bsengine_scene::Name,
        &mut AnimationPlayer,
    )>,
) {
    let requested: Vec<(String, String, f32)> = inspector
        .as_ref()
        .and_then(|insp| insp.timeline_preview.as_ref())
        .map(|preview| preview.clips.clone())
        .unwrap_or_default();

    if requested.is_empty() {
        // Restore in one pass and forget. An entity that has since been
        // despawned simply is not found, which is not an error.
        for (entity, original) in saved.drain(..) {
            if let Ok((_, _, mut player)) = players.get_mut(entity) {
                *player = original;
            }
        }
        return;
    }

    for (name, clip, clip_time) in requested {
        for (entity, entity_name, mut player) in players.iter_mut() {
            if entity_name.0 != name {
                continue;
            }
            // Snapshot the first time this entity is touched, never after --
            // re-snapshotting on a later frame would save the preview's own
            // values, and restore would then put those back.
            if !saved.iter().any(|(e, _)| *e == entity) {
                saved.push((entity, player.clone()));
            }
            player.clip = clip.clone();
            player.time = clip_time;
            // Paused: a scrub is a question about one instant, and the editor
            // has no clock advancing clips anyway.
            player.playing = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Scrubbing sets the named entity's clip and its time *within* that
    /// clip, and leaves it paused -- scrubbing asks what an instant looks
    /// like, not for playback to start.
    #[test]
    fn a_preview_sets_the_named_entitys_clip_and_time() {
        use bsengine_core::TimelinePreview;

        let mut app = bevy_app::App::new();
        app.insert_resource(InspectorState::default());
        app.add_systems(bevy_app::Update, apply_timeline_preview_animation);
        let mut before = AnimationPlayer::new("Idle");
        before.time = 3.0;
        before.playing = true;
        let subject = app
            .world_mut()
            .spawn((bsengine_scene::Name("Subject".to_string()), before))
            .id();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = Some(TimelinePreview {
            camera: None,
            clips: vec![("Subject".to_string(), "Survey".to_string(), 1.25)],
        });
        app.update();

        let player = app.world().get::<AnimationPlayer>(subject).expect("player");
        assert_eq!(player.clip, "Survey");
        assert!((player.time - 1.25).abs() < 1e-6, "got {}", player.time);
        assert!(
            !player.playing,
            "scrubbing shows an instant; it must not start playback"
        );
    }

    /// The pairing that matters: ending the preview puts back exactly what
    /// was there. Without restore, one scrub would permanently change the
    /// scene and a save afterwards would freeze the character mid-cutscene.
    #[test]
    fn ending_a_preview_restores_the_animation_player() {
        use bsengine_core::TimelinePreview;

        let mut app = bevy_app::App::new();
        app.insert_resource(InspectorState::default());
        app.add_systems(bevy_app::Update, apply_timeline_preview_animation);
        let mut before = AnimationPlayer::new("Idle");
        before.time = 3.0;
        before.playing = true;
        let subject = app
            .world_mut()
            .spawn((bsengine_scene::Name("Subject".to_string()), before.clone()))
            .id();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = Some(TimelinePreview {
            camera: None,
            clips: vec![("Subject".to_string(), "Survey".to_string(), 1.25)],
        });
        app.update();
        assert_eq!(
            app.world().get::<AnimationPlayer>(subject).unwrap().clip,
            "Survey",
            "the preview must actually have applied, or the restore below \
             proves nothing"
        );

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = None;
        app.update();

        let after = app.world().get::<AnimationPlayer>(subject).expect("player");
        assert_eq!(after, &before, "the preview must leave nothing behind");
    }

    /// The snapshot is taken once, on the first frame an entity is touched.
    /// Re-snapshotting every frame would capture the preview's own values,
    /// and restore would then put *those* back -- which looks like working
    /// code until a preview lasts more than one frame, i.e. always.
    #[test]
    fn a_multi_frame_preview_still_restores_the_original() {
        use bsengine_core::TimelinePreview;

        let mut app = bevy_app::App::new();
        app.insert_resource(InspectorState::default());
        app.add_systems(bevy_app::Update, apply_timeline_preview_animation);
        let mut before = AnimationPlayer::new("Idle");
        before.time = 3.0;
        before.playing = true;
        let subject = app
            .world_mut()
            .spawn((bsengine_scene::Name("Subject".to_string()), before.clone()))
            .id();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = Some(TimelinePreview {
            camera: None,
            clips: vec![("Subject".to_string(), "Survey".to_string(), 1.25)],
        });
        app.update();
        // A second previewing frame, with the playhead moved on.
        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = Some(TimelinePreview {
            camera: None,
            clips: vec![("Subject".to_string(), "Survey".to_string(), 2.5)],
        });
        app.update();

        app.world_mut()
            .resource_mut::<InspectorState>()
            .timeline_preview = None;
        app.update();

        let after = app.world().get::<AnimationPlayer>(subject).expect("player");
        assert_eq!(
            after, &before,
            "a preview lasting two frames must restore the pre-preview state, \
             not the state the first previewing frame left behind"
        );
    }
}
