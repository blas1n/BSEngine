use std::path::Path;
use crate::descriptor::PluginDescriptor;

pub struct PluginLoader;

impl PluginLoader {
    pub fn load_from_dir(dir: &Path) -> Result<PluginDescriptor, String> {
        let toml_path = dir.join("plugin.toml");
        let content = std::fs::read_to_string(&toml_path)
            .map_err(|e| format!("Cannot read {}: {e}", toml_path.display()))?;
        toml::from_str(&content)
            .map_err(|e| format!("Parse error in {}: {e}", toml_path.display()))
    }

    pub fn scan_directory(root: &Path) -> Result<Vec<PluginDescriptor>, String> {
        let mut plugins = Vec::new();
        let entries = std::fs::read_dir(root)
            .map_err(|e| format!("Cannot read dir {}: {e}", root.display()))?;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(desc) = Self::load_from_dir(&path) {
                    plugins.push(desc);
                }
            }
        }
        Ok(plugins)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_plugin_dir(name: &str, toml_content: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("bsengine_plugin_test_{name}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("plugin.toml"), toml_content).unwrap();
        dir
    }

    #[test]
    fn loader_loads_single_plugin_dir() {
        let dir = create_plugin_dir("loader_single", "name = \"test-plugin\"\nversion = \"1.0.0\"\n");
        let desc = PluginLoader::load_from_dir(&dir).expect("load failed");
        assert_eq!(desc.name, "test-plugin");
        assert_eq!(desc.version, "1.0.0");
    }

    #[test]
    fn loader_returns_error_if_no_plugin_toml() {
        let dir = std::env::temp_dir().join("bsengine_plugin_empty_test");
        std::fs::create_dir_all(&dir).unwrap();
        // Remove any existing plugin.toml to ensure it's absent
        let _ = std::fs::remove_file(dir.join("plugin.toml"));
        let result = PluginLoader::load_from_dir(&dir);
        assert!(result.is_err());
    }

    #[test]
    fn loader_scans_plugins_dir() {
        let root = std::env::temp_dir().join("bsengine_plugins_scan_root");
        std::fs::create_dir_all(&root).unwrap();

        for (name, ver) in [("scan-plugin-a", "1.0.0"), ("scan-plugin-b", "2.0.0")] {
            let sub = root.join(name);
            std::fs::create_dir_all(&sub).unwrap();
            std::fs::write(
                sub.join("plugin.toml"),
                format!("name = \"{name}\"\nversion = \"{ver}\"\n"),
            ).unwrap();
        }

        let plugins = PluginLoader::scan_directory(&root).expect("scan failed");
        assert_eq!(plugins.len(), 2);
        let names: Vec<&str> = plugins.iter().map(|p| p.name.as_str()).collect();
        assert!(names.contains(&"scan-plugin-a"));
        assert!(names.contains(&"scan-plugin-b"));
    }
}
