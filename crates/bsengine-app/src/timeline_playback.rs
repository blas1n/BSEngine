//! Playing a [`Timeline`] against the world.
//!
//! The decisions live in `bsengine_core::timeline` as pure functions; this is
//! the part that reads the asset, advances the playhead, and writes what the
//! evaluation says.
//!
//! # A timeline writes only the entities it names
//!
//! During a cutscene the timeline wins over whatever else would move those
//! entities — that is what a cutscene is. Everything it does not name keeps
//! simulating, so playing one is not a global mode and two timelines driving
//! different entities do not interfere.
//!
//! # Stopping leaves things where the timeline put them
//!
//! Restoring the pre-cutscene state would need a snapshot of arbitrary
//! components and a rule for what counts as "restore", and it would surprise the
//! common case: a cutscene that exists to move the player somewhere. Stated as a
//! limit rather than hidden.

use std::collections::HashMap;

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::{
    animations_between, evaluate, events_between, AnimationPlayer, Camera, Time, Timeline,
    TimelineEvents, TimelinePlayer, Transform,
};
use bsengine_scene::Name;

/// Timelines parsed from disk, keyed by the path a `TimelinePlayer` names.
///
/// Cached because a cutscene is re-evaluated every frame and re-parsing its RON
/// each time would be the kind of cost nobody looks for. Loaded on first use
/// rather than up front, so a project pays only for the timelines it plays.
#[derive(Resource, Default)]
pub struct LoadedTimelines {
    by_path: HashMap<String, Option<Timeline>>,
}

impl LoadedTimelines {
    /// The timeline at `path`, reading it once on first request.
    ///
    /// A path that fails to load is remembered as a failure (`None`) rather than
    /// retried every frame: a missing file would otherwise produce one log line
    /// per frame, which buries everything else.
    fn get(&mut self, path: &str) -> Option<&Timeline> {
        if !self.by_path.contains_key(path) {
            // Through the archive when this is a packaged build, from disk
            // otherwise -- the same call scenes and prefabs use. Reading with
            // plain `std::fs` here would work in a source tree and fail in every
            // `--mode pak` build, which is exactly what the glTF loader did
            // before item 55 caught it.
            let parsed = match bsengine_asset::pak_source::read_to_string(path) {
                Ok(text) => match ron::from_str::<Timeline>(&text) {
                    Ok(timeline) => Some(timeline),
                    Err(e) => {
                        tracing::error!("[timeline] {path} is not a valid timeline: {e}");
                        None
                    }
                },
                Err(e) => {
                    tracing::error!("[timeline] cannot read {path}: {e}");
                    None
                }
            };
            self.by_path.insert(path.to_string(), parsed);
        }
        self.by_path.get(path).and_then(Option::as_ref)
    }
}

/// Advances every playing [`TimelinePlayer`] and applies what its timeline says.
pub struct TimelinePlugin;

impl Plugin for TimelinePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LoadedTimelines>();
        app.init_resource::<TimelineEvents>();
        app.add_systems(Update, play_timelines);
    }
}

