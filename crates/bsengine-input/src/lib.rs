pub mod convert;
pub mod plugin;
pub mod state;
pub mod types;

pub use plugin::{InputPlugin, MouseState};
pub use state::Input;
pub use types::{
    CursorMoved, ElementState, GamepadButton, GamepadSticks, KeyCode, KeyInput, MouseButton,
    MouseInput, MouseMotion,
};
