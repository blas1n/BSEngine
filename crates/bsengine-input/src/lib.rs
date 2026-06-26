pub mod convert;
pub mod plugin;
pub mod state;
pub mod types;

pub use plugin::InputPlugin;
pub use state::Input;
pub use types::{CursorMoved, ElementState, KeyCode, KeyInput, MouseButton, MouseInput};
