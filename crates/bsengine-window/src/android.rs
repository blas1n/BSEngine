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

/// How far the system bars and a display cutout reach into the window from
/// each edge, in window pixels: `(left, top, right, bottom)`.
///
/// Read from Java's `WindowInsets` (`activity.getWindow().getDecorView()
/// .getRootWindowInsets()`), since winit forwards neither of Android's inset
/// notifications and `AndroidApp::content_rect()` spans the whole content
/// view: an app targeting Android 15 is drawn edge to edge, under the status
/// bar, and must ask where the bars are. Each edge is the larger of the
/// system-bar inset and, from API 28, the cutout's safe inset -- a camera
/// hole in a landscape side is not a system bar.
///
/// `None` until the window's view is attached (the root insets are null
/// before), and on any JNI failure, which is logged: the caller asks again.
pub fn safe_area_insets() -> Option<(f32, f32, f32, f32)> {
    use jni::objects::JObject;
    use jni::{jni_sig, jni_str, Env, JavaVM};

    fn read(env: &mut Env, activity: &JObject) -> jni::errors::Result<Option<[i32; 4]>> {
        let window = env
            .call_method(
                activity,
                jni_str!("getWindow"),
                jni_sig!("()Landroid/view/Window;"),
                &[],
            )?
            .l()?;
        let decor = env
            .call_method(
                &window,
                jni_str!("getDecorView"),
                jni_sig!("()Landroid/view/View;"),
                &[],
            )?
            .l()?;
        let insets = env
            .call_method(
                &decor,
                jni_str!("getRootWindowInsets"),
                jni_sig!("()Landroid/view/WindowInsets;"),
                &[],
            )?
            .l()?;
        if insets.is_null() {
            return Ok(None);
        }
        let mut edges = [0i32; 4];
        // Deprecated at API 30 for `getInsets(Type)`, but present from 20 to
        // now, which a minimum of 26 needs; they are the system bars.
        for (edge, name) in edges.iter_mut().zip([
            jni_str!("getSystemWindowInsetLeft"),
            jni_str!("getSystemWindowInsetTop"),
            jni_str!("getSystemWindowInsetRight"),
            jni_str!("getSystemWindowInsetBottom"),
        ]) {
            *edge = env.call_method(&insets, name, jni_sig!("()I"), &[])?.i()?;
        }
        if AndroidApp::sdk_version() >= 28 {
            let cutout = env
                .call_method(
                    &insets,
                    jni_str!("getDisplayCutout"),
                    jni_sig!("()Landroid/view/DisplayCutout;"),
                    &[],
                )?
                .l()?;
            if !cutout.is_null() {
                for (edge, name) in edges.iter_mut().zip([
                    jni_str!("getSafeInsetLeft"),
                    jni_str!("getSafeInsetTop"),
                    jni_str!("getSafeInsetRight"),
                    jni_str!("getSafeInsetBottom"),
                ]) {
                    let safe = env.call_method(&cutout, name, jni_sig!("()I"), &[])?.i()?;
                    *edge = (*edge).max(safe);
                }
            }
        }
        Ok(Some(edges))
    }

    let app = app();
    // SAFETY: the pointers are the activity glue's own, valid while the
    // activity is; `android_main` runs on a thread the glue attached.
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr().cast()) };
    let result = vm.attach_current_thread(|env| {
        env.with_local_frame(16, |env| {
            // SAFETY: a global reference the glue holds for the activity.
            let activity = unsafe { JObject::from_raw(env, app.activity_as_ptr().cast()) };
            read(env, &activity)
        })
    });
    match result {
        Ok(Some([l, t, r, b])) => Some((l as f32, t as f32, r as f32, b as f32)),
        Ok(None) => None,
        Err(e) => {
            tracing::warn!("safe area: reading the window insets failed: {e}");
            None
        }
    }
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
