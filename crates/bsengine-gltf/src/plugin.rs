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
            .add_systems(Update, (load_gltf_assets, rebuild_modified_gltf).chain());
    }
}

/// The in-flight load for a [`GltfAsset`], held so the request is made once
/// rather than re-issued every frame.
///
/// Re-calling `AssetServer::load` for a path whose load has *failed* resets it
/// to `Loading` and starts the load again (`bevy_asset` 0.14.2,
/// `server/info.rs:216-221`). `LoadState::Failed` is set in `PreUpdate` while
/// this system runs in `Update`, so a polling loop that re-requests erases the
/// failure before it can observe it — retrying a missing file forever and
/// spawning a fresh filesystem task every frame. Holding the handle also keeps
/// it strong; nothing else does between frames.
///
/// Internal to this system — `GltfAsset.path` stays a plain `String`, so scene
/// RON, the scripting API and the MCP tools are unaffected.
#[derive(Component)]
struct PendingGltf(bevy_asset::Handle<LoadedGltf>);

/// What a resolved [`GltfAsset`] produced, kept on the entity so a later reload
/// can rebuild it.
///
/// The handle is retained because `AssetEvent::Modified` only fires while a
/// strong handle exists — drop it and `Assets::track_assets` frees the asset and
/// `AssetServer::reload` becomes a silent no-op (measured by
/// `reload_emits_modified_only_while_a_handle_is_retained`).
///
/// The ids are recorded rather than re-derived because the rebuild replaces
/// buffer *contents* under them, leaving `MeshRenderer.mesh_id` and
/// `Material.texture_id` untouched — so the extra entities a multi-mesh glTF
/// spawns are updated without having to find them.
#[derive(Component)]
struct GltfLoaded {
    handle: bevy_asset::Handle<LoadedGltf>,
    /// GPU mesh ids created for this asset, in `LoadedGltf::meshes` order.
    mesh_ids: Vec<u64>,
    /// GPU texture ids created for this asset, in `LoadedGltf::images` order.
    texture_ids: Vec<u64>,
}

fn load_gltf_assets(
    mut commands: Commands,
    query: Query<
        (Entity, &GltfAsset, Option<&PendingGltf>, Option<&Transform>),
        Without<MeshRenderer>,
    >,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut tex_registry: Option<ResMut<GpuTextureRegistry>>,
    mut gltf_assets: ResMut<bevy_asset::Assets<LoadedGltf>>,
    asset_server: Res<bevy_asset::AssetServer>,
) {
    for (entity, asset, pending, existing_transform) in query.iter() {
        // Request exactly once, then retain the handle. See `PendingGltf`.
        let Some(pending) = pending else {
            match bsengine_asset::load(
                bsengine_asset::LoadMode::Async,
                &asset_server,
                &mut gltf_assets,
                &asset.path,
                GltfLoader::load_full,
            ) {
                Ok(handle) => {
                    commands.entity(entity).insert(PendingGltf(handle));
                }
                Err(e) => {
                    // Unreachable: `LoadMode::Async` is infallible. This arm
                    // exists only because the shared `load()` signature
                    // returns `Result` for the benefit of `Sync` callers.
                    warn!("Failed to request GLTF: {e}");
                    commands.entity(entity).remove::<GltfAsset>();
                }
            }
            continue;
        };

        let Some(loaded) = gltf_assets.get(&pending.0) else {
            // Not resolved yet. A failed load never resolves, so ask whether
            // it failed -- otherwise a missing file retries silently forever,
            // where the old blocking path warned once and gave up.
            if let bevy_asset::LoadState::Failed(e) = asset_server.load_state(&pending.0) {
                warn!("Failed to load GLTF {}: {e}", asset.path);
                commands.entity(entity).remove::<(GltfAsset, PendingGltf)>();
            }
            continue;
        };

        // The data is here, but turning it into meshes needs the GPU. Without
        // a registry (no window yet), keep both markers and retry next frame
        // — the load itself is already done, so this costs nothing.
        let Some(mesh_reg) = mesh_registry.as_mut() else {
            continue;
        };
        let tex_ids: Vec<Option<u64>> = if let Some(tr) = tex_registry.as_mut() {
            loaded
                .images
                .iter()
                .map(|img| Some(tr.load_from_rgba(img.width, img.height, &img.rgba)))
                .collect()
        } else {
            vec![None; loaded.images.len()]
        };

        let mut first = true;
        // Recorded for `GltfLoaded`. Collected across the whole loop -- including
        // the extra entities spawned for meshes after the first -- because
        // replacing under those ids is exactly how those entities get updated,
        // and none of them are findable from this one later.
        let mut mesh_ids: Vec<u64> = Vec::with_capacity(loaded.meshes.len());
        for (mesh_data, tex_idx) in loaded.meshes.iter().zip(loaded.mesh_tex_indices.iter()) {
            let mesh_id = mesh_reg.register(&mesh_data.vertices, &mesh_data.indices);
            mesh_ids.push(mesh_id);
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
                e.remove::<(GltfAsset, PendingGltf)>();
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
            commands.entity(entity).remove::<(GltfAsset, PendingGltf)>();
            warn!("GLTF {} has no meshes", asset.path);
        } else {
            // Inserted here rather than beside the `remove` above because the
            // full id list is only known once the loop has run. Goes on the
            // same entity that just got `MeshRenderer`, so `rebuild_modified_gltf`
            // finds it. `tex_ids` is either all-`Some` (registry present) or
            // all-`None` (absent), never mixed, so flattening keeps
            // `LoadedGltf::images` order intact.
            commands.entity(entity).insert(GltfLoaded {
                handle: pending.0.clone(),
                mesh_ids,
                texture_ids: tex_ids.iter().flatten().copied().collect(),
            });
        }
    }
}

