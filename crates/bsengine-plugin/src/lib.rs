//! Runtime plugin loader for BSEngine.
//!
//! `PluginLoader` reads a `PluginDescriptor` and loads the described
//! plugin at runtime; `PluginRegistry`/`PluginRegistryResource` track
//! what's currently loaded. `PluginSystemPlugin` wires the loader into
//! the app.
#![warn(missing_docs)]

pub mod descriptor;
pub mod loader;
pub mod plugin;
pub mod registry;
pub use descriptor::PluginDescriptor;
pub use loader::PluginLoader;
pub use plugin::{PluginRegistryResource, PluginSystemPlugin};
pub use registry::PluginRegistry;
