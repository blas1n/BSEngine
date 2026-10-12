//! The browser backend, in a real browser: `wasm-bindgen-test-runner` with a
//! WebDriver (`MSEDGEDRIVER`/`CHROMEDRIVER`). The same file the native
//! backends are tested with, given to the page as a Blob URL -- 64x48, red
//! over blue for a second, then green, two seconds long.
#![cfg(target_arch = "wasm32")]

use bevy_ecs::prelude::Entity;
use bsengine_core::{VideoFrames, VideoPlayer, VideoStatus};
use bsengine_video::{Playbacks, VideoPlugin};
use wasm_bindgen::JsCast;
use wasm_bindgen_test::*;

wasm_bindgen_test_configure!(run_in_browser);

const VIDEO: &[u8] = include_bytes!("assets/two_colors.mp4");

fn blob_url() -> String {
    let array = js_sys::Uint8Array::from(VIDEO);
    let options = web_sys::BlobPropertyBag::new();
    options.set_type("video/mp4");
    let blob = web_sys::Blob::new_with_u8_array_sequence_and_options(
        &js_sys::Array::of1(&array),
        &options,
    )
    .unwrap();
    web_sys::Url::create_object_url_with_blob(&blob).unwrap()
}

/// Waits `ms` of real time: the element plays in real time, not on a clock
/// a test can step.
async fn sleep(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        web_sys::window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(resolve.unchecked_ref(), ms)
            .unwrap();
    });
    wasm_bindgen_futures::JsFuture::from(promise).await.unwrap();
}

fn app_with(player: VideoPlayer) -> (bevy_app::App, Entity) {
    let mut app = bsengine_app::new_app();
    app.add_plugins(VideoPlugin);
    let e = app.world_mut().spawn(player).id();
    (app, e)
}

fn player(app: &bevy_app::App, e: Entity) -> VideoPlayer {
    app.world().get::<VideoPlayer>(e).unwrap().clone()
}

/// Updates every 30 ms until `done` says so or `seconds` run out; whether it
/// did.
async fn until(
    app: &mut bevy_app::App,
    seconds: f64,
    done: impl Fn(&bevy_app::App) -> bool,
) -> bool {
    for _ in 0..(seconds * 1000.0 / 30.0) as usize {
        app.update();
        if done(app) {
            return true;
        }
        sleep(30).await;
    }
    false
}

fn top(app: &bevy_app::App, name: &str) -> [u8; 3] {
    let frame = &app.world().resource::<VideoFrames>().0[name];
    let i = ((4 * frame.width + frame.width / 2) * 4) as usize;
    [frame.rgba[i], frame.rgba[i + 1], frame.rgba[i + 2]]
}

fn is(rgb: [u8; 3], expected: [u8; 3]) -> bool {
    rgb.iter()
        .zip(expected)
        .all(|(a, e)| (i32::from(*a) - i32::from(e)).abs() < 60)
}

/// The element's pictures reach the named texture at their size and in
/// their colours, in time, and the end is reported.
#[wasm_bindgen_test]
async fn a_video_plays_through_the_browsers_element() {
    let path = blob_url();
    let (mut app, e) = app_with(VideoPlayer {
        path: path.clone(),
        ..Default::default()
    });
    assert!(
        until(&mut app, 10.0, |a| player(a, e).status
            == VideoStatus::Playing)
        .await,
        "it starts: {:?}",
        player(&app, e).status
    );
    assert!(until(&mut app, 5.0, |a| player(a, e).time > 0.3).await);
    let frame = &app.world().resource::<VideoFrames>().0[&path];
    assert_eq!((frame.width, frame.height), (64, 48));
    assert!(player(&app, e).time < 1.0);
    assert!(
        is(top(&app, &path), [255, 0, 0]),
        "red first: {:?}",
        top(&app, &path)
    );
    assert!(until(&mut app, 5.0, |a| player(a, e).time > 1.3).await);
    assert!(
        is(top(&app, &path), [0, 255, 0]),
        "then green: {:?}",
        top(&app, &path)
    );
    assert!(
        until(&mut app, 5.0, |a| player(a, e).status == VideoStatus::Ended).await,
        "and it ends: {:?}",
        player(&app, e).status
    );
}

/// Pausing holds the picture; resuming goes on.
#[wasm_bindgen_test]
async fn pausing_holds_the_picture() {
    let (mut app, e) = app_with(VideoPlayer {
        path: blob_url(),
        ..Default::default()
    });
    assert!(until(&mut app, 10.0, |a| player(a, e).time > 0.3).await);
    app.world_mut().get_mut::<VideoPlayer>(e).unwrap().playing = false;
    until(&mut app, 0.3, |_| false).await;
    let held = player(&app, e).time;
    until(&mut app, 0.6, |_| false).await;
    assert!(
        (player(&app, e).time - held).abs() < 0.05,
        "paused: {held} -> {}",
        player(&app, e).time
    );
    app.world_mut().get_mut::<VideoPlayer>(e).unwrap().playing = true;
    assert!(
        until(&mut app, 3.0, |a| player(a, e).time > held + 0.3).await,
        "resumed"
    );
}

/// Looping, it never ends, and its time keeps counting up past the length
/// -- as natively, though the element's own clock wraps to 0.
#[wasm_bindgen_test]
async fn a_looping_video_counts_on() {
    let (mut app, e) = app_with(VideoPlayer {
        path: blob_url(),
        looping: true,
        texture: "screen".into(),
        ..Default::default()
    });
    assert!(
        until(&mut app, 15.0, |a| player(a, e).time > 2.3).await,
        "past one loop: {}",
        player(&app, e).time
    );
    assert_eq!(player(&app, e).status, VideoStatus::Playing);
    assert!(app
        .world()
        .resource::<VideoFrames>()
        .0
        .contains_key("screen"));
}

/// A file the page cannot load fails with a reason; despawning a player
/// drops its element.
#[wasm_bindgen_test]
async fn a_missing_file_fails_and_despawning_stops() {
    let (mut app, e) = app_with(VideoPlayer {
        path: "no/such/video.mp4".into(),
        ..Default::default()
    });
    assert!(
        until(&mut app, 10.0, |a| matches!(
            player(a, e).status,
            VideoStatus::Failed(_)
        ))
        .await,
        "fails: {:?}",
        player(&app, e).status
    );
    assert!(app.world().resource::<Playbacks>().is_playing(e));
    app.world_mut().despawn(e);
    app.update();
    assert!(!app.world().resource::<Playbacks>().is_playing(e));
}
