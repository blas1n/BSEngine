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
/// `GltfPlugin` does not register this type for reflection, and that is
/// deliberate: the plugin is absent from the headless
/// `bsengine-runtime --test` app (it needs the GPU registries), so a
/// registration made here would be missing from exactly the host the E2E
/// replays run in. `bsengine_scene::register_gameplay_reflect_types` — which
/// both hosts call, and which already depends on this crate — registers it
/// instead.
#[derive(Component, Clone, Debug, bevy_reflect::Reflect)]
#[reflect(Component)]
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
/// See [`bsengine_asset::AssetSlot`] for why re-requesting a path whose load
/// has failed is the specific thing that must not happen, and why the handle is
/// kept strong between frames.
///
/// The terminal state here is not [`bsengine_asset::AssetSlot::GaveUp`] but the
/// removal of this component *and* `GltfAsset`: with nothing left naming the
/// path, a re-request becomes structurally impossible rather than merely
/// avoided. The slot reports; [`load_gltf_assets`] decides what that means.
///
/// Internal to this system — `GltfAsset.path` stays a plain `String`, so scene
/// RON, the scripting API and the MCP tools are unaffected.
#[derive(Component)]
struct PendingGltf(bsengine_asset::AssetSlot<LoadedGltf>);

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
    mut query: Query<
        (
            Entity,
            &GltfAsset,
            Option<&mut PendingGltf>,
            Option<&Transform>,
        ),
        Without<MeshRenderer>,
    >,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut tex_registry: Option<ResMut<GpuTextureRegistry>>,
    mut gltf_assets: ResMut<bevy_asset::Assets<LoadedGltf>>,
    asset_server: Res<bevy_asset::AssetServer>,
) {
    for (entity, asset, pending, existing_transform) in query.iter_mut() {
        // Request exactly once, then retain the handle. See `PendingGltf`.
        let Some(mut pending) = pending else {
            match bsengine_asset::load(
                bsengine_asset::LoadMode::Async,
                &asset_server,
                &mut gltf_assets,
                &asset.path,
                GltfLoader::load_full,
            ) {
                Ok(handle) => {
                    commands
                        .entity(entity)
                        .insert(PendingGltf(bsengine_asset::AssetSlot::from_handle(handle)));
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

        if let bsengine_asset::Polled::Failed(e) = pending.0.poll(&asset_server, &gltf_assets) {
            // A failed load never resolves, so the path is dropped entirely --
            // otherwise a missing file retries silently forever, where the old
            // blocking path warned once and gave up.
            warn!("Failed to load GLTF {}: {e}", asset.path);
            commands.entity(entity).remove::<(GltfAsset, PendingGltf)>();
            continue;
        }
        // Deliberately not `Arrived`-only: the data can land before
        // `GpuMeshRegistry` exists, and the spawn below then retries every frame
        // until it does. Acting only on the arrival frame would drop those loads.
        // Cloned out so the handle outlives the borrow of `pending`; a `Handle`
        // is refcounted, so this is a bump rather than a copy of the asset.
        let handle = pending.0.handle().clone();
        let Some(loaded) = gltf_assets.get(&handle) else {
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
                handle: handle.clone(),
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
///
/// It also refreshes the CPU-side skinning state, which is not an optional
/// extra: `update_skinned_meshes` runs in PostUpdate and overwrites the very
/// buffer replaced here, re-deriving it from `SkinnedMesh.rest_vertices`. Leave
/// that at its load-time value and the reload is stomped in the same frame it
/// landed. See [`refresh_skinning`].
fn rebuild_modified_gltf(
    mut events: bevy_ecs::prelude::EventReader<bevy_asset::AssetEvent<LoadedGltf>>,
    mut query: Query<(
        &GltfLoaded,
        Option<&mut SkinnedMesh>,
        Option<&mut AnimationClipLibrary>,
        Option<&mut AnimationPlayer>,
    )>,
    gltf_assets: Res<bevy_asset::Assets<LoadedGltf>>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut tex_registry: Option<ResMut<GpuTextureRegistry>>,
) {
    for event in events.read() {
        let bevy_asset::AssetEvent::Modified { id } = event else {
            continue;
        };
        for (loaded, skinned, library, player) in
            query.iter_mut().filter(|(l, ..)| l.handle.id() == *id)
        {
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
                    if !reg.replace(*mesh_id, &mesh_data.vertices, &mesh_data.indices) {
                        warn!(
                            "mesh id {mesh_id} recorded at load time is no longer \
                             registered; whatever draws it keeps the pre-reload geometry"
                        );
                    }
                }
            }
            if let Some(reg) = tex_registry.as_mut() {
                if data.images.len() != loaded.texture_ids.len() {
                    warn!(
                        "reloaded glTF now has {} image(s), was {} -- rebuilding \
                         the overlap only; a structural change needs a restart",
                        data.images.len(),
                        loaded.texture_ids.len()
                    );
                }
                for (tex_id, img) in loaded.texture_ids.iter().zip(data.images.iter()) {
                    if !reg.replace(*tex_id, img.width, img.height, &img.rgba) {
                        warn!(
                            "texture id {tex_id} recorded at load time is no longer \
                             loaded; whatever samples it keeps the pre-reload pixels"
                        );
                    }
                }
            }
            refresh_skinning(data, skinned, library, player);
        }
    }
}

/// Re-derives an entity's CPU-side skinning state from reloaded asset data.
///
/// Must mirror what `load_gltf_assets` builds for the *first* mesh — the only
/// one that gets skinning — so the two have to be kept in step. It exists
/// because `update_skinned_meshes` (PostUpdate) re-uploads the same GPU buffer
/// `rebuild_modified_gltf` just replaced, deforming from `rest_vertices`: a
/// stale rest pose discards the reload one schedule later, and a stale rest
/// pose that is *longer* than the rebuilt buffer is a wgpu `BufferOverrun`.
///
/// `mesh_id` is deliberately left alone — the rebuild swapped contents under it,
/// so it still names the buffer this entity draws — and so is `player.clip`, in
/// case an `AnimationStateMachine` chose it. Only `player.duration` is re-taken,
/// since `AnimationPlayer::tick` wraps on it when looping and is a no-op at
/// `<= 0`, so an edited clip length would otherwise never take effect.
///
/// Gaining or losing a skin across a reload is a structural change of the same
/// kind as a changed mesh count: it would mean inserting or removing components
/// on an entity the reload has no other reason to touch. Both warn and leave the
/// entity as it is, matching what this system already does for extra meshes.
fn refresh_skinning(
    data: &LoadedGltf,
    skinned: Option<bevy_ecs::prelude::Mut<SkinnedMesh>>,
    library: Option<bevy_ecs::prelude::Mut<AnimationClipLibrary>>,
    player: Option<bevy_ecs::prelude::Mut<AnimationPlayer>>,
) {
    // Same gate as the load path: a skin only counts if the first mesh carries
    // per-vertex bindings *and* the document defines a skin to bind them to.
    let reloaded_skin = data
        .meshes
        .first()
        .and_then(|m| m.skin.clone())
        .filter(|_| !data.skins.is_empty());

    let Some(mut skinned) = skinned else {
        if reloaded_skin.is_some() {
            warn!(
                "reloaded glTF gained a skin, but it was imported without one -- \
                 it stays unskinned; attaching skinning needs a restart"
            );
        }
        return;
    };
    let (Some(mesh_data), Some(skin_verts)) = (data.meshes.first(), reloaded_skin) else {
        // Deliberately leaves rest_vertices alone rather than half-updating it:
        // the entity keeps deforming its pre-reload pose. If that pose no longer
        // fits the rebuilt buffer, `GpuMeshRegistry::update_vertices` refuses
        // the upload rather than letting wgpu panic.
        warn!(
            "reloaded glTF no longer has a skinned first mesh -- the entity keeps \
             its pre-reload rest pose and skeleton; removing skinning needs a restart"
        );
        return;
    };

    skinned.rest_vertices = mesh_data.vertices.clone();
    skinned.skin = skin_verts;
    skinned.skin_data = data.skins[0].clone();
    skinned.nodes = data.nodes.clone();

    let Some(mut library) = library else {
        return;
    };
    *library = AnimationClipLibrary::from_clips(data.animations.clone());

    let Some(mut player) = player else {
        return;
    };
    match library.clips.get(&player.clip) {
        Some(clip) => player.duration = clip.duration.max(0.0),
        None => warn!(
            "reloaded glTF no longer defines clip '{}'; the player keeps it and \
             the mesh holds the rebuilt rest pose until another clip is selected",
            player.clip
        ),
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
    // A file watcher reports native OS paths, while assets are loaded with the
    // engine's own form (`ProjectDir`-joined, forward slashes, relative to the
    // CWD). This pins how much of that difference `AssetServer::reload`
    // tolerates, because a mismatch is a *silent* no-op -- no warning, no
    // event, the watcher simply does nothing.
    //
    // Measured: on Windows, where `/` and `\` are both separators, either
    // spelling matches. A canonicalised spelling -- `..` segments resolved,
    // plus Windows' `\\?\` verbatim prefix -- does not, on either platform. So
    // the watcher must reconstruct the engine-form path from the event rather
    // than handing over what the OS gave it.
    //
    // The separator half is asserted `#[cfg(windows)]` only. On Unix `\` is a
    // legal filename character, so swapping separators there names a different,
    // nonexistent file rather than respelling the same one -- CI caught this
    // test claiming otherwise.
    //
    // If the canonicalised case ever starts matching, this test fails and that
    // is good news: the reconstruction in the watcher could then be dropped.
    #[test]
    fn reload_tolerates_separator_style_but_not_a_canonicalised_path() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");
        let loaded_form = fixture.to_str().unwrap().to_owned();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(GltfPlugin);

        let handle = {
            let server = app.world().resource::<AssetServer>();
            server.load::<LoadedGltf>(loaded_form.clone())
        };

        let mut ok = false;
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .resource::<Assets<LoadedGltf>>()
                .get(&handle)
                .is_some()
            {
                ok = true;
                break;
            }
        }
        assert!(ok, "fixture must load first");

        let mut reader: ManualEventReader<AssetEvent<LoadedGltf>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<LoadedGltf>>>()
            .get_reader();

        // Spellings a watcher could plausibly hand us, all naming this file.
        let backslashed = loaded_form.replace('/', "\\");
        let forward = loaded_form.replace('\\', "/");
        let canonical = std::fs::canonicalize(&fixture)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|e| format!("<canonicalize failed: {e}>"));

        let mut modified_count = |app: &mut bevy_app::App, spelling: String| -> usize {
            {
                let events = app.world().resource::<Events<AssetEvent<LoadedGltf>>>();
                let _ = reader.read(events).count();
            }
            app.world().resource::<AssetServer>().reload(spelling);
            let mut hits = 0;
            for _ in 0..40 {
                app.update();
                let events = app.world().resource::<Events<AssetEvent<LoadedGltf>>>();
                hits += reader
                    .read(events)
                    .filter(|e| matches!(e, AssetEvent::Modified { .. }))
                    .count();
            }
            hits
        };

        assert_eq!(
            modified_count(&mut app, loaded_form.clone()),
            1,
            "reloading with the exact spelling the asset was loaded with must work"
        );
        // Windows only, and not an oversight: on Unix `\` is a perfectly legal
        // *filename* character, so swapping `/` for it does not respell the
        // same file -- it names a different, nonexistent one, and `reload`
        // correctly does nothing. The claim "separator direction is free" is
        // therefore a statement about Windows, where both are separators.
        #[cfg(windows)]
        assert_eq!(
            modified_count(&mut app, backslashed),
            1,
            "on Windows both separators name the same file, so the watcher's \
             native backslashes must not need normalising before reload"
        );
        #[cfg(not(windows))]
        let _ = backslashed;
        assert_eq!(
            modified_count(&mut app, forward),
            1,
            "the forward-slash spelling of the same path must work too"
        );
        assert_eq!(
            modified_count(&mut app, canonical.clone()),
            0,
            "a canonicalised spelling ({canonical}) does NOT match, which is why \
             the watcher reconstructs the engine-form path instead of forwarding \
             the OS path. If this ever starts matching, delete that \
             reconstruction."
        );
    }

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
    ///
    /// `GpuQueueResource` is inserted for the same reason, and is not optional
    /// dressing: `update_skinned_meshes` (PostUpdate) early-returns without it,
    /// so a helper that supplies only the two registries silently excludes the
    /// skinning system from every test built on it. `WgpuRHIPlugin` inserts all
    /// three together at surface-creation time, so supplying a strict subset
    /// here would be a configuration the engine itself never produces.
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
        app.insert_resource(GpuTextureRegistry::new(device, queue.clone()));
        app.insert_resource(bsengine_rhi_wgpu::GpuQueueResource(queue));
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

    /// Boots an app with *both* halves of the skinned-glTF pipeline —
    /// `GltfPlugin` (Update: load, then rebuild on `Modified`) and
    /// `SkinnedMeshPlugin` (PostUpdate: deform and re-upload) — loads `fox.glb`
    /// onto one entity, and runs until it resolves into `GltfLoaded` +
    /// `SkinnedMesh`.
    ///
    /// This is the shipping configuration, not a contrived one:
    /// `bsengine-runtime/src/main.rs` adds both plugins, and `fox.glb` — the
    /// only glTF `games/mini-arena/assets/scenes/main.ron` loads — is skinned
    /// (one skin, `JOINTS_0` on its primitive, three clips).
    ///
    /// Returns the app, the entity, and the asset id, and asserts on the way out
    /// that `update_skinned_meshes` cannot early-return or skip this entity —
    /// see the block at the end for why that matters.
    fn load_skinned_fox() -> (bevy_app::App, Entity, bevy_asset::AssetId<LoadedGltf>) {
        use crate::skinned_mesh::{AnimationClipLibrary, SkinnedMesh, SkinnedMeshPlugin};

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/models/fox.glb");
        let path = fixture.to_str().unwrap().to_owned();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin);
        app.add_plugins(GltfPlugin);
        app.add_plugins(SkinnedMeshPlugin);
        insert_headless_gpu_registries(&mut app);
        let e = app.world_mut().spawn(GltfAsset::new(path)).id();

        for _ in 0..200 {
            app.update();
            if app.world().get::<SkinnedMesh>(e).is_some() {
                break;
            }
        }
        let asset_id = app
            .world()
            .get::<GltfLoaded>(e)
            .expect("fox.glb must resolve into GltfLoaded within 200 frames")
            .handle
            .id();
        assert!(
            app.world().get::<SkinnedMesh>(e).is_some(),
            "fox.glb is skinned, so the import must attach SkinnedMesh -- without \
             it the reload-vs-skinning interaction under test cannot arise at all"
        );

        // `update_skinned_meshes` returns before touching anything unless both
        // GPU resources are present, and `continue`s past any entity whose
        // player names a clip its library lacks. Pinning all of that here is
        // what makes the tests below statements about the *interaction*: the
        // branch's original helper inserted only the two registries, so the
        // skinning system never ran under it and the buffer it stomps every
        // frame was invisible to every test built on it.
        assert!(
            app.world().get_resource::<GpuMeshRegistry>().is_some(),
            "update_skinned_meshes early-returns without GpuMeshRegistry"
        );
        assert!(
            app.world()
                .get_resource::<bsengine_rhi_wgpu::GpuQueueResource>()
                .is_some(),
            "update_skinned_meshes early-returns without GpuQueueResource -- \
             omitting it is what hid this whole class of defect"
        );
        let clip = app
            .world()
            .get::<AnimationPlayer>(e)
            .expect("a skinned glTF import attaches an AnimationPlayer")
            .clip
            .clone();
        assert!(
            app.world()
                .get::<AnimationClipLibrary>(e)
                .expect("a skinned glTF import attaches its clip library")
                .clips
                .contains_key(&clip),
            "the player's clip must resolve in the library, or update_skinned_meshes \
             skips this entity and never uploads"
        );

        (app, e, asset_id)
    }

    /// `rebuild_modified_gltf` (Update) and `update_skinned_meshes` (PostUpdate)
    /// both own the same GPU vertex buffer: the first replaces its contents from
    /// the reloaded asset, the second overwrites them every frame from
    /// `SkinnedMesh.rest_vertices` — a `clone()` taken at *load* time. So unless
    /// the rebuild also refreshes that CPU-side copy (and the skin, skeleton and
    /// clips beside it), PostUpdate stomps the fresh geometry with vertices
    /// derived from the pre-reload data in the very frame the reload landed, and
    /// hot reload is silently a no-op for every skinned glTF — which is every
    /// glTF the shipping game loads.
    #[test]
    fn reloading_a_skinned_gltf_refreshes_the_rest_pose_the_skinner_deforms_from() {
        use crate::skinned_mesh::{AnimationClipLibrary, SkinnedMesh};
        use bevy_asset::Assets;

        let (mut app, e, asset_id) = load_skinned_fox();

        // A re-export moves geometry, skin weights, skeleton and clip lengths
        // together, so all five are changed here. Mutating the asset in place is
        // how a changed file is expressed without a second .glb on disk:
        // `Assets::get_mut` is what queues `AssetEvent::Modified`, the only
        // trigger `rebuild_modified_gltf` has.
        {
            let mut assets = app.world_mut().resource_mut::<Assets<LoadedGltf>>();
            let data = assets
                .get_mut(asset_id)
                .expect("the retained handle keeps the asset mutable in place");
            for v in &mut data.meshes[0].vertices {
                v.position[1] += 100.0;
            }
            for s in data.meshes[0]
                .skin
                .as_mut()
                .expect("fox.glb's first primitive carries JOINTS_0/WEIGHTS_0")
            {
                // += rather than = : guaranteed different from whatever was
                // imported, so a stale copy cannot coincidentally match.
                s.weights[0] += 1.0;
            }
            data.nodes[0].position[0] += 7.0;
            data.skins[0].inverse_bind_matrices[0][3][0] += 5.0;
            for clip in &mut data.animations {
                clip.duration += 1.0;
            }
        }
        // `Assets::asset_events` flushes in PostUpdate, so the Modified event
        // lands in the *next* Update where rebuild_modified_gltf reads it; the
        // PostUpdate after that is where a stale rest pose would stomp it.
        for _ in 0..5 {
            app.update();
        }

        let (expected_positions, expected_skin, expected_nodes, expected_ibm, expected_clips) = {
            let assets = app.world().resource::<Assets<LoadedGltf>>();
            let data = assets.get(asset_id).expect("the asset outlives the reload");
            (
                data.meshes[0]
                    .vertices
                    .iter()
                    .map(|v| v.position)
                    .collect::<Vec<_>>(),
                data.meshes[0]
                    .skin
                    .as_ref()
                    .unwrap()
                    .iter()
                    .map(|s| (s.joints, s.weights))
                    .collect::<Vec<_>>(),
                data.nodes.clone(),
                data.skins[0].inverse_bind_matrices[0],
                data.animations
                    .iter()
                    .map(|c| (c.name.clone(), c.duration))
                    .collect::<Vec<_>>(),
            )
        };

        let skinned = app
            .world()
            .get::<SkinnedMesh>(e)
            .expect("the entity keeps its SkinnedMesh across a reload");
        assert_eq!(
            skinned
                .rest_vertices
                .iter()
                .map(|v| v.position)
                .collect::<Vec<_>>(),
            expected_positions,
            "rest_vertices still holds the pre-reload geometry, so \
             update_skinned_meshes re-derives from it every PostUpdate and \
             re-uploads into the very buffer rebuild_modified_gltf just replaced \
             -- the reload is discarded in the frame it landed"
        );
        assert_eq!(
            skinned
                .skin
                .iter()
                .map(|s| (s.joints, s.weights))
                .collect::<Vec<_>>(),
            expected_skin,
            "the per-vertex skin is stale: re-weighted vertices would deform \
             through the old bindings"
        );
        assert_eq!(
            skinned.nodes, expected_nodes,
            "the node hierarchy is stale: a re-exported skeleton would animate \
             from the old bind pose"
        );
        assert_eq!(
            skinned.skin_data.inverse_bind_matrices[0], expected_ibm,
            "the skin's inverse bind matrices are stale"
        );

        let library = app
            .world()
            .get::<AnimationClipLibrary>(e)
            .expect("the entity keeps its clip library across a reload");
        for (name, duration) in &expected_clips {
            assert_eq!(
                library.clips.get(name).map(|c| c.duration),
                Some(*duration),
                "clip '{name}' is stale: an edited animation would play at its \
                 pre-reload length"
            );
        }
        let player = app
            .world()
            .get::<AnimationPlayer>(e)
            .expect("the entity keeps its AnimationPlayer across a reload");
        assert_eq!(
            player.duration, library.clips[&player.clip].duration,
            "the player's duration must follow the reloaded clip: `tick` wraps on \
             it when looping and is a no-op at <= 0, so a stale duration keeps \
             playback on the old clip length"
        );
    }

    /// The case that kills the process rather than merely looking wrong: a
    /// reload that *shrinks* a skinned mesh. `replace` sizes the new vertex
    /// buffer to the new data, while `update_vertices` writes at offset 0, so a
    /// stale (longer) rest pose is a wgpu `BufferOverrun` — and since nothing in
    /// this codebase installs an error scope or `on_uncaptured_error`, that
    /// reaches wgpu's default handler, which panics.
    ///
    /// Reaching the assertions at all is therefore half of what this test
    /// measures; the other half is that the two halves of `SkinnedMesh` stay the
    /// same length, since `update_skinned_meshes` zips them and a half-refreshed
    /// pair would silently upload a truncated mesh.
    #[test]
    fn a_skinned_gltf_that_loses_vertices_on_reload_neither_panics_nor_desyncs() {
        use crate::skinned_mesh::SkinnedMesh;
        use bevy_asset::Assets;

        let (mut app, e, asset_id) = load_skinned_fox();

        let before = app
            .world()
            .get::<SkinnedMesh>(e)
            .expect("loaded above")
            .rest_vertices
            .len();
        assert!(
            before > 3,
            "the fixture must start with more vertices than the reload leaves, \
             or nothing shrinks and this test proves nothing (was {before})"
        );

        {
            let mut assets = app.world_mut().resource_mut::<Assets<LoadedGltf>>();
            let data = assets
                .get_mut(asset_id)
                .expect("the retained handle keeps the asset mutable in place");
            data.meshes[0].vertices.truncate(3);
            data.meshes[0]
                .skin
                .as_mut()
                .expect("fox.glb's first primitive is skinned")
                .truncate(3);
            data.meshes[0].indices = vec![0, 1, 2];
        }
        for _ in 0..5 {
            app.update();
        }

        let skinned = app
            .world()
            .get::<SkinnedMesh>(e)
            .expect("the entity keeps its SkinnedMesh across a reload");
        assert_eq!(
            skinned.rest_vertices.len(),
            3,
            "the rest pose must shrink with the asset; a longer one is written \
             straight past the end of the buffer replace just rebuilt"
        );
        assert_eq!(
            skinned.skin.len(),
            skinned.rest_vertices.len(),
            "rest_vertices and skin are zipped every frame -- refreshing one \
             without the other silently uploads the shorter of the two"
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
