use bevy_app::{App, Plugin};
use bevy_asset::AssetApp;

use crate::types::TextureAsset;

/// Installs `bevy_asset`'s `AssetPlugin` and registers every content-asset
/// type this engine's loading systems use. Downstream crates (`bsengine-gltf`,
/// `bsengine-render`, `bsengine-audio`) register their own asset types from
/// their own plugins — this only owns the types defined in this crate.
pub struct AssetPlugin;

impl Plugin for AssetPlugin {
    fn build(&self, app: &mut App) {
        // bevy_asset's AssetServer::load spawns its background load task on
        // the IoTaskPool. Upstream Bevy initializes this via
        // bevy_core::TaskPoolPlugin as part of DefaultPlugins/MinimalPlugins
        // — this workspace doesn't depend on bevy_core, so LoadMode::Async
        // (and this plugin's own AssetServer) would panic the first time
        // anything tried to load without this. get_or_init is idempotent,
        // so this is safe even if a future plugin also initializes it.
        bevy_tasks::IoTaskPool::get_or_init(bevy_tasks::TaskPool::default);

        // Every asset path in this engine (scene RON `gltf:` fields,
        // Bsengine.setShader() paths, playSound() paths, ...) is already
        // fully resolved via bsengine_core::resolve_project_path before it
        // reaches any loader — it's a real filesystem-relative path, not a
        // path meant to be joined under bevy_asset's own "assets/" root
        // convention. file_path: "" makes AssetServer.load(path) treat
        // `path` as directly filesystem-relative (matching every existing
        // std::fs::read call site this plan migrates), instead of silently
        // prepending "assets/" and resolving the wrong file.
        app.add_plugins(bevy_asset::AssetPlugin {
            file_path: String::new(),
            ..Default::default()
        })
        .init_asset::<TextureAsset>()
        .register_asset_loader(crate::texture_loader::TextureAssetLoader);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::{AssetServer, Assets};
    use bsengine_app::new_app;

    #[test]
    fn asset_plugin_registers_asset_server() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        assert!(app.world().get_resource::<AssetServer>().is_some());
    }

    #[test]
    fn asset_plugin_registers_texture_assets() {
        let mut app = new_app();
        app.add_plugins(AssetPlugin);
        assert!(app.world().get_resource::<Assets<TextureAsset>>().is_some());
    }
}
