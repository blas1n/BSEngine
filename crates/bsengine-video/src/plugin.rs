//! Playing [`VideoPlayer`]s: a decoding thread per video, pictures paced to
//! the sound.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};

use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_audio::stream::AudioStream;
use bsengine_audio::world::AudioWorld;
use bsengine_core::{ProjectDir, Time, VideoFrameData, VideoFrames, VideoPlayer, VideoStatus};

use crate::{Packet, VideoFrame, VideoInfo};

/// Plays every entity's [`VideoPlayer`].
///
/// Each video is decoded on a thread of its own, a little ahead of what is
/// shown. Its sound goes to the mixer as an [`AudioStream`], and how much of
/// that has been heard is the clock: a picture is shown once the sound has
/// reached its time. Unity, Unreal and Godot all sync video to audio this
/// way -- the sound card's clock cannot drift, and a picture shown a frame
/// late is invisible where a sound a frame late is a click. With no sound
/// (a silent video, or no audio device) the game clock stands in.
pub struct VideoPlugin;

impl Plugin for VideoPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VideoFrames>();
        app.init_resource::<Playbacks>();
        app.register_type::<VideoPlayer>();
        app.add_systems(Update, drive_video_players);
    }
}

/// How many pictures may wait decoded ahead of the one shown.
const PICTURES_AHEAD: usize = 12;
/// How much sound is kept queued for the mixer, in seconds.
const SOUND_AHEAD: f64 = 0.5;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
enum Message {
    Opened(VideoInfo),
    Failed(String),
    Packet(Packet),
}

/// One playing video.
struct Playback {
    path: String,
    messages: Mutex<Receiver<Message>>,
    stop: Arc<AtomicBool>,
    looping: Arc<AtomicBool>,
    /// The sound, when there is some and a device to play it on.
    sound: Option<Arc<AudioStream>>,
    /// The game-clock stand-in, when there is no sound.
    clock: f64,
    pictures: VecDeque<VideoFrame>,
    opened: bool,
    ended: bool,
    serial: u64,
}

impl Drop for Playback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(sound) = &self.sound {
            sound.stop();
        }
    }
}

/// Every entity's playback.
#[derive(Resource, Default)]
pub struct Playbacks(HashMap<Entity, Playback>);

impl Playbacks {
    /// Whether `entity`'s video is being played -- has a decoding thread.
    pub fn is_playing(&self, entity: Entity) -> bool {
        self.0.contains_key(&entity)
    }

    /// Whether `entity`'s video has its sound on the mixer, so its pictures
    /// are paced by the sound card rather than the game clock.
    pub fn is_paced_by_sound(&self, entity: Entity) -> bool {
        self.0.get(&entity).is_some_and(|p| p.sound.is_some())
    }
}

/// The file to hand the platform's decoder: the project's own, or -- in a
/// packaged build, where it is inside the archive and an OS decoder can only
/// open files -- a copy of it written out once to the temporary directory.
fn local_file(project_dir: Option<&ProjectDir>, path: &str) -> Result<PathBuf, String> {
    let on_disk = PathBuf::from(bsengine_core::resolve_project_path(project_dir, path));
    if on_disk.is_file() {
        return Ok(on_disk);
    }
    if let Some((pak, dir)) = bsengine_asset::pak_source::archive() {
        if let Some(bytes) = pak.get(&bsengine_asset::pak_source::archive_key(
            &on_disk.to_string_lossy(),
            dir,
        )) {
            let name = path.replace(['/', '\\', ':'], "_");
            let copy =
                std::env::temp_dir().join(format!("bsengine-video-{}-{name}", std::process::id()));
            if !copy.is_file() {
                std::fs::write(&copy, bytes)
                    .map_err(|e| format!("cannot unpack {path} to play it: {e}"))?;
            }
            return Ok(copy);
        }
    }
    Err(format!("{path}: no such file"))
}

