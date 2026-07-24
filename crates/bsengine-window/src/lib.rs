//! Platform window management for BSEngine, via `winit`.
//!
//! `WindowPlugin` creates and owns the OS window, exposing `WindowHandle`
//! and firing `WindowCreated`/`WindowResized`/`WindowClosed` events;
//! `WindowDescriptor` configures initial size/title.
#![warn(missing_docs)]

/// The Bevy plugin that owns window creation and lifecycle.
pub mod plugin;
/// The `winit` event loop runner that drives the ECS app per frame.
pub mod runner;
/// Window-related resources and events (handle, descriptor, resize/create/close).
pub mod types;

pub use plugin::WindowPlugin;
pub use types::{WindowClosed, WindowCreated, WindowDescriptor, WindowHandle, WindowResized};
