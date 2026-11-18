//! What runs, at once, when the app leaves the screen and when it returns.

use bevy_ecs::prelude::Resource;
use bevy_ecs::world::World;

/// What has to happen at once when the app goes into the background, and
/// when it comes back: the window runner calls these from winit's
/// `suspended` and `resumed`, before either returns. Android sends those
/// when the player switches away (home button, app switcher, a call) and
/// back; iOS on resigning and regaining active; a browser when the page goes
/// into and out of the back-forward cache. Desktop window systems never do.
///
/// Not a message, because a message is read at the next frame and there is
/// no next frame in time: on Android the activity's native window is
/// destroyed as soon as `suspended` returns, and the process may be killed
/// at any point after it -- the renderer must have let go of its surface by
/// then, and a game that saves on the way out must have saved. Each plugin
/// adds what it owns:
///
/// - the renderer releases and remakes the window's surface
///   (`bsengine_rhi_wgpu`);
/// - the mixer pauses and resumes every sound where it is (`bsengine_audio`);
/// - scripts are told, through `Bsengine.onApplicationPause`
///   (`bsengine_scripting`) -- Unity's `OnApplicationPause`.
///
/// Called in the order added; none of them depends on another having run.
/// Plain function pointers, so the resource stays `Send`; each finds what it
/// needs in the world.
#[derive(Resource, Default)]
pub struct AppLifecycleHooks {
    /// Called when the app leaves the screen, in the order added.
    pub on_suspend: Vec<fn(&mut World)>,
    /// Called when it comes back, in the order added, before the next frame.
    pub on_resume: Vec<fn(&mut World)>,
}
