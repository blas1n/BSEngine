use bsengine_ecs::{Event, Resource};
use std::sync::Arc;
use winit::window::Window;

/// ECS resource holding a shared handle to the OS window created by the runner.
#[derive(Resource, Clone)]
pub struct WindowHandle(pub Arc<Window>);

/// Initial window configuration, inserted as a resource by `WindowPlugin`.
#[derive(Resource, Debug, Clone)]
pub struct WindowDescriptor {
    /// Text shown in the OS window's title bar.
    pub title: String,
    /// Initial window width, in logical pixels.
    pub width: u32,
    /// Initial window height, in logical pixels.
    pub height: u32,
    /// Whether the user can resize the window after creation.
    pub resizable: bool,
}

impl Default for WindowDescriptor {
    fn default() -> Self {
        Self {
            title: "BSEngine".to_string(),
            width: 1280,
            height: 720,
            resizable: true,
        }
    }
}

/// Fired whenever the OS window is resized, with the new dimensions.
#[derive(Event, Debug, Clone)]
pub struct WindowResized {
    /// New window width, in physical pixels.
    pub width: u32,
    /// New window height, in physical pixels.
    pub height: u32,
}

/// Fired once the OS window has been created and its handle inserted as a resource.
#[derive(Event, Debug, Clone)]
pub struct WindowCreated;

/// Fired when the user requests the window be closed (e.g. clicking the close button).
#[derive(Event, Debug, Clone)]
pub struct WindowClosed;

/// What the first frame has to wait for, where it cannot be had at once.
///
/// On a desktop everything a frame needs is made synchronously before the
/// first one runs -- the GPU device included -- and every system is written
/// for that: one that resolves a mesh when its entity appears, say, finds the
/// mesh registry already there. In a browser the device arrives as a future,
/// a few frames after the window does, and such a system would have had its
/// one chance and missed. So the browser's first frame waits: a plugin adds
/// a hook that starts the work once the window exists, and a check that says
/// when it is done, and the runner calls `App::update` only when every check
/// passes. Native builds never wait; nothing is added there.
///
/// A non-send resource: the hooks hold the page's futures and closures.
#[derive(Default)]
pub struct FirstFrameGate {
    /// Called once, with the window, as soon as it exists.
    pub on_window: Vec<Box<dyn FnOnce(Arc<Window>)>>,
    /// The first frame runs when all of these say true.
    pub ready: Vec<Box<dyn Fn() -> bool>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn window_descriptor_default() {
        let desc = WindowDescriptor::default();
        assert_eq!(desc.width, 1280);
        assert_eq!(desc.height, 720);
        assert_eq!(desc.title, "BSEngine");
        assert!(desc.resizable);
    }
}
