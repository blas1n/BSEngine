//! The browser: a `<video>` element per player.
//!
//! A page has no threads to decode on and needs none: the browser's own
//! video element decodes -- in hardware, whatever formats that browser plays
//! (H.264/AAC in MP4 everywhere) -- and plays the sound itself, keeping the
//! two in step. Unity's WebGL player does the same. So here the element *is*
//! the player: its `currentTime` is the clock, each new picture is drawn into
//! a canvas and read back as the RGBA a native backend would have produced,
//! and the rest -- the named texture, the material showing it -- is unchanged.
//!
//! The sound bypasses the mixer (an element plays to the page's output), so
//! `bus` has no effect on the web; `volume` sets the element's.
//!
//! Browsers refuse to start sound before the person has interacted with the
//! page. A player that is refused starts muted instead -- pictures move, as
//! they must for an intro to play -- and is unmuted by the first click, touch
//! or key, as the engine's audio is (`web.rs` in the runtime).

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::sync::Arc;

use bevy_ecs::prelude::*;
use bsengine_core::{ProjectDir, VideoFrameData, VideoFrames, VideoPlayer, VideoStatus};
use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement, HtmlVideoElement};

/// `HTMLMediaElement.HAVE_CURRENT_DATA`: a picture can be drawn.
const HAVE_CURRENT_DATA: u16 = 2;

struct WebPlayback {
    path: String,
    video: HtmlVideoElement,
    canvas: HtmlCanvasElement,
    context: CanvasRenderingContext2d,
    /// A Blob URL this playback made, revoked when it ends.
    object_url: Option<String>,
    /// The element's time at the last picture taken, to take each once.
    last_time: f64,
    /// Times the element has wrapped around while looping, so the time
    /// reported keeps counting up as natively.
    loops: u32,
    serial: u64,
}

impl Drop for WebPlayback {
    fn drop(&mut self) {
        let _ = self.video.pause();
        self.video.set_src("");
        if let Some(url) = &self.object_url {
            let _ = web_sys::Url::revoke_object_url(url);
        }
    }
}

thread_local! {
    static PLAYBACKS: RefCell<HashMap<Entity, WebPlayback>> = RefCell::new(HashMap::new());
    /// Elements started muted because sound was refused, waiting for the
    /// first input to be unmuted.
    static MUTED_FOR_AUTOPLAY: RefCell<Vec<HtmlVideoElement>> = const { RefCell::new(Vec::new()) };
    static UNMUTE_LISTENING: Cell<bool> = const { Cell::new(false) };
}

/// Whether `entity` has a video element.
pub(crate) fn is_playing(entity: Entity) -> bool {
    PLAYBACKS.with(|p| p.borrow().contains_key(&entity))
}

/// Where the element loads from: a Blob of the archive's bytes in a packaged
/// build, the path itself (a URL relative to the page, or already a URL)
/// otherwise.
fn source(project_dir: Option<&ProjectDir>, path: &str) -> (String, Option<String>) {
    let resolved = bsengine_core::resolve_project_path(project_dir, path);
    if let Some((pak, dir)) = bsengine_asset::pak_source::archive() {
        if let Some(bytes) = pak.get(&bsengine_asset::pak_source::archive_key(&resolved, dir)) {
            let array = js_sys::Uint8Array::from(bytes);
            let parts = js_sys::Array::of1(&array);
            let options = web_sys::BlobPropertyBag::new();
            options.set_type("video/mp4");
            if let Ok(url) = web_sys::Blob::new_with_u8_array_sequence_and_options(&parts, &options)
                .and_then(|blob| web_sys::Url::create_object_url_with_blob(&blob))
            {
                return (url.clone(), Some(url));
            }
        }
    }
    (resolved, None)
}

fn start(project_dir: Option<&ProjectDir>, path: &str) -> Result<WebPlayback, String> {
    let document = web_sys::window()
        .and_then(|w| w.document())
        .ok_or("no document")?;
    let video: HtmlVideoElement = document
        .create_element("video")
        .map_err(describe)?
        .dyn_into()
        .map_err(|_| "not a video element".to_string())?;
    let canvas: HtmlCanvasElement = document
        .create_element("canvas")
        .map_err(describe)?
        .dyn_into()
        .map_err(|_| "not a canvas".to_string())?;
    // Read back every frame: tell the canvas, so it keeps its pixels where
    // reading them is cheap.
    let options = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&options, &"willReadFrequently".into(), &JsValue::TRUE);
    let context: CanvasRenderingContext2d = canvas
        .get_context_with_context_options("2d", &options)
        .map_err(describe)?
        .ok_or("no 2d context")?
        .dyn_into()
        .map_err(|_| "not a 2d context".to_string())?;
    let (url, object_url) = source(project_dir, path);
    video.set_preload("auto");
    // Inline on phones, rather than taking over the screen.
    let _ = video.set_attribute("playsinline", "");
    video.set_src(&url);
    Ok(WebPlayback {
        path: path.to_string(),
        video,
        canvas,
        context,
        object_url,
        last_time: -1.0,
        loops: 0,
        serial: 0,
    })
}

