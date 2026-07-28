use serde::{Deserialize, Serialize};

/// Plugin manifest metadata, parsed from a plugin's `plugin.toml` file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginDescriptor {
    /// Unique plugin name, used as its registry key.
    pub name: String,
    /// Plugin version string (e.g. semver).
    pub version: String,
    /// Optional human-readable description of the plugin.
    pub description: Option<String>,
    /// Optional path, relative to the plugin directory, to its entry script.
    pub entry_script: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_parses_minimal_toml() {
        let toml_str = r#"
            name = "my-plugin"
            version = "1.0.0"
        "#;
        let desc: PluginDescriptor = toml::from_str(toml_str).expect("parse failed");
        assert_eq!(desc.name, "my-plugin");
        assert_eq!(desc.version, "1.0.0");
        assert!(desc.entry_script.is_none());
        assert!(desc.description.is_none());
    }

    #[test]
    fn descriptor_parses_full_toml() {
        let toml_str = r#"
            name = "ai-companion"
            version = "0.2.1"
            description = "An AI companion plugin"
            entry_script = "scripts/main.ts"
        "#;
        let desc: PluginDescriptor = toml::from_str(toml_str).expect("parse failed");
        assert_eq!(desc.name, "ai-companion");
        assert_eq!(desc.entry_script.as_deref(), Some("scripts/main.ts"));
        assert_eq!(desc.description.as_deref(), Some("An AI companion plugin"));
    }

    #[test]
    fn descriptor_serializes_to_toml() {
        let desc = PluginDescriptor {
            name: "test-plugin".to_string(),
            version: "0.1.0".to_string(),
            description: None,
            entry_script: Some("scripts/init.ts".to_string()),
        };
        let toml_str = toml::to_string(&desc).expect("serialize failed");
        assert!(toml_str.contains("test-plugin"));
        assert!(toml_str.contains("scripts/init.ts"));
    }
}