fn start(project_dir: Option<&ProjectDir>, path: &str, looping: bool) -> Playback {
    let (sender, receiver) = sync_channel::<Message>(PICTURES_AHEAD * 4);
    let stop = Arc::new(AtomicBool::new(false));
    let looping = Arc::new(AtomicBool::new(looping));
    let file = local_file(project_dir, path);
    // A page has no threads to decode on, and no decoder of this crate's
    // yet: the browser's own video element is the web backend still to come.
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (file, &stop, &looping);
        let _ = sender.send(Message::Failed(format!(
            "cannot play {path}: video playback has no decoder in the browser yet"
        )));
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let (stop, looping) = (stop.clone(), looping.clone());
        std::thread::Builder::new()
            .name(format!("video {path}"))
            .spawn(move || {
                // Opened here, on this thread: the platform decoders are COM
                // and Objective-C objects, best kept on the thread that made
                // them.
                let mut decoder = match file.and_then(|f| crate::open(&f)) {
                    Ok(d) => d,
                    Err(e) => {
                        let _ = sender.send(Message::Failed(e));
                        return;
                    }
                };
                let info = decoder.info().clone();
                if sender.send(Message::Opened(info.clone())).is_err() {
                    return;
                }
                // Each loop's times continue from the last, so the clock
                // never runs backwards.
                let mut offset = 0.0;
                let mut last_pts: f64 = 0.0;
                while !stop.load(Ordering::Acquire) {
                    let packet = match decoder.next_packet() {
                        Ok(p) => p,
                        Err(e) => {
                            let _ = sender.send(Message::Failed(e));
                            return;
                        }
                    };
                    let packet = match packet {
                        Packet::End if looping.load(Ordering::Acquire) => {
                            offset += info.duration.unwrap_or(last_pts).max(last_pts);
                            if decoder.rewind().is_err() {
                                let _ = sender.send(Message::Packet(Packet::End));
                                return;
                            }
                            continue;
                        }
                        Packet::Video(mut f) => {
                            last_pts = last_pts.max(f.pts);
                            f.pts += offset;
                            Packet::Video(f)
                        }
                        Packet::Audio(mut a) => {
                            a.pts += offset;
                            Packet::Audio(a)
                        }
                        Packet::End => {
                            let _ = sender.send(Message::Packet(Packet::End));
                            return;
                        }
                    };
                    if sender.send(Message::Packet(packet)).is_err() {
                        return;
                    }
                }
            })
            .expect("spawn a video thread");
    }
    Playback {
        path: path.to_string(),
        messages: Mutex::new(receiver),
        stop,
        looping,
        sound: None,
        clock: 0.0,
        pictures: VecDeque::new(),
        opened: false,
        ended: false,
        serial: 0,
    }
}

