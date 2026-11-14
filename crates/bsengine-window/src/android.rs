//! The `AndroidApp` an Android build's entry point receives.
//!
//! On Android the process does not start at `main`: the activity loads the
//! game's shared library and calls its `android_main(app)`, and winit's event
//! loop can only be built with that `app`. The entry point and the runner are
//! in different crates and the app is built in between, so the handle is
//! parked here once and read back when the loop is made.

use std::sync::OnceLock;

pub use winit::platform::android::activity::AndroidApp;

static APP: OnceLock<AndroidApp> = OnceLock::new();

/// Keeps the activity's `AndroidApp` for the event loop. Called once, by
/// `android_main`, before the app runs.
pub fn set_app(app: AndroidApp) {
    // A second `android_main` in one process (the activity recreated after
    // being destroyed) hands the same activity glue; the first is kept.
    let _ = APP.set(app);
}

/// The activity's `AndroidApp`.
///
/// # Panics
/// If `set_app` was never called: the runner was started from somewhere
/// other than `android_main`, and there is no activity to draw into.
pub fn app() -> AndroidApp {
    APP.get()
        .cloned()
        .expect("bsengine_window::android::set_app was not called before the event loop")
}
