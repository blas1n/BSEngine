use bsengine_app::new_app;
use bsengine_plugin::{PluginLoader, PluginRegistryResource, PluginSystemPlugin};

#[test]
fn full_plugin_system_integration() {
    let root = std::env::temp_dir().join("bsengine_integ_plugins");
    std::fs::create_dir_all(&root).unwrap();
    let plugin_dir = root.join("integ-plugin");
    std::fs::create_dir_all(&plugin_dir).unwrap();
    std::fs::write(
        plugin_dir.join("plugin.toml"),
        "name = \"integ-plugin\"\nversion = \"1.0.0\"\ndescription = \"Integration test plugin\"\n",
    )
    .unwrap();

    let mut app = new_app();
    app.add_plugins(PluginSystemPlugin);
    app.update();

    let plugins = PluginLoader::scan_directory(&root).expect("scan failed");
    assert_eq!(plugins.len(), 1);

    {
        let mut reg = app.world_mut().resource_mut::<PluginRegistryResource>();
        for p in plugins {
            reg.0.register(p);
        }
    }

    let reg = &app.world().resource::<PluginRegistryResource>().0;
    assert!(reg.get("integ-plugin").is_some());
    assert_eq!(reg.get("integ-plugin").unwrap().version, "1.0.0");
}
