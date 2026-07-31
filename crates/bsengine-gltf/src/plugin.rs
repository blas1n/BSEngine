use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::{AnimationPlayer, GlobalTransform, Material, Transform};
use bsengine_render::MeshRenderer;
use bsengine_rhi_wgpu::{GpuMeshRegistry, GpuTextureRegistry};
use tracing::warn;

use crate::loader::{GltfLoader, LoadedGltf};
use crate::skinned_mesh::{AnimationClipLibrary, SkinnedMesh};

/// Marker component requesting that a GLTF/GLB file be loaded onto this
/// entity; replaced by `MeshRenderer`/`Material` once loading completes.
#[derive(Component, Clone, Debug)]
pub struct GltfAsset {
    /// Filesystem path to the GLTF/GLB file to load.
    pub path: String,
}

impl GltfAsset {
    /// Creates a new `GltfAsset` pointing at the given file path.
    pub fn new(path: impl Into<String>) -> Self {
        Self { path: path.into() }
    }
}

/// Bevy plugin that loads `GltfAsset` entities into renderable meshes each frame.
pub struct GltfPlugin;

impl Plugin for GltfPlugin {
    fn build(&self, app: &mut App) {
        use bevy_asset::AssetApp;
        app.init_asset::<LoadedGltf>()
            .register_asset_loader(crate::asset_loader::GltfSourceLoader)
            .add_systems(Update, load_gltf_assets);
    }
}

fn load_gltf_assets(
    mut commands: Commands,
    query: Query<(Entity, &GltfAsset, Option<&Transform>), Without<MeshRenderer>>,
    mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    tex_registry: Option<ResMut<GpuTextureRegistry>>,
    mut gltf_assets: ResMut<bevy_asset::Assets<LoadedGltf>>,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
) {
    let Some(mut mesh_reg) = mesh_registry else {
        return;
    };
    let mut tex_reg = tex_registry;

    for (entity, asset, existing_transform) in query.iter() {
        let load_result = bsengine_asset::load(
            bsengine_asset::LoadMode::Sync,
            &asset_server,
            &mut gltf_assets,
            &asset.path,
            GltfLoader::load_full,
        );
        match load_result.and_then(|handle| {
            gltf_assets
                .get(&handle)
                .ok_or_else(|| "just-inserted asset missing from Assets<LoadedGltf>".to_string())
        }) {
            Ok(loaded) => {
                let tex_ids: Vec<Option<u64>> = if let Some(ref mut tr) = tex_reg {
                    loaded
                        .images
                        .iter()
                        .map(|img| Some(tr.load_from_rgba(img.width, img.height, &img.rgba)))
                        .collect()
                } else {
                    vec![None; loaded.images.len()]
                };

                let mut first = true;
                for (mesh_data, tex_idx) in loaded.meshes.iter().zip(loaded.mesh_tex_indices.iter())
                {
                    let mesh_id = mesh_reg.register(&mesh_data.vertices, &mesh_data.indices);
                    let texture_id = tex_idx.and_then(|i| tex_ids.get(i).copied().flatten());
                    let mat = Material {
                        texture_id,
                        ..Default::default()
                    };

                    if first {
                        let mut e = commands.entity(entity);
                        e.insert((MeshRenderer { mesh_id }, mat));
                        if let Some(skin_verts) =
                            mesh_data.skin.clone().filter(|_| !loaded.skins.is_empty())
                        {
                            let skin_data = loaded.skins[0].clone();
                            let clip_library =
                                AnimationClipLibrary::from_clips(loaded.animations.clone());
                            let first_clip_name = clip_library
                                .clips
                                .keys()
                                .next()
                                .cloned()
                                .unwrap_or_default();
                            // AnimationPlayer::new defaults duration to 0.0, and
                            // AnimationPlayer::tick is a no-op whenever duration <= 0.0
                            // -- without this, the player's `time` would never
                            // advance and the clip would appear frozen forever.
                            let duration = clip_library
                                .clips
                                .get(&first_clip_name)
                                .map(|c| c.duration)
                                .unwrap_or(0.0);
                            e.insert((
                                SkinnedMesh {
                                    mesh_id,
                                    rest_vertices: mesh_data.vertices.clone(),
                                    skin: skin_verts,
                                    skin_data,
                                    nodes: loaded.nodes.clone(),
                                },
                                clip_library,
                                AnimationPlayer::new(first_clip_name).with_duration(duration),
                            ));
                        }
                        e.remove::<GltfAsset>();
                        if existing_transform.is_none() {
                            e.insert((Transform::default(), GlobalTransform::default()));
                        }
                        first = false;
                    } else {
                        let t = existing_transform.cloned().unwrap_or_default();
                        commands.spawn((
                            MeshRenderer { mesh_id },
                            mat,
                            t,
                            GlobalTransform::default(),
                        ));
                    }
                }

                if first {
                    commands.entity(entity).remove::<GltfAsset>();
                    warn!("GLTF {} has no meshes", asset.path);
                }
            }
            Err(e) => {
                warn!("Failed to load GLTF {}: {e}", asset.path);
                commands.entity(entity).remove::<GltfAsset>();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_app::new_app;
    use bsengine_rhi_wgpu::WgpuRHIPlugin;

    #[test]
    fn gltf_plugin_builds_and_runs() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        app.update();
    }

    #[test]
    fn no_registry_leaves_gltf_asset_intact() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(GltfPlugin);
        let e = app.world_mut().spawn(GltfAsset::new("missing.gltf")).id();
        app.update();
        assert!(
            app.world().get::<GltfAsset>(e).is_some(),
            "GltfAsset should remain when GpuMeshRegistry is unavailable"
        );
    }

    #[test]
    fn with_rhi_plugin_but_no_window_gltf_asset_stays() {
        // WgpuRHIPlugin creates headless RHI but GpuMeshRegistry requires a
        // WindowHandle (created by winit). Without a window, load_gltf_assets
        // returns early and the GltfAsset marker stays on the entity.
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        let e = app.world_mut().spawn(GltfAsset::new("bad.gltf")).id();
        app.update();
        assert!(app.world().get::<GltfAsset>(e).is_some());
    }

    #[test]
    fn unskinned_gltf_asset_does_not_attach_skinning_components() {
        // Reuses the existing missing-file error path: even before hitting
        // the skin-attach branch, a load failure or an empty-skins result
        // must never insert SkinnedMesh. This is a smoke test that the new
        // branch is correctly gated, not a full skinned-asset test (that's
        // Task 7, once a real .glb fixture exists).
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        let e = app.world_mut().spawn(GltfAsset::new("missing.gltf")).id();
        app.update();
        assert!(app
            .world()
            .get::<crate::skinned_mesh::SkinnedMesh>(e)
            .is_none());
    }

    #[test]
    fn gltf_asset_loads_async_and_becomes_available() {
        use bevy_asset::{AssetServer, Assets};

        // cargo test's working directory is this crate's own manifest
        // directory (crates/bsengine-gltf), not the workspace root, so the
        // fixture path must climb back up to it — matching the convention
        // other crates use via `CARGO_MANIFEST_DIR`-relative joins (see
        // bsengine-mcp/tests and bsengine-runtime/tests).
        const FIXTURE_GLTF_PATH: &str = "../../games/mini-arena/assets/models/fox.glb";

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<LoadedGltf>(FIXTURE_GLTF_PATH)
        };

        let mut loaded = false;
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .resource::<Assets<LoadedGltf>>()
                .get(&handle)
                .is_some()
            {
                loaded = true;
                break;
            }
        }
        assert!(
            loaded,
            "glTF asset did not finish loading within 200 frames"
        );
    }
}
