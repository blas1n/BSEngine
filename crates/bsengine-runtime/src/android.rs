//! The Android build's entry point.
//!
//! An Android app does not start at `main`. The system starts the activity
//! the APK's manifest names -- `android.app.NativeActivity`, which needs no
//! Java of ours -- and the activity loads this crate as a shared library and
//! calls `android_main` on a thread of its own (winit's `android-activity`
//! glue). From there it is the browser build's path (`web.rs`): the game's
//! archive, a `--package <project> --mode pak` `game.pak`, is read -- here
//! from the APK's `assets/` -- installed as the project, and the same player
//! app a desktop build runs is built and run in the activity's window.
//!
//! `scripts/build_android.sh` builds the library and the APK.

use std::io::Read;
use std::sync::Arc;

use bsengine_window::android::AndroidApp;

/// The project directory the archive is installed under -- a name, not a
/// directory that exists, as in a browser build (`web::PROJECT_DIR`).
const PROJECT_DIR: &str = "game";

/// The archive's name inside the APK's `assets/`.
const PAK_ASSET: &std::ffi::CStr = c"game.pak";

/// Called by the activity once its native thread is up.
#[no_mangle]
fn android_main(app: AndroidApp) {
    bsengine_core::logging::init_logging();
    // A panic's message goes to stderr, which an Android app does not have:
    // without this, a crash at startup leaves nothing in logcat but a
    // signal.
    std::panic::set_hook(Box::new(|info| tracing::error!("panic: {info}")));
    // Saves and crash reports go in the per-user directory, which on a
    // desktop is found from the environment; an app's working directory is
    // `/`, read-only, and nothing there names a home. The app's own internal
    // storage is that directory on Android -- private to it, kept across
    // updates, removed with it -- as Unity's `persistentDataPath` and
    // Godot's `user://` are there. Set before anything reads it, on the one
    // thread running yet.
    if let Some(dir) = app.internal_data_path() {
        std::env::set_var(bsengine_core::crash::USER_DIR_ENV, dir);
    }
    bsengine_window::android::set_app(app.clone());
    if let Err(e) = run(&app) {
        tracing::error!("the game could not start: {e}");
    }
}

fn run(android: &AndroidApp) -> Result<(), String> {
    let mut asset = android
        .asset_manager()
        .open(PAK_ASSET)
        .ok_or("the APK has no assets/game.pak")?;
    let mut bytes = Vec::new();
    asset
        .read_to_end(&mut bytes)
        .map_err(|e| format!("reading assets/game.pak: {e}"))?;
    let pak = bsengine_asset::pak::Pak::from_bytes(bytes)
        .map_err(|e| format!("assets/game.pak is not a game archive: {e}"))?;
    let pak = Arc::new(pak);
    bsengine_asset::pak_source::install(pak.clone(), PROJECT_DIR);
    tracing::info!(
        "starting {}",
        crate::read_manifest(PROJECT_DIR, Some(pak.as_ref()))
            .project
            .name
    );
    crate::build_player_app(PROJECT_DIR, Some(pak)).run();
    Ok(())
}