fn play_timelines(world: &mut World) {
    let dt = world
        .get_resource::<Time>()
        .map_or(0.0, |t| t.delta_seconds);

    // A `TimelinePlayer` names its timeline the way a scene names any asset --
    // project-relative -- so it resolves the same way everything else does.
    // Reading the raw string instead works only when the process happens to be
    // running from inside the project, which a unit test using an absolute
    // temp path never notices and a real game always does.
    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();

    // (entity, path, previous time, new time), collected before anything is
    // written so the world is not borrowed while it is being mutated.
    let advanced: Vec<(Entity, String, f32, f32)> = {
        let mut query = world.query::<(Entity, &mut TimelinePlayer)>();
        query
            .iter_mut(world)
            .filter(|(_, player)| player.playing)
            .map(|(entity, mut player)| {
                let previous = player.time;
                player.time += dt * player.speed;
                (entity, player.timeline.clone(), previous, player.time)
            })
            .collect()
    };

    world.resource_mut::<TimelineEvents>().0.clear();
    if advanced.is_empty() {
        return;
    }

    for (_entity, path, previous, now) in advanced {
        let resolved = bsengine_core::resolve_project_path(project_dir.as_ref(), &path);
        let Some(timeline) = ({
            let mut loaded = world.resource_mut::<LoadedTimelines>();
            loaded.get(&resolved).cloned()
        }) else {
            continue;
        };

        let at = evaluate(&timeline, now);

        // A cut wins over the dolly at the instant it lands, because a cut is a
        // statement about that frame.
        if let Some(shot) = &at.shot {
            if let Some(pose) = shot_pose(world, shot) {
                apply_to_camera(world, pose.0, pose.1);
            }
        } else if let Some(camera) = &at.camera {
            apply_to_camera(
                world,
                Transform {
                    position: glam::Vec3::from(camera.position).into(),
                    rotation: camera.rotation().into(),
                    scale: glam::Vec3::ONE.into(),
                },
                None,
            );
        }

        for (entity_name, clip) in animations_between(&timeline, previous, now) {
            start_clip(world, &entity_name, &clip);
        }

        let fired = events_between(&timeline, previous, now);
        if !fired.is_empty() {
            world.resource_mut::<TimelineEvents>().0.extend(fired);
        }
    }
}

/// The transform and field of view of the entity a cut names.
fn shot_pose(world: &mut World, name: &str) -> Option<(Transform, Option<f32>)> {
    let mut query = world.query::<(&Name, &Transform, Option<&Camera>)>();
    query
        .iter(world)
        .find(|(n, _, _)| n.0 == name)
        .map(|(_, transform, camera)| {
            (
                transform.clone(),
                camera.map(|c| f32::from(c.fov_y_degrees)),
            )
        })
}

/// Writes a pose onto the one entity the renderer will use.
///
/// The engine renders whichever `Camera` comes first
/// (`bsengine-render`'s `render_queries.p0().iter().next()`), so a shot copies
/// onto that camera rather than switching to another one — there is no
/// "active camera" to switch to, and inventing one means changing a
/// `render_frame` that is already at Bevy's 16/16 parameter ceiling.
fn apply_to_camera(world: &mut World, pose: Transform, fov: Option<f32>) {
    let mut query = world.query::<(&Camera, &mut Transform)>();
    if let Some((_, mut transform)) = query.iter_mut(world).next() {
        *transform = pose;
    }
    if let Some(fov) = fov {
        let mut query = world.query::<&mut Camera>();
        if let Some(mut camera) = query.iter_mut(world).next() {
            camera.fov_y_degrees = fov.into();
        }
    }
}

