//! Keyboard, mouse, and gamepad input abstraction for BSEngine.
//!
//! `InputPlugin` polls the platform window layer and exposes `Input<T>`
//! (for `KeyCode`/`MouseButton`/`GamepadButton`) plus cursor/wheel events
//! (`CursorMoved`, `MouseMotion`, `MouseWheel`) and `MouseState` as
//! ECS resources.
#![warn(missing_docs)]

pub mod convert;
pub mod plugin;
pub mod state;
pub mod types;

pub use plugin::{InputPlugin, MouseState};
pub use state::Input;
pub use types::{
    CursorMoved, ElementState, GamepadButton, GamepadSticks, KeyCode, KeyInput, MouseButton,
    MouseInput, MouseMotion, MouseWheel,
};
