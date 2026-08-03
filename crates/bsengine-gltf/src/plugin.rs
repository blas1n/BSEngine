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
        // Async: the handle comes back immediately, the data does not.
        // `AssetServer::load` dedups by path, so re-requesting each frame is
        // cheap and yields the same handle.
        let handle = match bsengine_asset::load(
            bsengine_asset::LoadMode::Async,
            &asset_server,
            &mut gltf_assets,
            &asset.path,
            GltfLoader::load_full,
        ) {
            Ok(handle) => handle,
            Err(e) => {
                warn!("Failed to request GLTF: {e}");
                commands.entity(entity).remove::<GltfAsset>();
                continue;
            }
        };
        let Some(loaded) = gltf_assets.get(&handle) else {
            // Not resolved yet. A failed load never resolves, so ask whether
            // it failed -- otherwise a missing file retries silently forever,
            // where the old blocking path warned once and gave up.
            if let bevy_asset::LoadState::Failed(e) = asset_server.load_state(&handle) {
                warn!("Failed to load GLTF {}: {e}", asset.path);
                commands.entity(entity).remove::<GltfAsset>();
            }
            continue;
        };

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
        for (mesh_data, tex_idx) in loaded.meshes.iter().zip(loaded.mesh_tex_indices.iter()) {
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
                    let clip_library = AnimationClipLibrary::from_clips(loaded.animations.clone());
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
                commands.spawn((MeshRenderer { mesh_id }, mat, t, GlobalTransform::default()));
            }
        }

        if first {
            commands.entity(entity).remove::<GltfAsset>();
            warn!("GLTF {} has no meshes", asset.path);
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

        // Joined against CARGO_MANIFEST_DIR (this crate's own directory)
        // rather than assumed relative to the process's cwd, matching the
        // real, working convention `bsengine-mcp/tests` and
        // `bsengine-runtime/tests` already use for fixtures outside the
        // crate — robust regardless of how/where the test binary is
        // invoked from.
        let fixture_gltf_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<LoadedGltf>(fixture_gltf_path.to_str().unwrap().to_owned())
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

    // The hazard that makes LoadMode::Async different from Sync: `load`
    // hands back a Handle unconditionally, so a missing file is
    // indistinguishable from a slow one at the call site. `load_gltf_assets`
    // therefore has to ask the AssetServer whether the load *failed*, or a
    // bad path would retry silently forever. This pins down that the failure
    // really does surface as `LoadState::Failed`, and pins the exact
    // accessor form the system uses.
    //
    // It exercises the dispatcher directly rather than through
    // `load_gltf_assets`, because that system early-returns without a
    // `GpuMeshRegistry`, which needs a real window (see
    // `with_rhi_plugin_but_no_window_gltf_asset_stays`).
    #[test]
    fn async_load_of_a_missing_path_reaches_load_state_failed() {
        use bevy_asset::{AssetServer, Assets, LoadState};

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(GltfPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>().clone();
            let mut assets = app.world_mut().resource_mut::<Assets<LoadedGltf>>();
            bsengine_asset::load(
                bsengine_asset::LoadMode::Async,
                &server,
                &mut assets,
                "definitely/not/a/real/file.glb",
                GltfLoader::load_full,
            )
            .expect("Async load always returns a handle, even for a bad path")
        };

        let mut failed = false;
        for _ in 0..200 {
            app.update();
            let server = app.world().resource::<AssetServer>();
            if matches!(server.load_state(&handle), LoadState::Failed(_)) {
                failed = true;
                break;
            }
        }
        assert!(
            failed,
            "a missing path must surface as LoadState::Failed, otherwise the \
             not-ready branch would retry it forever"
        );
        // The asset must never appear in Assets<T>, which is exactly why the
        // `gltf_assets.get(&handle)` miss cannot be treated as "still loading".
        assert!(
            app.world()
                .resource::<Assets<LoadedGltf>>()
                .get(&handle)
                .is_none(),
            "a failed load must never resolve into Assets<LoadedGltf>"
        );
    }
}
