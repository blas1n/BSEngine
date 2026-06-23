use bevy_app::{App, Plugin};
use bsengine_ecs::Resource;
use crate::registry::PluginRegistry;

#[derive(Resource)]
pub struct PluginRegistryResource(pub PluginRegistry);

pub struct PluginSystemPlugin;

impl Plugin for PluginSystemPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(PluginRegistryResource(PluginRegistry::new()));
    }
}

#[cfg(test)]
mod tests {
    use bsengine_app::new_app;
    use super::{PluginSystemPlugin, PluginRegistryResource};

    #[test]
    fn plugin_system_registers_registry() {
        let mut app = new_app();
        app.add_plugins(PluginSystemPlugin);
        assert!(app.world().get_resource::<PluginRegistryResource>().is_some());
    }

    #[test]
    fn plugin_system_registry_starts_empty() {
        let mut app = new_app();
        app.add_plugins(PluginSystemPlugin);
        let reg = &app.world().resource::<PluginRegistryResource>().0;
        assert_eq!(reg.all().len(), 0);
    }
}
