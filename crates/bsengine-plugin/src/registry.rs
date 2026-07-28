use crate::descriptor::PluginDescriptor;
use std::collections::HashMap;

/// In-memory table of loaded plugin descriptors, keyed by plugin name.
pub struct PluginRegistry {
    plugins: HashMap<String, PluginDescriptor>,
}

impl PluginRegistry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self {
            plugins: HashMap::new(),
        }
    }

    /// Registers a plugin descriptor, overwriting any existing entry with the same name.
    pub fn register(&mut self, desc: PluginDescriptor) {
        self.plugins.insert(desc.name.clone(), desc);
    }

    /// Looks up a registered plugin descriptor by name.
    pub fn get(&self, name: &str) -> Option<&PluginDescriptor> {
        self.plugins.get(name)
    }

    /// Returns all registered plugin descriptors, in arbitrary order.
    pub fn all(&self) -> Vec<&PluginDescriptor> {
        self.plugins.values().collect()
    }
}

impl Default for PluginRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::descriptor::PluginDescriptor;

    fn make_desc(name: &str, version: &str) -> PluginDescriptor {
        PluginDescriptor {
            name: name.to_string(),
            version: version.to_string(),
            description: None,
            entry_script: None,
        }
    }

    #[test]
    fn registry_register_and_get() {
        let mut reg = PluginRegistry::new();
        reg.register(make_desc("my-plugin", "1.0.0"));
        let found = reg.get("my-plugin");
        assert!(found.is_some());
        assert_eq!(found.unwrap().version, "1.0.0");
    }

    #[test]
    fn registry_list_all() {
        let mut reg = PluginRegistry::new();
        reg.register(make_desc("plugin-a", "1.0.0"));
        reg.register(make_desc("plugin-b", "2.0.0"));
        assert_eq!(reg.all().len(), 2);
    }

    #[test]
    fn registry_get_nonexistent_returns_none() {
        let reg = PluginRegistry::new();
        assert!(reg.get("does-not-exist").is_none());
    }

    #[test]
    fn registry_duplicate_register_overwrites() {
        let mut reg = PluginRegistry::new();
        reg.register(make_desc("my-plugin", "1.0.0"));
        reg.register(make_desc("my-plugin", "2.0.0"));
        assert_eq!(reg.get("my-plugin").unwrap().version, "2.0.0");
        assert_eq!(reg.all().len(), 1);
    }
}
