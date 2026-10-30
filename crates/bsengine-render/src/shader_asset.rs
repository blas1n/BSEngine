use bevy_asset::Asset;
use bevy_reflect::TypePath;

/// Raw WGSL source text for a custom shader, loaded from disk. Compilation
/// into a `wgpu::ShaderModule` happens separately in `bsengine-rhi-wgpu`
/// (this crate has no GPU device handle to compile with).
#[derive(Asset, TypePath, Debug, Clone)]
pub struct ShaderSource(pub String);

/// Reads a `.wgsl` file from disk into a [`ShaderSource`].
pub fn load_shader_source(path: &str) -> Result<ShaderSource, String> {
    std::fs::read_to_string(path)
        .map(ShaderSource)
        .map_err(|e| e.to_string())
}

use bevy_asset::io::Reader;
use bevy_asset::{AssetLoader, LoadContext};

/// Backs `LoadMode::Async` for custom shaders via `AssetServer::load`.
// `TypePath`: Bevy 0.18 makes it a supertrait of `AssetLoader`.
#[derive(Default, bevy_reflect::TypePath)]
pub struct ShaderSourceLoader;

impl AssetLoader for ShaderSourceLoader {
    type Asset = ShaderSource;
    type Settings = ();
    type Error = String;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &Self::Settings,
        _load_context: &mut LoadContext<'_>,
    ) -> Result<Self::Asset, Self::Error> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .await
            .map_err(|e| format!("read: {e}"))?;
        String::from_utf8(bytes)
            .map(ShaderSource)
            .map_err(|e| format!("not valid UTF-8: {e}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_shader_source_reads_file() {
        let path = std::env::temp_dir().join("test_shader_asset.wgsl");
        std::fs::write(&path, "// wgsl\n").unwrap();
        let src = load_shader_source(path.to_str().unwrap()).unwrap();
        assert_eq!(src.0, "// wgsl\n");
    }

    #[test]
    fn load_shader_source_missing_file_errors() {
        assert!(load_shader_source("definitely/missing.wgsl").is_err());
    }

    #[test]
    fn shader_source_loads_async_and_becomes_available() {
        use bevy_asset::{AssetApp, AssetServer, Assets};
        use bsengine_app::new_app;

        // Its own directory, which is the test's project: assets load by
        // their project path, so the shader is `shader.wgsl` in it.
        let dir =
            std::env::temp_dir().join(format!("bsengine_test_async_shader_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("shader.wgsl"), "// async wgsl\n").unwrap();

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            dir.to_string_lossy().into_owned(),
        ));
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.init_asset::<ShaderSource>();
        app.register_asset_loader(ShaderSourceLoader);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<ShaderSource>("shader.wgsl")
        };

        let mut loaded = None;
        for _ in 0..200 {
            app.update();
            if let Some(src) = app.world().resource::<Assets<ShaderSource>>().get(&handle) {
                loaded = Some(src.0.clone());
                break;
            }
        }
        assert_eq!(loaded, Some("// async wgsl\n".to_string()));
    }
}