fn describe(e: JsValue) -> String {
    e.as_string().unwrap_or_else(|| format!("{e:?}"))
}

/// Starts `video` playing -- with sound if the browser allows it, muted (and
/// unmuted on the first input) if it refuses.
fn play(video: &HtmlVideoElement) {
    let Ok(promise) = video.play() else {
        return;
    };
    let video = video.clone();
    wasm_bindgen_futures::spawn_local(async move {
        if wasm_bindgen_futures::JsFuture::from(promise).await.is_err() && !video.muted() {
            video.set_muted(true);
            let _ = video.play();
            MUTED_FOR_AUTOPLAY.with(|m| m.borrow_mut().push(video));
            unmute_on_first_input();
        }
    });
}

fn unmute_on_first_input() {
    if UNMUTE_LISTENING.with(|l| l.replace(true)) {
        return;
    }
    let Some(window) = web_sys::window() else {
        return;
    };
    let unmute = Closure::<dyn FnMut()>::new(|| {
        MUTED_FOR_AUTOPLAY.with(|m| {
            for video in m.borrow_mut().drain(..) {
                video.set_muted(false);
            }
        });
    });
    for event in ["pointerdown", "keydown", "touchend"] {
        let _ = window.add_event_listener_with_callback(event, unmute.as_ref().unchecked_ref());
    }
    // Lives as long as the page.
    unmute.forget();
}

/// Plays every [`VideoPlayer`] through a video element.
pub(crate) fn drive_web_players(
    mut players: Query<(Entity, &mut VideoPlayer)>,
    mut frames: ResMut<VideoFrames>,
    project_dir: Option<Res<ProjectDir>>,
) {
    let alive: Vec<Entity> = players.iter().map(|(e, _)| e).collect();
    PLAYBACKS.with(|p| p.borrow_mut().retain(|e, _| alive.contains(e)));

    for (entity, mut player) in players.iter_mut() {
        if player.path.is_empty() {
            PLAYBACKS.with(|p| p.borrow_mut().remove(&entity));
            continue;
        }
        let restart = PLAYBACKS.with(|p| {
            p.borrow()
                .get(&entity)
                .is_none_or(|playback| playback.path != player.path)
        });
        if restart {
            match start(project_dir.as_deref(), &player.path) {
                Ok(playback) => {
                    PLAYBACKS.with(|p| p.borrow_mut().insert(entity, playback));
                    player.status = VideoStatus::Loading;
                    player.time = 0.0;
                }
                Err(e) => {
                    player.status = VideoStatus::Failed(e);
                    continue;
                }
            }
        }
        PLAYBACKS.with(|p| {
            let mut playbacks = p.borrow_mut();
            let Some(playback) = playbacks.get_mut(&entity) else {
                return;
            };
            let video = &playback.video;
            if let Some(error) = video.error() {
                let why = format!(
                    "cannot play {}: the browser could not load or decode it (media error {})",
                    player.path,
                    error.code()
                );
                if player.status != VideoStatus::Failed(why.clone()) {
                    tracing::warn!("[video] {why}");
                    player.status = VideoStatus::Failed(why);
                }
                return;
            }
            video.set_loop(player.looping);
            video.set_volume(f64::from(player.volume.clamp(0.0, 1.0)));
            if player.playing && video.paused() && !video.ended() {
                play(video);
            } else if !player.playing && !video.paused() {
                let _ = video.pause();
            }
            if video.ready_state() < HAVE_CURRENT_DATA {
                return;
            }
            if player.status == VideoStatus::Loading {
                player.status = VideoStatus::Playing;
            }
            let now = video.current_time();
            if (now - playback.last_time).abs() > 1e-6 {
                if now + 0.5 < playback.last_time {
                    // Wrapped around: looping.
                    playback.loops += 1;
                }
                let (w, h) = (video.video_width(), video.video_height());
                if w > 0 && h > 0 {
                    if playback.canvas.width() != w || playback.canvas.height() != h {
                        playback.canvas.set_width(w);
                        playback.canvas.set_height(h);
                    }
                    let drawn = playback
                        .context
                        .draw_image_with_html_video_element(video, 0.0, 0.0)
                        .and_then(|()| {
                            playback
                                .context
                                .get_image_data(0.0, 0.0, f64::from(w), f64::from(h))
                        });
                    if let Ok(image) = drawn {
                        playback.serial += 1;
                        playback.last_time = now;
                        let duration = video.duration();
                        let duration = if duration.is_finite() { duration } else { 0.0 };
                        player.time = f64::from(playback.loops) * duration + now;
                        frames.0.insert(
                            player.texture_name().to_string(),
                            VideoFrameData {
                                width: w,
                                height: h,
                                rgba: Arc::new(image.data().0),
                                serial: playback.serial,
                            },
                        );
                    }
                }
            }
            if video.ended() && !player.looping && player.status == VideoStatus::Playing {
                player.status = VideoStatus::Ended;
            }
        });
    }
}