fn drive_video_players(
    mut players: Query<(Entity, &mut VideoPlayer)>,
    mut playbacks: ResMut<Playbacks>,
    mut frames: ResMut<VideoFrames>,
    mut audio: Option<ResMut<AudioWorld>>,
    time: Option<Res<Time>>,
    project_dir: Option<Res<ProjectDir>>,
) {
    let dt = time.map_or(0.0, |t| f64::from(t.delta_seconds));
    // A despawned player, or one whose component was removed, stops.
    let alive: Vec<Entity> = players.iter().map(|(e, _)| e).collect();
    playbacks.0.retain(|e, _| alive.contains(e));

    for (entity, mut player) in players.iter_mut() {
        if player.path.is_empty() {
            playbacks.0.remove(&entity);
            continue;
        }
        let restart = playbacks
            .0
            .get(&entity)
            .is_none_or(|p| p.path != player.path);
        if restart {
            playbacks.0.insert(
                entity,
                start(project_dir.as_deref(), &player.path, player.looping),
            );
            player.status = VideoStatus::Loading;
            player.time = 0.0;
        }
        let playback = playbacks.0.get_mut(&entity).expect("inserted above");
        playback.looping.store(player.looping, Ordering::Release);

        // Take what has been decoded, keeping a little ahead of the clock:
        // enough pictures to never miss one, enough sound to never run dry.
        let messages = playback
            .messages
            .get_mut()
            .unwrap_or_else(|e| e.into_inner());
        while playback.pictures.len() < PICTURES_AHEAD
            && playback
                .sound
                .as_ref()
                .is_none_or(|s| s.buffered() < SOUND_AHEAD)
        {
            match messages.try_recv() {
                Ok(Message::Opened(info)) => {
                    playback.opened = true;
                    player.status = VideoStatus::Playing;
                    if let (Some(format), Some(world)) = (info.audio, audio.as_deref_mut()) {
                        let sound = AudioStream::new(format.sample_rate, format.channels);
                        let bus = (!player.bus.is_empty()).then_some(player.bus.as_str());
                        if world.play_stream(bus, sound.clone()) {
                            playback.sound = Some(sound);
                        }
                    }
                }
                Ok(Message::Failed(e)) => {
                    tracing::warn!("[video] {}: {e}", player.path);
                    player.status = VideoStatus::Failed(e);
                    break;
                }
                Ok(Message::Packet(Packet::Video(f))) => playback.pictures.push_back(f),
                Ok(Message::Packet(Packet::Audio(a))) => {
                    if let Some(sound) = &playback.sound {
                        sound.push(&a.samples);
                    }
                }
                Ok(Message::Packet(Packet::End)) => {
                    playback.ended = true;
                    break;
                }
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        if !playback.opened {
            continue;
        }

        // The clock: the sound heard so far, or the game's time.
        if let Some(sound) = &playback.sound {
            sound.set_paused(!player.playing);
            sound.set_volume(player.volume);
        }
        let now = match &playback.sound {
            Some(sound) => sound.played(),
            None => {
                if player.playing {
                    playback.clock += dt;
                }
                playback.clock
            }
        };

        // The latest picture whose time has come.
        let mut shown = None;
        while playback.pictures.front().is_some_and(|f| f.pts <= now) {
            shown = playback.pictures.pop_front();
        }
        if let Some(frame) = shown {
            playback.serial += 1;
            player.time = frame.pts;
            frames.0.insert(
                player.texture_name().to_string(),
                VideoFrameData {
                    width: frame.width,
                    height: frame.height,
                    rgba: Arc::new(frame.rgba),
                    serial: playback.serial,
                },
            );
        }
        if playback.ended && playback.pictures.is_empty() && player.status == VideoStatus::Playing {
            player.status = VideoStatus::Ended;
        }
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn asset() -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/assets/two_colors.mp4")
            .to_string_lossy()
            .into_owned()
    }

    /// A player app on a fixed 0.1 s clock, with no audio device -- the game
    /// clock paces it, deterministically.
    fn app_with(player: VideoPlayer) -> (bevy_app::App, Entity) {
        let mut app = bsengine_app::new_app();
        app.add_plugins(bsengine_app::TimePlugin);
        app.insert_resource(Time::fixed(0.1));
        app.add_plugins(VideoPlugin);
        let e = app.world_mut().spawn(player).id();
        (app, e)
    }

    /// Updates until the clock reaches `seconds` -- with a pause between
    /// frames so the decoding thread keeps ahead of it, as a real frame's
    /// length would.
    fn play_to(app: &mut bevy_app::App, seconds: f64) {
        let mut t = 0.0;
        while t < seconds {
            std::thread::sleep(std::time::Duration::from_millis(15));
            app.update();
            t += 0.1;
        }
    }

    fn player(app: &bevy_app::App, e: Entity) -> VideoPlayer {
        app.world().get::<VideoPlayer>(e).unwrap().clone()
    }

    /// The top-middle pixel of what is showing under `name`.
    fn top(app: &bevy_app::App, name: &str) -> [u8; 3] {
        let frame = &app.world().resource::<VideoFrames>().0[name];
        let i = ((4 * frame.width + frame.width / 2) * 4) as usize;
        [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
    }

    fn is(rgb: [u8; 3], expected: [u8; 3]) -> bool {
        rgb.iter()
            .zip(expected)
            .all(|(a, e)| (i32::from(*a) - i32::from(e)).abs() < 50)
    }

    /// Playing: the picture under the video's own path is the file's at the
    /// time the clock says -- red in the first second, green in the second --
    /// and at the end it says so.
    #[test]
    fn a_video_plays_its_pictures_in_time_and_ends() {
        let path = asset();
        let (mut app, e) = app_with(VideoPlayer {
            path: path.clone(),
            ..Default::default()
        });
        play_to(&mut app, 0.5);
        assert_eq!(player(&app, e).status, VideoStatus::Playing);
        assert!(
            is(top(&app, &path), [255, 0, 0]),
            "red first: {:?}",
            top(&app, &path)
        );
        let early = player(&app, e).time;
        assert!(
            (0.3..=0.6).contains(&early),
            "showing about 0.5 s in: {early}"
        );
        play_to(&mut app, 1.0);
        assert!(
            is(top(&app, &path), [0, 255, 0]),
            "green after a second: {:?}",
            top(&app, &path)
        );
        play_to(&mut app, 1.0);
        assert_eq!(player(&app, e).status, VideoStatus::Ended);
    }

    /// Paused, the picture and its time stand still; resumed, they go on.
    #[test]
    fn pausing_holds_the_picture() {
        let (mut app, e) = app_with(VideoPlayer {
            path: asset(),
            ..Default::default()
        });
        play_to(&mut app, 0.5);
        app.world_mut().get_mut::<VideoPlayer>(e).unwrap().playing = false;
        let held = player(&app, e).time;
        play_to(&mut app, 0.8);
        assert_eq!(player(&app, e).time, held, "paused: no new picture");
        app.world_mut().get_mut::<VideoPlayer>(e).unwrap().playing = true;
        play_to(&mut app, 0.5);
        assert!(player(&app, e).time > held + 0.3, "resumed");
    }

    /// Looping: past the end it starts over -- red again -- and its time
    /// keeps counting up rather than jumping back.
    #[test]
    fn a_looping_video_starts_over() {
        let path = asset();
        let (mut app, e) = app_with(VideoPlayer {
            path: path.clone(),
            looping: true,
            texture: "screen".into(),
            ..Default::default()
        });
        play_to(&mut app, 2.5);
        let p = player(&app, e);
        assert_eq!(p.status, VideoStatus::Playing, "never ends");
        assert!(p.time > 2.0, "time counts on: {}", p.time);
        assert!(
            is(top(&app, "screen"), [255, 0, 0]),
            "red again: {:?}",
            top(&app, "screen")
        );
        assert!(
            !app.world().resource::<VideoFrames>().0.contains_key(&path),
            "shown under the texture name it was given, not its path"
        );
    }

    /// A file that cannot be played says why, and nothing panics.
    #[test]
    fn a_missing_file_fails_with_a_reason() {
        let (mut app, e) = app_with(VideoPlayer {
            path: "assets/no/such/video.mp4".into(),
            ..Default::default()
        });
        play_to(&mut app, 0.3);
        match player(&app, e).status {
            VideoStatus::Failed(why) => assert!(why.contains("no such file"), "{why}"),
            other => panic!("expected a failure, got {other:?}"),
        }
    }

    /// Despawning a player stops its playback; changing its path starts the
    /// new video from the beginning.
    #[test]
    fn despawning_stops_and_a_new_path_restarts() {
        let (mut app, e) = app_with(VideoPlayer {
            path: asset(),
            ..Default::default()
        });
        play_to(&mut app, 1.3);
        assert!(player(&app, e).time > 1.0);
        let copy = std::env::temp_dir().join("bsengine-video-restart.mp4");
        std::fs::copy(asset(), &copy).unwrap();
        app.world_mut().get_mut::<VideoPlayer>(e).unwrap().path =
            copy.to_string_lossy().into_owned();
        play_to(&mut app, 0.3);
        assert!(
            player(&app, e).time < 0.5,
            "from the start: {}",
            player(&app, e).time
        );
        assert!(app.world().resource::<Playbacks>().is_playing(e));
        app.world_mut().despawn(e);
        app.update();
        assert!(!app.world().resource::<Playbacks>().is_playing(e));
        let _ = std::fs::remove_file(copy);
    }
}
