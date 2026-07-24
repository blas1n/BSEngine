//! Keyboard, mouse, and gamepad input abstraction for BSEngine.
//!
//! `InputPlugin` polls the platform window layer and exposes `Input<T>`
//! (for `KeyCode`/`MouseButton`/`GamepadButton`) plus cursor/wheel events
//! (`CursorMoved`, `MouseMotion`, `MouseWheel`) and `MouseState` as
//! ECS resources.
#![warn(missing_docs)]

/// Translates platform (winit/gilrs) input events into BSEngine's own input types.
pub mod convert;
/// The `InputPlugin` bevy plugin and the per-frame `MouseState` resource.
pub mod plugin;
/// The generic `Input<T>` press/release tracking resource.
pub mod state;
/// Input event and resource type definitions (keys, buttons, cursor, gamepad).
pub mod types;

pub use plugin::{InputPlugin, MouseState};
pub use state::Input;
pub use types::{
    CursorMoved, ElementState, GamepadButton, GamepadSticks, KeyCode, KeyInput, MouseButton,
    MouseInput, MouseMotion, MouseWheel,
};
