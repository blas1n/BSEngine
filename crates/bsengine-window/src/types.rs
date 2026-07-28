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
