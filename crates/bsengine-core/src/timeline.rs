//! A cutscene as data: camera, animation and event tracks over a duration.
//!
//! # Continuous tracks and discrete ones are different things
//!
//! A camera pose is a function of **where the playhead is**: ask for time 1.3
//! and there is an answer. An event is a function of **what the playhead
//! crossed** between two frames: "the cutscene ended" happens once, at a moment.
//!
//! Modelling an event as "is the playhead past it" is what makes a cutscene fire
//! its ending event sixty times a second, so the two kinds have separate entry
//! points here — [`evaluate`] for the continuous tracks, [`events_between`] and
//! [`animations_between`] for the discrete ones. The type signatures are the
//! distinction: one takes a time, the others take an interval.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};

/// One camera pose in a dolly track.
///
/// Carries `look_at` rather than a quaternion because that is what somebody
/// composing a shot actually knows, and because it is what the scene format
/// already uses for cameras.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub struct CameraKey {
    /// Seconds from the start of the timeline.
    pub time: f32,
    /// World position of the camera.
    pub position: [f32; 3],
    /// World point the camera faces.
    pub look_at: [f32; 3],
}

/// A hard cut to a named entity's viewpoint.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub struct ShotCut {
    /// Seconds from the start of the timeline.
    pub time: f32,
    /// Entity whose transform and field of view the rendering camera takes.
    ///
    /// An ordinary positioned entity — a marker for "the camera looks like this
    /// here". It needs no `Camera` component of its own, because the engine
    /// renders exactly one camera and a shot copies onto it rather than
    /// switching to it.
    pub entity: String,
}

/// A clip to start on an entity.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub struct AnimationKey {
    /// Seconds from the start of the timeline.
    pub time: f32,
    /// Name of the clip to start.
    pub clip: String,
}

/// Something for scripts to react to.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub struct EventKey {
    /// Seconds from the start of the timeline.
    pub time: f32,
    /// Name a script matches on.
    pub name: String,
}

/// One lane of a timeline.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Reflect)]
pub enum Track {
    /// A dolly: the camera moves smoothly between poses.
    Camera {
        /// Poses in time order.
        keys: Vec<CameraKey>,
    },
    /// Cuts: the camera jumps to a named entity's viewpoint.
    ///
    /// Coexists with [`Track::Camera`] deliberately — a dolly and a cut are both
    /// real cutscene grammar, and a cut wins at the instant it happens because a
    /// cut is a statement about that frame. Blending into a cut is not a cut.
    CameraShot {
        /// Cuts in time order.
        cuts: Vec<ShotCut>,
    },
    /// Clips started on one entity.
    Animation {
        /// Entity whose `AnimationPlayer` this drives.
        entity: String,
        /// Clip starts in time order.
        keys: Vec<AnimationKey>,
    },
    /// Named moments for scripts.
    Event {
        /// Events in time order.
        keys: Vec<EventKey>,
    },
}

/// A cutscene.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, Reflect)]
pub struct Timeline {
    /// Total length in seconds.
    pub duration: f32,
    /// The lanes, evaluated together.
    pub tracks: Vec<Track>,
}

/// Playback state for one timeline, on the entity that owns it.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component, Default)]
pub struct TimelinePlayer {
    /// Project-relative path of the timeline asset.
    pub timeline: String,
    /// Playback position in seconds.
    pub time: f32,
    /// Whether time is advancing.
    pub playing: bool,
    /// Rate multiplier.
    pub speed: f32,
}

impl Default for TimelinePlayer {
    fn default() -> Self {
        Self {
            timeline: String::new(),
            time: 0.0,
            // Not playing by default: a cutscene that started the moment its
            // scene loaded would take the camera away before the player did
            // anything. Scripts start it.
            playing: false,
            speed: 1.0,
        }
    }
}

/// Where the camera should be, as of some instant.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraPose {
    /// World position.
    pub position: [f32; 3],
    /// World point being faced.
    pub look_at: [f32; 3],
}

impl CameraPose {
    /// The rotation that faces `look_at` from `position`.
    ///
    /// Uses the same expression the scene loader uses for a camera's `look_at`
    /// (`bsengine-scene`, `Quat::from_rotation_arc(NEG_Z, dir)`). Deliberately
    /// not a second implementation of the same rule: two copies that drift give
    /// a camera that points one way when the scene places it and another when a
    /// timeline does, which reads as the timeline being wrong rather than as
    /// the duplication it is.
    pub fn rotation(&self) -> Quat {
        let direction = Vec3::from(self.look_at) - Vec3::from(self.position);
        if direction.length_squared() > 1e-10 {
            Quat::from_rotation_arc(Vec3::NEG_Z, direction.normalize())
        } else {
            Quat::IDENTITY
        }
    }
}

/// What the continuous tracks say at one instant.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Evaluation {
    /// The dolly pose, if a camera track covers this timeline at all.
    pub camera: Option<CameraPose>,
    /// The entity to cut to, if a cut has happened at or before this instant.
    ///
    /// Takes precedence over [`Self::camera`]: a cut is a statement about the
    /// frame it lands on.
    pub shot: Option<String>,
}

