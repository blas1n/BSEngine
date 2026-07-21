//! Platform window management for BSEngine, via `winit`.
//!
//! `WindowPlugin` creates and owns the OS window, exposing `WindowHandle`
//! and firing `WindowCreated`/`WindowResized`/`WindowClosed` events;
//! `WindowDescriptor` configures initial size/title.
#![warn(missing_docs)]

pub mod plugin;
pub mod runner;
pub mod types;

pub use plugin::WindowPlugin;
pub use types::{WindowClosed, WindowCreated, WindowDescriptor, WindowHandle, WindowResized};
