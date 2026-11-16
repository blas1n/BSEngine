//! Keyboard, mouse, gamepad and touch input abstraction for BSEngine.
//!
//! `InputPlugin` polls the platform window layer and exposes `Input<T>`
//! (for `KeyCode`/`MouseButton`/`GamepadButton`) plus cursor/wheel events
//! (`CursorMoved`, `MouseMotion`, `MouseWheel`), `MouseState` and `Touches`
//! as ECS resources.
#![warn(missing_docs)]

/// Named actions ("jump") bound to keys, buttons and stick directions.
pub mod actions;
/// Translates platform (winit/gilrs) input events into BSEngine's own input types.
pub mod convert;
/// Hold, tap and hold-and-release: when an action's press fires.
pub mod interaction;
/// The `InputPlugin` bevy plugin and the per-frame `MouseState` resource.
pub mod plugin;
/// The generic `Input<T>` press/release tracking resource.
pub mod state;
/// Fingers on a touch screen, and the first one standing in for the mouse.
pub mod touch;
/// Input event and resource type definitions (keys, buttons, cursor, gamepad).
pub mod types;

pub use actions::{ActionState, ActionValue, Binding, GamepadAxis, InputActions};
pub use interaction::Interaction;
pub use plugin::{InputPlugin, InputSystems, MouseState};
pub use state::Input;
pub use touch::{Touch, TouchEvent, TouchInput, TouchPhase, TouchSettings, Touches};
pub use types::{
    CursorMoved, ElementState, GamepadButton, GamepadSticks, KeyCode, KeyInput, MouseButton,
    MouseInput, MouseMotion, MouseWheel, KEY_NAMES,
};
