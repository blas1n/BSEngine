//! The iOS build's entry point.
//!
//! An iOS app does start at `main` (`src/main.rs`), and winit then hands the
//! main thread to UIKit (`UIApplicationMain`), so no entry of its own is
//! needed as Android's `android_main` is. What differs from a desktop run is
//! where the game is: there are no arguments naming a project, and the
//! working directory is `/`. The game's archive -- a `--package <project>
//! --mode ios` `game.pak`, its manifest inside -- sits at the root of the
//! `.app` beside the executable (`scripts/build_ios.sh` puts it there), as
//! Android's sits in the APK's `assets/` (`android.rs`), and it is installed
//! as the project and run by the same player app every build runs.
//!
//! One argument is taken, for CI: `--frames <n>` quits after `n` frames
//! printing what was drawn, as a desktop run's does (`quit_after_frames`).
//! `xcrun simctl launch` passes it.

use std::sync::Arc;

/// The project directory the archive is installed under -- a name, not a
/// directory that exists, as in a browser or Android build.
const PROJECT_DIR: &str = "game";

/// Whether this process is the app being launched -- no arguments, or only
/// `--frames` -- rather than the runtime's command line (`--package` and the
/// rest), which has no use on a phone but costs nothing to leave reachable.
pub(crate) fn is_app_launch() -> bool {
    std::env::args()
        .nth(1)
        .is_none_or(|first| first == "--frames")
}

/// Called by `crate::run` on iOS for an app launch, in place of the desktop
/// command line.
pub(crate) fn run() {
    bsengine_core::logging::init_logging();
    let frames = frames_arg(std::env::args().skip(1));
    if let Err(e) = start(frames) {
        // Stdout and stderr reach the console `simctl launch --console-pty`
        // attaches, and Xcode's; nothing else would show why the app ended.
        tracing::error!("the game could not start: {e}");
        eprintln!("the game could not start: {e}");
        std::process::exit(1);
    }
}

/// `--frames <n>` from the arguments, if given; anything else is ignored
/// (iOS passes none of its own, but a debugger may).
fn frames_arg(mut args: impl Iterator<Item = String>) -> Option<u32> {
    while let Some(arg) = args.next() {
        if arg == "--frames" {
            return args.next().and_then(|n| n.parse().ok());
        }
    }
    None
}

fn start(frames: Option<u32>) -> Result<(), String> {
    let exe = std::env::current_exe().map_err(|e| format!("cannot locate this executable: {e}"))?;
    let bundle = exe
        .parent()
        .ok_or_else(|| format!("{} has no parent directory", exe.display()))?;
    let path = bundle.join(bsengine_asset::cook::PAK_FILE_NAME);
    let pak = bsengine_asset::pak::Pak::open(&path)
        .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    let pak = Arc::new(pak);
    bsengine_asset::pak_source::install(pak.clone(), PROJECT_DIR);
    tracing::info!(
        "starting {}",
        crate::read_manifest(PROJECT_DIR, Some(pak.as_ref()))
            .project
            .name
    );
    let mut app = crate::build_player_app(PROJECT_DIR, Some(pak));
    if let Some(frames) = frames {
        crate::quit_after_frames(&mut app, frames, None);
    }
    app.run();
    Ok(())
}
