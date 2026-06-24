pub mod descriptor;
pub mod loader;
pub mod plugin;
pub mod registry;
pub use descriptor::PluginDescriptor;
pub use loader::PluginLoader;
pub use plugin::{PluginRegistryResource, PluginSystemPlugin};
pub use registry::PluginRegistry;
