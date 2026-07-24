use crate::server::AssetServer;
use bevy_app::{App, Plugin};
use bsengine_ecs::Resource;

/// ECS resource wrapper exposing the shared `AssetServer` to systems.
#[derive(Resource, Clone)]
pub struct AssetServerResource(pub AssetServer);

/// App plugin that installs an `AssetServerResource` for asset loading.
pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(AssetServerResource(AssetServer::new()));
    }
}

#[cfg(test)]
mod tests {
    use super::{AssetPlugin, AssetServerResource};
    use bsengine_app::new_app;

    #[test]
    fn asset_plugin_registers_server_resource() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        assert!(app.world().get_resource::<AssetServerResource>().is_some());
    }
}
