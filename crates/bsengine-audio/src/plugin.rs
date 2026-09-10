use bevy_app::prelude::*;
use bevy_ecs::prelude::*;
use bsengine_core::{ProjectDir, Transform};

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
        // Only if nothing has provided one. A caller that inserted
        // `AudioWorld::silent()` did so to avoid constructing a real
        // `AudioManager`, and overwriting it here would undo exactly that.
        if !app.world().contains_resource::<AudioWorld>() {
            app.insert_resource(AudioWorld::default());
        }
        // Registered here rather than in `bsengine_scene::register_gameplay_
        // reflect_types` because `bsengine-scene` does not depend on this
        // crate, and `AudioPlugin` is in both the windowed runtime and the
        // headless `--test` app, so the two cannot drift.
        app.register_type::<AudioListener>();
        app.register_type::<AudioEmitter>();
        // Listener first: an emitter's spatial track is created *linked to* a
        // listener, so on the very first frame the ears have to exist before
        // any source can be attached to them.
        // Startup, not Update: the layout is authored data, read once. A
        // system that re-applied it every frame would stomp any runtime
        // setBusVolume straight back to the authored value.
        app.add_systems(Startup, load_bus_layout);
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
    audio.set_listener_pose(transform.position.0, transform.rotation.0);
}

/// Pushes every [`AudioEmitter`] entity's position into [`AudioWorld`].
fn sync_emitters(
    mut audio: ResMut<AudioWorld>,
    query: Query<(Entity, &Transform), With<AudioEmitter>>,
) {
    for (entity, transform) in query.iter() {
        audio.set_emitter_position(entity, transform.position.0);
    }
}

/// Loads `<project>/assets/audio/buses.ron` if it exists.
///
/// Discovery is by convention rather than configuration — one project-wide
/// layout at a known path, as Godot does with `default_bus_layout.tres` — so
/// a manifest and an asset cannot disagree about where buses live, and no
/// `project.toml` change is needed.
///
/// A project with no such file keeps today's behaviour exactly: every sound
/// plays on Master. That is what lets this land without editing a single
/// existing game or E2E recording.
fn load_bus_layout(mut audio: ResMut<AudioWorld>, project_dir: Option<Res<ProjectDir>>) {
    let Some(project_dir) = project_dir else {
        return;
    };
    let path = std::path::Path::new(&project_dir.0).join("assets/audio/buses.ron");
    if !path.is_file() {
        return;
    }
    let src = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(e) => {
            tracing::warn!("[audio] cannot read {}: {e}", path.display());
            return;
        }
    };
    match crate::bus::BusLayout::from_ron(&src) {
        Ok(layout) => audio.apply_bus_layout(&layout),
        // Reported, not fatal: a malformed mixer file should not stop a game
        // from starting, but it must never be silent about it either.
        Err(e) => tracing::warn!("[audio] cannot parse {}: {e}", path.display()),
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
        app.insert_resource(AudioWorld::silent());
        app.add_plugins(AudioPlugin);
        let at = Vec3::new(1.0, 2.0, 3.0);
        let entity = app
            .world_mut()
            .spawn((AudioListener, Transform::from_position(at)))
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
            .position = moved.into();
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
        app.insert_resource(AudioWorld::silent());
        app.add_plugins(AudioPlugin);
        app.world_mut()
            .spawn((AudioListener, Transform::from_position(Vec3::ZERO)));
        let at = Vec3::new(5.0, 0.0, 0.0);
        let emitter = app
            .world_mut()
            .spawn((AudioEmitter::default(), Transform::from_position(at)))
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
            .position = moved.into();
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
        app.insert_resource(AudioWorld::silent());
        app.add_plugins(AudioPlugin);
        app.world_mut()
            .spawn((AudioListener, Transform::from_position(Vec3::ZERO)));
        let plain = app
            .world_mut()
            .spawn(Transform::from_position(Vec3::new(3.0, 0.0, 0.0)))
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
        app.insert_resource(AudioWorld::silent());
        app.add_plugins(AudioPlugin);
        app.world_mut().spawn((
            AudioEmitter::default(),
            Transform::from_position(Vec3::ZERO),
        ));
        app.update();
    }
}

#[cfg(test)]
mod bus_loading_tests {
    use super::*;
    use bsengine_app::new_app;

    /// A project directory containing a bus layout at the conventional path.
    fn project_with_layout(src: &str) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join("assets/audio")).expect("mkdir");
        std::fs::write(dir.path().join("assets/audio/buses.ron"), src).expect("write");
        dir
    }

    fn app_for(dir: &tempfile::TempDir) -> App {
        let mut app = new_app();
        app.insert_resource(AudioWorld::silent());
        app.insert_resource(ProjectDir(dir.path().to_string_lossy().into_owned()));
        app.add_plugins(AudioPlugin);
        app
    }

    #[test]
    fn a_project_with_a_layout_gets_its_buses() {
        let dir = project_with_layout(
            r#"BusLayout(buses: [
                Bus(name: "music", parent: None,        volume_db:  0.0),
                Bus(name: "sfx",   parent: None,        volume_db: -6.0),
                Bus(name: "ui",    parent: Some("sfx"), volume_db: -3.0),
            ])"#,
        );
        let mut app = app_for(&dir);
        app.update();

        let audio = app.world().resource::<AudioWorld>();
        assert_eq!(audio.bus_volume("sfx"), Some(-6.0));
        assert_eq!(
            audio.bus_volume("ui"),
            Some(-3.0),
            "distinct volumes, so reading the wrong bus cannot look correct"
        );
        assert_eq!(audio.bus_parent("ui"), Some(Some("sfx".to_string())));
    }

    #[test]
    fn a_project_with_no_layout_still_runs() {
        // The feature is additive: every existing game has no buses.ron and
        // must behave exactly as before. This is what keeps the 12 committed
        // E2E recordings valid.
        let dir = tempfile::tempdir().expect("tempdir");
        let mut app = app_for(&dir);
        app.update();

        assert_eq!(app.world().resource::<AudioWorld>().bus_volume("sfx"), None);
    }

    #[test]
    fn a_malformed_layout_is_survivable() {
        let dir = project_with_layout("this is not ron at all");
        let mut app = app_for(&dir);
        app.update();

        assert_eq!(
            app.world().resource::<AudioWorld>().bus_volume("sfx"),
            None,
            "a broken mixer file must not stop the game from starting"
        );
    }

    #[test]
    fn the_layout_is_loaded_once_not_every_frame() {
        let dir = project_with_layout(
            r#"BusLayout(buses: [Bus(name: "sfx", parent: None, volume_db: -6.0)])"#,
        );
        let mut app = app_for(&dir);
        app.update();
        // A runtime change has to survive the next frames. Re-applying the
        // authored file every frame would stomp it straight back to -6.
        app.world_mut()
            .resource_mut::<AudioWorld>()
            .set_bus_volume("sfx", -20.0);
        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<AudioWorld>().bus_volume("sfx"),
            Some(-20.0),
            "the layout is authored data loaded once, not re-applied every frame"
        );
    }
}