/// The continuous tracks at `time`.
///
/// Before the first key a camera track reports the first pose and after the last
/// it **holds** the last, rather than extrapolating. Same choice, for the same
/// reason, as snapshot interpolation in `bsengine-network`: a guess that
/// overshoots snaps visibly when the truth arrives, and a held pose is at least
/// stable while it is wrong.
pub fn evaluate(timeline: &Timeline, time: f32) -> Evaluation {
    let mut evaluation = Evaluation::default();

    for track in &timeline.tracks {
        match track {
            Track::Camera { keys } => {
                evaluation.camera = sample_camera(keys, time);
            }
            Track::CameraShot { cuts } => {
                // The latest cut at or before now. Cuts are stated in time
                // order, but `max_by` rather than `rfind` so an out-of-order
                // file still behaves sensibly instead of silently picking the
                // wrong shot.
                evaluation.shot = cuts
                    .iter()
                    .filter(|cut| cut.time <= time)
                    .max_by(|a, b| a.time.total_cmp(&b.time))
                    .map(|cut| cut.entity.clone());
            }
            Track::Animation { .. } | Track::Event { .. } => {}
        }
    }

    evaluation
}

/// The dolly pose at `time`, or `None` if the track has no keys.
fn sample_camera(keys: &[CameraKey], time: f32) -> Option<CameraPose> {
    let first = keys.first()?;
    let last = keys.last()?;

    if time <= first.time {
        return Some(CameraPose {
            position: first.position,
            look_at: first.look_at,
        });
    }
    if time >= last.time {
        return Some(CameraPose {
            position: last.position,
            look_at: last.look_at,
        });
    }

    let pair = keys
        .windows(2)
        .find(|w| time >= w[0].time && time <= w[1].time)?;
    let (a, b) = (&pair[0], &pair[1]);

    // Guarded because two keys can share a time in a hand-written file.
    // Dividing by that span puts a NaN into the camera matrix, which blanks the
    // screen and presents nowhere near this function.
    let span = b.time - a.time;
    let t = if span > 0.0 {
        ((time - a.time) / span).clamp(0.0, 1.0)
    } else {
        1.0
    };

    let lerp =
        |x: [f32; 3], y: [f32; 3]| -> [f32; 3] { Vec3::from(x).lerp(Vec3::from(y), t).into() };
    Some(CameraPose {
        position: lerp(a.position, b.position),
        // The direction is interpolated and the rotation rebuilt from it, rather
        // than slerping two rotations. A camera tracking a subject therefore
        // keeps pointing at it between keys, instead of drifting off the subject
        // and snapping back at the next one.
        look_at: lerp(a.look_at, b.look_at),
    })
}

/// Events crossed in `(from, to]`.
///
/// An interval rather than an instant, because an event happens once. Asking
/// "is the playhead past it" instead is what makes an ending event fire on every
/// frame after it.
pub fn events_between(timeline: &Timeline, from: f32, to: f32) -> Vec<String> {
    timeline
        .tracks
        .iter()
        .filter_map(|track| match track {
            Track::Event { keys } => Some(keys),
            _ => None,
        })
        .flatten()
        .filter(|key| key.time > from && key.time <= to)
        .map(|key| key.name.clone())
        .collect()
}

