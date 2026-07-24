//! Runtime plugin loader for BSEngine.
//!
//! `PluginLoader` reads a `PluginDescriptor` and loads the described
//! plugin at runtime; `PluginRegistry`/`PluginRegistryResource` track
//! what's currently loaded. `PluginSystemPlugin` wires the loader into
//! the app.
#![warn(missing_docs)]

/// Defines `PluginDescriptor`, the manifest format parsed from `plugin.toml`.
pub mod descriptor;
/// Discovers and reads plugin manifests from disk.
pub mod loader;
/// Wires the plugin registry into the Bevy `App` as a resource.
pub mod plugin;
/// In-memory tracking of loaded plugin descriptors, keyed by name.
pub mod registry;
pub use descriptor::PluginDescriptor;
pub use loader::PluginLoader;
pub use plugin::{PluginRegistryResource, PluginSystemPlugin};
pub use registry::PluginRegistry;