/// Rebuilds GPU state for a glTF whose asset data was replaced.
///
/// Replaces buffer contents under the ids recorded at load time, so no entity
/// is touched. A structural change (different mesh or image count) cannot be
/// expressed this way — those entities were spawned at load time — so it warns
/// and rebuilds the overlap rather than pretending to succeed.
fn rebuild_modified_gltf(
    mut events: bevy_ecs::prelude::EventReader<bevy_asset::AssetEvent<LoadedGltf>>,
    query: Query<&GltfLoaded>,
    gltf_assets: Res<bevy_asset::Assets<LoadedGltf>>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut tex_registry: Option<ResMut<GpuTextureRegistry>>,
) {
    for event in events.read() {
        let bevy_asset::AssetEvent::Modified { id } = event else {
            continue;
        };
        for loaded in query.iter().filter(|l| l.handle.id() == *id) {
            let Some(data) = gltf_assets.get(&loaded.handle) else {
                continue;
            };
            if let Some(reg) = mesh_registry.as_mut() {
                if data.meshes.len() != loaded.mesh_ids.len() {
                    warn!(
                        "reloaded glTF now has {} mesh(es), was {} -- rebuilding \
                         the overlap only; a structural change needs a restart",
                        data.meshes.len(),
                        loaded.mesh_ids.len()
                    );
                }
                for (mesh_id, mesh_data) in loaded.mesh_ids.iter().zip(data.meshes.iter()) {
                    reg.replace(*mesh_id, &mesh_data.vertices, &mesh_data.indices);
                }
            }
            if let Some(reg) = tex_registry.as_mut() {
                for (tex_id, img) in loaded.texture_ids.iter().zip(data.images.iter()) {
                    reg.replace(*tex_id, img.width, img.height, &img.rgba);
                }
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

    // Hot reload rests on `AssetEvent::Modified` reaching each consumer, and
    // that only happens while something still holds a strong handle: once the
    // last one drops, `Assets::track_assets` frees the asset and
    // `AssetServer::reload` on its path becomes a silent no-op. Measured here
    // rather than assumed, because it is the reason each consumer keeps its
    // handle after the load resolves instead of dropping it as dead weight --
    // a future cleanup that "tidies away" a retained handle would disable hot
    // reload for that asset type with no other symptom.
    #[test]
    fn reload_emits_modified_only_while_a_handle_is_retained() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");
        let path = fixture.to_str().unwrap().to_owned();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        // GltfPlugin is what calls init_asset::<LoadedGltf>(); without it the
        // AssetServer panics on the first load. No entity carries GltfAsset
        // here, so its system is inert.
        app.add_plugins(GltfPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<LoadedGltf>(path.clone())
        };
        let id = handle.id();

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
            "fixture must load before the experiment means anything"
        );

        let mut reader: ManualEventReader<AssetEvent<LoadedGltf>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<LoadedGltf>>>()
            .get_reader();

        let drain = |app: &mut bevy_app::App,
                     reader: &mut ManualEventReader<AssetEvent<LoadedGltf>>|
         -> Vec<String> {
            let events = app.world().resource::<Events<AssetEvent<LoadedGltf>>>();
            reader.read(events).map(|e| format!("{e:?}")).collect()
        };
        let _ = drain(&mut app, &mut reader);

        // --- A: handle retained ---
        app.world().resource::<AssetServer>().reload(path.clone());
        let mut with_handle = Vec::new();
        for _ in 0..60 {
            app.update();
            with_handle.extend(drain(&mut app, &mut reader));
        }
        let still_present_after_reload = app
            .world()
            .resource::<Assets<LoadedGltf>>()
            .get(&handle)
            .is_some();

        // --- B: handle dropped ---
        drop(handle);
        for _ in 0..10 {
            app.update();
        }
        let present_after_drop = app
            .world()
            .resource::<Assets<LoadedGltf>>()
            .get(id)
            .is_some();
        let _ = drain(&mut app, &mut reader);

        app.world().resource::<AssetServer>().reload(path.clone());
        let mut without_handle = Vec::new();
        for _ in 0..60 {
            app.update();
            without_handle.extend(drain(&mut app, &mut reader));
        }
        let present_after_reload_without_handle = app
            .world()
            .resource::<Assets<LoadedGltf>>()
            .get(id)
            .is_some();

        assert!(
            with_handle.iter().any(|e| e.starts_with("Modified")),
            "reloading a path whose handle is still held must emit \
             AssetEvent::Modified -- that event is the whole mechanism hot \
             reload runs on; got {with_handle:?}"
        );
        assert!(
            still_present_after_reload,
            "the asset must stay in Assets across a reload, or consumers would \
             see it vanish rather than change"
        );

        assert!(
            !present_after_drop,
            "dropping the last handle must free the asset; if this ever stops \
             being true the rest of this test proves nothing"
        );
        assert!(
            without_handle.is_empty(),
            "reloading a path with no handle held must emit nothing at all -- \
             this is why every consumer retains its handle after the load \
             resolves; got {without_handle:?}"
        );
        assert!(
            !present_after_reload_without_handle,
            "reload must not resurrect an asset nobody holds"
        );
    }

    /// Inserts real `GpuMeshRegistry`/`GpuTextureRegistry` resources built on a
    /// real headless `wgpu` device.
    ///
    /// These are not stand-ins: they are the same types the renderer uses,
    /// backed by an actual adapter, doing real buffer and texture uploads. The
    /// plugin only builds them next to a swapchain surface, which needs a winit
    /// window (see `with_rhi_plugin_but_no_window_gltf_asset_stays`), and
    /// `WgpuRHI`'s device is `pub(crate)` — so a headless test has to construct
    /// them here or the entire GPU-side load path, and therefore every claim
    /// hot reload rests on, is untestable.
    fn insert_headless_gpu_registries(app: &mut bevy_app::App) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::None,
            compatible_surface: None,
            force_fallback_adapter: false,
        }))
        .expect("a headless adapter; the rest of this suite already requires one");
        let (device, queue) = pollster::block_on(adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("bsengine-gltf test device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::downlevel_defaults(),
                memory_hints: wgpu::MemoryHints::default(),
            },
            None,
        ))
        .expect("headless device request");
        let device = std::sync::Arc::new(device);
        let queue = std::sync::Arc::new(queue);
        app.insert_resource(GpuMeshRegistry::new(device.clone()));
        app.insert_resource(GpuTextureRegistry::new(device, queue));
    }

    #[test]
    fn a_loaded_gltf_keeps_its_handle_so_a_reload_can_reach_it() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");
        let path = fixture.to_str().unwrap().to_owned();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        insert_headless_gpu_registries(&mut app);
        let e = app.world_mut().spawn(GltfAsset::new(path.clone())).id();

        for _ in 0..200 {
            app.update();
            if app.world().get::<GltfLoaded>(e).is_some() {
                break;
            }
        }

        let loaded = app.world().get::<GltfLoaded>(e).expect(
            "a resolved glTF must record what it created, or a reload \
             has nothing to rebuild",
        );
        assert!(
            !loaded.mesh_ids.is_empty(),
            "the recorded mesh ids are what hot reload replaces in place"
        );
        let asset_id = loaded.handle.id();
        let recorded_mesh_ids = loaded.mesh_ids.clone();
        let recorded_texture_ids = loaded.texture_ids.clone();
        assert_eq!(
            app.world().get::<MeshRenderer>(e).map(|m| m.mesh_id),
            recorded_mesh_ids.first().copied(),
            "GltfLoaded must sit on the same entity as MeshRenderer and record \
             the id that entity actually draws, or the rebuild would refresh \
             geometry nothing is rendering"
        );

        // The texture ids are stored flattened out of a `Vec<Option<u64>>`, so
        // a miscount here would silently misalign the rebuild's zip against
        // `LoadedGltf::images` and repaint meshes with the wrong image.
        let image_count = app
            .world()
            .resource::<Assets<LoadedGltf>>()
            .get(asset_id)
            .expect("the retained handle keeps the asset alive")
            .images
            .len();
        assert_eq!(
            recorded_texture_ids.len(),
            image_count,
            "every image the load uploaded must be recorded, in images order"
        );

        // Read Modified specifically, rather than `Events::len() > 0`: the
        // buffer still holds the events the *load* emitted, so a bare length
        // check would pass even if the reload reached nothing at all.
        let mut reader: ManualEventReader<AssetEvent<LoadedGltf>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<LoadedGltf>>>()
            .get_reader();
        {
            let events = app.world().resource::<Events<AssetEvent<LoadedGltf>>>();
            let _ = reader.read(events).count();
        }

        // Registry ids are handed out sequentially, so a probe taken either
        // side of the reload detects any id allocated in between.
        let (probe_v, probe_i) = bsengine_rhi_wgpu::triangle_vertices();
        let probe_before = app
            .world_mut()
            .resource_mut::<GpuMeshRegistry>()
            .register(&probe_v, &probe_i);

        app.world().resource::<AssetServer>().reload(path);
        let mut saw_modified = false;
        for _ in 0..60 {
            app.update();
            let events = app.world().resource::<Events<AssetEvent<LoadedGltf>>>();
            if reader
                .read(events)
                .any(|ev| matches!(ev, AssetEvent::Modified { id } if *id == asset_id))
            {
                saw_modified = true;
                break;
            }
        }
        assert!(
            saw_modified,
            "reloading a loaded glTF must emit AssetEvent::Modified for the \
             retained handle; none means the handle was dropped and hot reload \
             is impossible for glTF"
        );

        // The whole design in one assertion: the rebuild swaps buffer contents
        // under the ids the load already handed out, so nothing has to find the
        // entities -- including the extra ones a multi-mesh glTF spawns, which
        // are unreachable from here.
        let probe_after = app
            .world_mut()
            .resource_mut::<GpuMeshRegistry>()
            .register(&probe_v, &probe_i);
        assert_eq!(
            probe_after,
            probe_before + 1,
            "the reload allocated fresh mesh ids instead of replacing in place; \
             MeshRenderer.mesh_id would now point at the pre-reload geometry"
        );
        for id in &recorded_mesh_ids {
            assert!(
                app.world().resource::<GpuMeshRegistry>().get(*id).is_some(),
                "recorded mesh id {id} is gone from the registry after a reload"
            );
        }

        // Reloading the file reproduces identical geometry, so it cannot show
        // that the rebuild reached the GPU at all. Replacing the asset's data
        // -- which is what a changed file amounts to -- makes the effect
        // visible under the id recorded at load time.
        let bounds_before = app
            .world()
            .resource::<GpuMeshRegistry>()
            .get_bounds(recorded_mesh_ids[0])
            .expect("a recorded id must be live before the rebuild");
        {
            let mut assets = app.world_mut().resource_mut::<Assets<LoadedGltf>>();
            let data = assets
                .get_mut(asset_id)
                .expect("the retained handle keeps the asset mutable in place");
            for v in &mut data.meshes[0].vertices {
                v.position[0] *= 10.0;
            }
        }
        // `Assets::asset_events` flushes in PostUpdate, so the Modified event
        // lands in the *next* Update, where rebuild_modified_gltf reads it.
        for _ in 0..5 {
            app.update();
        }
        let bounds_after = app
            .world()
            .resource::<GpuMeshRegistry>()
            .get_bounds(recorded_mesh_ids[0])
            .expect("a recorded id must survive the rebuild");
        assert_ne!(
            bounds_before, bounds_after,
            "the id recorded at load time still holds the pre-reload geometry: \
             the rebuild never reached the GPU, so a hot reload would change \
             nothing on screen"
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

    // Drives the real system, which is the only way to catch the failure
    // mode the test above cannot see: re-calling `AssetServer::load` for a
    // path whose state is already `Failed` resets it to `Loading` and
    // restarts the load (bevy_asset 0.14.2, server/info.rs:216-221). Since
    // `LoadState::Failed` is set in PreUpdate and this system runs in Update,
    // a loop that re-requests every frame erases the failure before it can
    // ever observe it -- retrying a missing file forever and spawning a fresh
    // filesystem task each frame, which is strictly worse than the blocking
    // path that warned once and stopped.
    #[test]
    fn missing_gltf_is_given_up_on_instead_of_retried_forever() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        let e = app
            .world_mut()
            .spawn(GltfAsset::new("definitely/not/a/real/file.glb"))
            .id();

        let mut gave_up = false;
        for _ in 0..200 {
            app.update();
            if app.world().get::<GltfAsset>(e).is_none() {
                gave_up = true;
                break;
            }
        }
        assert!(
            gave_up,
            "a GltfAsset with an unloadable path must be given up on, not \
             retried on every frame forever"
        );
    }
}