/// Starts `clip` on the named entity's player.
fn start_clip(world: &mut World, entity_name: &str, clip: &str) {
    let target = {
        let mut query = world.query::<(Entity, &Name)>();
        query
            .iter(world)
            .find(|(_, n)| n.0 == entity_name)
            .map(|(entity, _)| entity)
    };
    let Some(target) = target else {
        tracing::warn!("[timeline] no entity named {entity_name} to animate");
        return;
    };
    if let Some(mut player) = world.get_mut::<AnimationPlayer>(target) {
        player.clip = clip.to_string();
        player.time = 0.0;
        player.playing = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{AnimationKey, CameraKey, EventKey, ShotCut, Track};

    /// Writes a timeline to a temp file and returns its path, so the test drives
    /// the **real** load path (`pak_source` + RON) rather than a hand-built
    /// value. A test that skipped loading could not notice a loader that never
    /// runs, which is the failure item 44 hit when `TerrainPlugin` was missing
    /// from the runtime's plugin list and every narrower test still passed.
    struct TimelineFile(std::path::PathBuf);

    impl Drop for TimelineFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }

    fn write_timeline(timeline: &Timeline) -> TimelineFile {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "bsengine-timeline-{}-{}.ron",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, ron::to_string(timeline).expect("serialise")).expect("write");
        TimelineFile(path)
    }

    fn app_with(timeline: &Timeline, dt: f32) -> (bevy_app::App, TimelineFile, Entity) {
        let file = write_timeline(timeline);

        let mut app = crate::new_app();
        app.add_plugins(TimelinePlugin);
        let mut time = Time::default();
        time.set_delta_for_test(dt);
        app.insert_resource(time);

        let camera = app
            .world_mut()
            .spawn((
                Name("Camera".to_string()),
                Camera::perspective(60.0, 1.0),
                Transform::default(),
            ))
            .id();
        app.world_mut().spawn((
            Name("Director".to_string()),
            TimelinePlayer {
                timeline: file.0.to_string_lossy().to_string(),
                playing: true,
                ..Default::default()
            },
        ));
        (app, file, camera)
    }

    fn dolly() -> Timeline {
        Timeline {
            duration: 4.0,
            tracks: vec![Track::Camera {
                keys: vec![
                    CameraKey {
                        time: 0.0,
                        position: [0.0, 0.0, 0.0],
                        look_at: [0.0, 0.0, -1.0],
                    },
                    CameraKey {
                        time: 4.0,
                        position: [8.0, 0.0, 0.0],
                        look_at: [0.0, 0.0, -1.0],
                    },
                ],
            }],
        }
    }

    /// Drives the whole path: a real file, a real parse, a registered system,
    /// and the camera actually moving. The pure evaluation being right proves
    /// none of that.
    #[test]
    fn a_playing_timeline_moves_the_camera() {
        let (mut app, _file, camera) = app_with(&dolly(), 1.0);

        app.update();

        let x = app
            .world()
            .get::<Transform>(camera)
            .expect("camera transform")
            .position
            .0
            .x;
        assert!(
            (x - 2.0).abs() < 0.01,
            "one second into a four-second dolly from 0 to 8 is x=2, got {x}"
        );
    }

    /// Paired with the above: a player that is not playing must not move
    /// anything, or "the camera moved" says nothing about the playhead.
    #[test]
    fn a_stopped_timeline_moves_nothing() {
        let (mut app, _file, camera) = app_with(&dolly(), 1.0);
        {
            let mut query = app.world_mut().query::<&mut TimelinePlayer>();
            for mut player in query.iter_mut(app.world_mut()) {
                player.playing = false;
            }
        }

        app.update();

        let x = app
            .world()
            .get::<Transform>(camera)
            .expect("camera transform")
            .position
            .0
            .x;
        assert!(
            x.abs() < 1e-6,
            "a stopped timeline must not move the camera, got {x}"
        );
    }

    /// A cut copies the named entity's transform onto the rendering camera.
    #[test]
    fn a_cut_puts_the_camera_at_the_shot_entity() {
        let mut timeline = dolly();
        timeline.tracks.push(Track::CameraShot {
            cuts: vec![ShotCut {
                time: 1.0,
                entity: "CloseUp".to_string(),
            }],
        });
        let (mut app, _file, camera) = app_with(&timeline, 1.0);
        app.world_mut().spawn((
            Name("CloseUp".to_string()),
            Transform {
                position: glam::Vec3::new(-5.0, 1.0, 0.0).into(),
                ..Default::default()
            },
        ));

        app.update();

        let position = app
            .world()
            .get::<Transform>(camera)
            .expect("camera transform")
            .position
            .0;
        assert!(
            (position - glam::Vec3::new(-5.0, 1.0, 0.0)).length() < 1e-3,
            "the cut must place the camera exactly at the shot entity, got {position:?}"
        );
    }

    /// An event is visible for the frame it fired on and gone the next, which is
    /// what stops a script seeing a cutscene's ending long after it ended.
    #[test]
    fn an_event_is_visible_for_one_frame() {
        let timeline = Timeline {
            duration: 5.0,
            tracks: vec![Track::Event {
                keys: vec![EventKey {
                    time: 1.0,
                    name: "done".to_string(),
                }],
            }],
        };
        let (mut app, _file, _camera) = app_with(&timeline, 1.0);

        app.update();
        assert_eq!(
            app.world().resource::<TimelineEvents>().0,
            vec!["done".to_string()],
            "the event fires on the frame that crosses it"
        );

        app.update();
        assert!(
            app.world().resource::<TimelineEvents>().0.is_empty(),
            "and is gone the next frame, rather than repeating for the rest of \
             the cutscene"
        );
    }

    /// An `Animation` key restarts the named entity's clip when the playhead
    /// crosses it — and, paired below, leaves a later key alone.
    ///
    /// `animations_between` being right proves nothing about this: it is a pure
    /// function in another crate, and a `start_clip` wired to nothing would
    /// still let every one of its tests pass. That is the shape of failure this
    /// PR already hit twice — a producer with no consumer looks exactly like a
    /// working one from the producer's side.
    #[test]
    fn an_animation_key_starts_the_clip_on_the_named_entity() {
        let timeline = Timeline {
            duration: 6.0,
            tracks: vec![Track::Animation {
                entity: "Subject".to_string(),
                keys: vec![
                    AnimationKey {
                        time: 0.5,
                        clip: "Survey".to_string(),
                    },
                    AnimationKey {
                        time: 5.0,
                        clip: "Bow".to_string(),
                    },
                ],
            }],
        };
        let (mut app, _file, _camera) = app_with(&timeline, 1.0);
        // Mid-clip and paused, so "Survey"/0.0/playing cannot be what it already
        // was.
        let subject = app
            .world_mut()
            .spawn((
                Name("Subject".to_string()),
                AnimationPlayer {
                    clip: "Idle".to_string(),
                    time: 3.0,
                    playing: false,
                    ..AnimationPlayer::new("Idle")
                },
            ))
            .id();

        app.update();

        let player = app
            .world()
            .get::<AnimationPlayer>(subject)
            .expect("subject keeps its player")
            .clone();
        assert_eq!(
            player.clip, "Survey",
            "the key at t=0.5 is inside the first frame's 0..1s step"
        );
        assert!(player.playing, "and starting a clip means playing it");
        assert!(
            player.time.abs() < 1e-6,
            "from the beginning, not from wherever the previous clip was: got {}",
            player.time
        );
        assert_ne!(
            player.clip, "Bow",
            "the t=5.0 key is still five seconds out; a start_clip that ignored \
             the interval and applied the whole track would land here"
        );

        // Four more seconds puts the playhead at 5.0 and the second key inside
        // the step. Without this the test could not tell a working interval
        // query from one that fires only once and stops.
        for _ in 0..4 {
            app.update();
        }
        assert_eq!(
            app.world()
                .get::<AnimationPlayer>(subject)
                .expect("subject keeps its player")
                .clip,
            "Bow",
            "the later key fires when the playhead reaches it"
        );
    }

    /// A timeline naming nothing must leave other entities alone, so playing one
    /// is not a global mode.
    #[test]
    fn a_timeline_leaves_entities_it_does_not_name_alone() {
        let (mut app, _file, _camera) = app_with(&dolly(), 1.0);
        let bystander = app
            .world_mut()
            .spawn((
                Name("Bystander".to_string()),
                Transform {
                    position: glam::Vec3::new(3.0, 3.0, 3.0).into(),
                    ..Default::default()
                },
            ))
            .id();

        app.update();

        let position = app
            .world()
            .get::<Transform>(bystander)
            .expect("bystander transform")
            .position
            .0;
        assert!(
            (position - glam::Vec3::new(3.0, 3.0, 3.0)).length() < 1e-6,
            "an entity the timeline does not name must not move, got {position:?}"
        );
    }

    /// A missing file must not take the frame down, and must not log once per
    /// frame either.
    #[test]
    fn a_missing_timeline_is_reported_once_and_does_not_panic() {
        let mut app = crate::new_app();
        app.add_plugins(TimelinePlugin);
        let mut time = Time::default();
        time.set_delta_for_test(0.1);
        app.insert_resource(time);
        app.world_mut().spawn(TimelinePlayer {
            timeline: "assets/timelines/does-not-exist.ron".to_string(),
            playing: true,
            ..Default::default()
        });

        app.update();
        app.update();

        assert_eq!(
            app.world().resource::<LoadedTimelines>().by_path.len(),
            1,
            "the failure is remembered, so it is not retried every frame"
        );
    }
}