/// Clip starts crossed in `(from, to]`, as `(entity, clip)`.
pub fn animations_between(timeline: &Timeline, from: f32, to: f32) -> Vec<(String, String)> {
    timeline
        .tracks
        .iter()
        .filter_map(|track| match track {
            Track::Animation { entity, keys } => Some((entity, keys)),
            _ => None,
        })
        .flat_map(|(entity, keys)| {
            keys.iter()
                .filter(|key| key.time > from && key.time <= to)
                .map(move |key| (entity.clone(), key.clip.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// The headline, asserted as an exact fraction of a real distance -- not
    /// "it moved", which an implementation jumping to the last key satisfies,
    /// and not "strictly between", which item 52 learned is met by ~2e-12 of
    /// numerical noise.
    #[test]
    fn a_camera_track_is_interpolated_between_its_keys() {
        let camera = evaluate(&dolly(), 1.0)
            .camera
            .expect("a camera track covering t=1 must produce a pose");

        assert!(
            (camera.position[0] - 2.0).abs() < 0.01,
            "a quarter of the way from x=0 to x=8 is x=2, got {}",
            camera.position[0]
        );
    }

    /// The endpoints pin the direction of the blend: interpolating backwards
    /// passes the midpoint but not these.
    #[test]
    fn a_camera_track_lands_on_its_keys() {
        assert!((evaluate(&dolly(), 0.0).camera.expect("t=0").position[0] - 0.0).abs() < 0.01);
        assert!((evaluate(&dolly(), 4.0).camera.expect("t=4").position[0] - 8.0).abs() < 0.01);
    }

    #[test]
    fn past_the_end_a_camera_track_holds_its_last_key() {
        assert!((evaluate(&dolly(), 99.0).camera.expect("held").position[0] - 8.0).abs() < 1e-6);
    }

    /// Both halves. "It ended up at the shot" alone passes for a track that cut
    /// immediately; what says the cut is *instantaneous* is that the frame
    /// before it is still on the dolly path.
    #[test]
    fn a_shot_cut_wins_at_its_instant_and_not_before() {
        let mut timeline = dolly();
        timeline.tracks.push(Track::CameraShot {
            cuts: vec![ShotCut {
                time: 2.0,
                entity: "CloseUp".to_string(),
            }],
        });

        let before = evaluate(&timeline, 1.9);
        assert_eq!(before.shot, None, "no cut has happened yet");
        let dollying_at = before.camera.expect("still dollying").position[0];
        assert!(
            (dollying_at - 3.8).abs() < 0.01,
            "and the camera is still on the interpolated path, got {dollying_at}"
        );

        assert_eq!(
            evaluate(&timeline, 2.0).shot.as_deref(),
            Some("CloseUp"),
            "the cut lands exactly on its own time"
        );
    }

    /// A cutscene that fires its ending event sixty times a second is the
    /// symptom this prevents, and it is the assertion most likely to be quietly
    /// wrong.
    #[test]
    fn an_event_fires_once_when_crossed_and_not_afterwards() {
        let timeline = Timeline {
            duration: 5.0,
            tracks: vec![Track::Event {
                keys: vec![EventKey {
                    time: 2.0,
                    name: "done".to_string(),
                }],
            }],
        };

        assert!(events_between(&timeline, 1.0, 1.9).is_empty(), "not before");
        assert_eq!(
            events_between(&timeline, 1.9, 2.1),
            vec!["done".to_string()],
            "on the frame that crosses it"
        );
        assert!(
            events_between(&timeline, 2.1, 4.0).is_empty(),
            "and never again -- an event behind the playhead is not an event now"
        );
    }

    #[test]
    fn an_animation_key_fires_once_when_crossed() {
        let timeline = Timeline {
            duration: 5.0,
            tracks: vec![Track::Animation {
                entity: "Fox".to_string(),
                keys: vec![AnimationKey {
                    time: 1.0,
                    clip: "Run".to_string(),
                }],
            }],
        };

        assert!(animations_between(&timeline, 0.0, 0.9).is_empty());
        assert_eq!(
            animations_between(&timeline, 0.9, 1.1),
            vec![("Fox".to_string(), "Run".to_string())]
        );
        assert!(animations_between(&timeline, 1.1, 3.0).is_empty());
    }

    #[test]
    fn an_empty_timeline_evaluates_to_nothing() {
        let empty = Timeline {
            duration: 1.0,
            tracks: Vec::new(),
        };
        assert!(evaluate(&empty, 0.5).camera.is_none());
        assert!(evaluate(&empty, 0.5).shot.is_none());
    }

    /// A NaN here reaches the camera matrix and blanks the screen, which
    /// presents nowhere near this function.
    #[test]
    fn two_keys_at_one_time_do_not_produce_a_nan() {
        let timeline = Timeline {
            duration: 1.0,
            tracks: vec![Track::Camera {
                keys: vec![
                    CameraKey {
                        time: 1.0,
                        position: [0.0, 0.0, 0.0],
                        look_at: [0.0, 0.0, -1.0],
                    },
                    CameraKey {
                        time: 1.0,
                        position: [5.0, 0.0, 0.0],
                        look_at: [0.0, 0.0, -1.0],
                    },
                ],
            }],
        };
        assert!(evaluate(&timeline, 1.0).camera.expect("in range").position[0].is_finite());
    }

    /// The rotation must agree with what the scene loader produces for the same
    /// look_at, since a scene and a timeline can place the same camera.
    #[test]
    fn a_pose_faces_what_it_looks_at() {
        let pose = CameraPose {
            position: [0.0, 0.0, 5.0],
            look_at: [0.0, 0.0, 0.0],
        };
        let forward = pose.rotation() * Vec3::NEG_Z;

        assert!(
            (forward - Vec3::new(0.0, 0.0, -1.0)).length() < 1e-5,
            "a camera at z=5 looking at the origin faces -Z, got {forward:?}"
        );
    }

    /// Degenerate input must not produce a NaN rotation.
    #[test]
    fn looking_at_your_own_position_is_the_identity_not_a_nan() {
        let pose = CameraPose {
            position: [1.0, 2.0, 3.0],
            look_at: [1.0, 2.0, 3.0],
        };
        assert!(pose.rotation().is_finite());
    }

    /// Authored by hand and by agents, so it has to round-trip.
    #[test]
    fn a_timeline_round_trips_through_ron() {
        let text = ron::to_string(&dolly()).expect("serialise");
        let back: Timeline = ron::from_str(&text).expect("parse");

        assert_eq!(back, dolly());
    }
}
