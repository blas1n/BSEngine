//! Turning a texture path into a `Material.texture_id`.
//!
//! `GpuTextureRegistry` hands out ids and never asks where a texture came from,
//! so something has to own the question "is this path already on the GPU?".
//! That is this cache, and answering it is what stops ten entities sharing one
//! image from uploading it ten times.
//!
//! The load state machine that used to live here is now
//! [`bsengine_asset::AssetSlot`], shared with the skybox, custom shaders,
//! scripts and glTF. What stays is the part that is only about textures: one
//! GPU id per path.

use std::collections::HashMap;

use bevy_ecs::prelude::*;
use bsengine_asset::TextureAsset;
use bsengine_core::{Material, TexturePath};
use bsengine_rhi_wgpu::GpuTextureRegistry;

/// One texture path's load, and the GPU id it produced.
///
/// Two facts, not one: the load either arrived or did not, and separately the
/// image either reached the GPU or is still waiting for a registry. They used
/// to share an enum, whose `Ready` therefore could not describe an image that
/// had decoded before a surface existed.
struct CachedTexture {
    /// The load. Its handle is retained even after the upload, because dropping
    /// it releases the asset and switches hot reload off silently.
    slot: bsengine_asset::AssetSlot<TextureAsset>,
    /// `Some` once the image is on the GPU.
    id: Option<u64>,
}

/// One entry per texture path.
#[derive(Resource, Default)]
pub struct TextureCache {
    by_path: HashMap<String, CachedTexture>,
}

impl TextureCache {
    /// The GPU id for a path, once it has finished uploading.
    pub fn id_for(&self, path: &str) -> Option<u64> {
        self.by_path.get(path).and_then(|c| c.id)
    }

    /// Whether this path reached a terminal failure.
    ///
    /// Distinct from "has no id": a path still loading also has no id, and a
    /// test that cannot tell those apart cannot tell a give-up from an infinite
    /// retry, which is the failure mode [`bsengine_asset::AssetSlot::GaveUp`]
    /// exists to prevent.
    pub fn gave_up(&self, path: &str) -> bool {
        self.by_path.get(path).is_some_and(|c| c.slot.gave_up())
    }

    /// How many distinct paths have reached the GPU. Uploading each one exactly
    /// once is what this cache is for, so tests count it.
    pub fn uploaded_count(&self) -> usize {
        self.by_path.values().filter(|c| c.id.is_some()).count()
    }

    /// Requests `path` if this is the first time anyone asked, polls it, and
    /// uploads it once it arrives. Returns the GPU id when there is one.
    ///
    /// Split out of [`resolve_texture_paths`] so that callers with nowhere to
    /// write an id -- UI images, which are not entities -- go through exactly
    /// the same request-once, upload-once path that entities do, rather than a
    /// second copy of it that could diverge.
    fn ensure_uploaded(
        &mut self,
        path: &str,
        asset_server: &bevy_asset::AssetServer,
        textures: &bevy_asset::Assets<TextureAsset>,
        registry: &mut GpuTextureRegistry,
    ) -> Option<u64> {
        // Requested here, the first time anyone asks for this path, so
        // "request exactly once" is a property of the map rather than of any
        // caller's control flow.
        let entry = self
            .by_path
            .entry(path.to_string())
            .or_insert_with(|| CachedTexture {
                slot: bsengine_asset::AssetSlot::requesting(asset_server, path),
                id: None,
            });

        match entry.slot.poll(asset_server, textures) {
            bsengine_asset::Polled::Arrived => {
                if let Some(tex) = textures.get(entry.slot.handle()) {
                    entry.id =
                        Some(registry.load_with(tex.width, tex.height, &tex.data, tex.settings));
                }
            }
            bsengine_asset::Polled::Failed(e) => {
                tracing::warn!("[texture] '{path}' failed to load: {e}");
            }
            bsengine_asset::Polled::Nothing => {}
        }
        entry.id
    }
}

/// Re-uploads a cached texture whose asset was modified -- a saved edit to
/// the image, or to the import settings in the sidecar beside it -- under
/// the id every `Material` already holds.
///
/// Until this existed a material texture was uploaded once and never again:
/// the asset watcher reloaded the file into `Assets<TextureAsset>`, the
/// `Modified` event fired, and nothing listened, so the GPU copy stayed as it
/// was while the skybox (which had its own handler) and glTF meshes (theirs)
/// updated. The sidecar path makes the gap visible in a way a pixel edit did
/// not: toggling `srgb` in a `.meta` is *only* an upload change.
pub fn reupload_modified_textures(
    mut events: bevy_ecs::prelude::EventReader<bevy_asset::AssetEvent<TextureAsset>>,
    cache: Res<TextureCache>,
    textures: Res<bevy_asset::Assets<TextureAsset>>,
    mut registry: Option<ResMut<GpuTextureRegistry>>,
) {
    for event in events.read() {
        let bevy_asset::AssetEvent::Modified { id } = event else {
            continue;
        };
        let Some(registry) = registry.as_mut() else {
            continue;
        };
        for (path, entry) in cache.by_path.iter() {
            if entry.slot.handle().id() != *id {
                continue;
            }
            let (Some(gpu_id), Some(tex)) = (entry.id, textures.get(entry.slot.handle())) else {
                continue;
            };
            if !registry.replace_with(gpu_id, tex.width, tex.height, &tex.data, tex.settings) {
                tracing::warn!(
                    "[texture] '{path}' was modified but its GPU texture {gpu_id} is no longer \
                     registered; whatever samples it keeps the pre-edit pixels"
                );
            }
        }
    }
}

/// Requests, polls and uploads the textures entities are waiting for, then
/// writes the resulting id onto their `Material`.
///
/// An entity keeps its `TexturePath` afterwards -- it is the record of what was
/// asked for, and the editor writes it back out when saving. The work is not
/// repeated because a `Material` that already has an id is skipped.
pub fn resolve_texture_paths(
    mut cache: ResMut<TextureCache>,
    mut wanting: Query<(&TexturePath, &mut Material)>,
    ui_state: Option<Res<bsengine_core::UiState>>,
    asset_server: Res<bevy_asset::AssetServer>,
    textures: Res<bevy_asset::Assets<TextureAsset>>,
    registry: Option<ResMut<GpuTextureRegistry>>,
) {
    let Some(mut registry) = registry else {
        // No GPU registry yet (headless startup, or the surface has not
        // arrived). Nothing is dropped; the requests are simply still pending.
        return;
    };

    for (wanted, mut material) in wanting.iter_mut() {
        if material.texture_id.is_some() {
            continue;
        }
        let id = cache.ensure_uploaded(wanted.0.as_str(), &asset_server, &textures, &mut registry);
        if let Some(id) = id {
            material.texture_id = Some(id);
        }
    }

    // UI images want the same thing an entity does -- a path on the GPU -- but
    // they are not entities and have no `Material` to write an id onto, so the
    // query above never sees them. Without this a `UiWidget::Image` names a
    // texture that is never requested, which is one of the two reasons that
    // widget drew nothing at all.
    if let Some(ui) = ui_state {
        for widget in &ui.widgets {
            if let bsengine_core::UiWidget::Image { texture_path, .. } = widget {
                if !texture_path.is_empty() {
                    cache.ensure_uploaded(texture_path, &asset_server, &textures, &mut registry);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_app::new_app;

    /// A real headless device, and the registry built on it.
    ///
    /// `WgpuRHIPlugin` only builds a `GpuTextureRegistry` next to a swapchain
    /// surface, which needs a window -- the same reason `bsengine-gltf`'s tests
    /// construct theirs by hand. Without one, `resolve_texture_paths` takes its
    /// early return and every test here would pass by doing nothing.
    fn with_gpu(app: &mut bevy_app::App) {
        let surface = pollster::block_on(bsengine_rhi_wgpu::surface::WgpuSurface::new_offscreen(
            16, 16, false,
        ))
        .expect("these tests need an adapter; a skip here would look like a pass");
        app.insert_resource(GpuTextureRegistry::new(
            surface.device_arc(),
            surface.queue_arc(),
        ));
    }

    fn test_app() -> bevy_app::App {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        with_gpu(&mut app);
        app.init_resource::<TextureCache>();
        app.add_systems(
            bevy_app::Update,
            (resolve_texture_paths, reupload_modified_textures),
        );
        app
    }

    /// The property the sidecar story depends on: an asset modified after it
    /// was uploaded is uploaded again under the same id, with its new pixels
    /// and its new settings. Driven through `Assets::get_mut`, which is what
    /// a reload ends in, rather than through the file watcher -- that half is
    /// `bsengine-asset`'s to prove.
    #[test]
    fn a_modified_texture_asset_is_reuploaded_under_its_existing_id() {
        use bsengine_core::TextureImportSettings;

        let mut app = test_app();
        app.world_mut()
            .spawn((Material::default(), TexturePath(REAL_TEXTURE.into())));
        for _ in 0..60 {
            app.update();
        }
        let gpu_id = app
            .world()
            .resource::<TextureCache>()
            .id_for(REAL_TEXTURE)
            .expect("premise: the texture uploaded");
        let handle = app.world().resource::<TextureCache>().by_path[REAL_TEXTURE]
            .slot
            .handle()
            .clone();
        {
            let registry = app.world().resource::<GpuTextureRegistry>();
            let (w, h) = registry.get_size(gpu_id).unwrap();
            assert_ne!((w, h), (2, 2), "premise: the checker is not already 2x2");
            assert_eq!(
                registry.get_settings(gpu_id),
                Some(TextureImportSettings::default()),
                "premise: a file texture without a sidecar uploads with the defaults"
            );
        }

        // What a reload produces: the same handle, new contents. `get_mut`
        // marks the asset modified, which is the event under test.
        let data_settings = TextureImportSettings {
            srgb: false,
            ..Default::default()
        };
        {
            let mut textures = app
                .world_mut()
                .resource_mut::<bevy_asset::Assets<TextureAsset>>();
            let tex = textures.get_mut(&handle).expect("the asset is loaded");
            tex.width = 2;
            tex.height = 2;
            tex.data = vec![0u8; 16];
            tex.settings = data_settings;
        }
        for _ in 0..3 {
            app.update();
        }

        let registry = app.world().resource::<GpuTextureRegistry>();
        assert_eq!(
            app.world().resource::<TextureCache>().id_for(REAL_TEXTURE),
            Some(gpu_id),
            "the id materials hold must not change"
        );
        assert_eq!(
            registry.get_size(gpu_id),
            Some((2, 2)),
            "the GPU texture must carry the modified pixels"
        );
        assert_eq!(
            registry.get_settings(gpu_id),
            Some(data_settings),
            "and the modified import settings"
        );
    }

    /// A real image, reached from this crate's directory.
    ///
    /// `AssetPlugin` pins the asset root to the current working directory, and
    /// for a crate test that is the crate directory. A made-up path would make
    /// the dedupe test below pass with *zero* uploads, which is the shape of
    /// vacuous test this repository has been finding all week.
    const REAL_TEXTURE: &str = "../../games/mini-arena/assets/textures/checker.png";

    #[test]
    fn two_entities_sharing_a_texture_upload_it_once() {
        // The reason this cache exists at all. Without it each entity takes its
        // own id and its own copy of the same image on the GPU.
        let mut app = test_app();
        for _ in 0..2 {
            app.world_mut()
                .spawn((Material::default(), TexturePath(REAL_TEXTURE.into())));
        }

        for _ in 0..60 {
            app.update();
        }

        assert_eq!(
            app.world().resource::<TextureCache>().uploaded_count(),
            1,
            "exactly one upload: zero would mean the image never loaded and this              test proved nothing, two would mean the cache is not doing its job"
        );

        // And both entities got that same id.
        let mut q = app.world_mut().query::<&Material>();
        let ids: Vec<Option<u64>> = q.iter(app.world()).map(|m| m.texture_id).collect();
        assert_eq!(ids.len(), 2);
        assert!(
            ids[0].is_some() && ids[0] == ids[1],
            "both should share one id, got {ids:?}"
        );
    }

    #[test]
    fn a_missing_texture_reaches_a_terminal_state_instead_of_retrying_forever() {
        // The hazard this whole state machine exists for. A path that is
        // re-requested every frame sits in Loading forever, so its failure is
        // never observable -- not here, and not in AssetStatuses either.
        let mut app = test_app();
        app.world_mut().spawn((
            Material::default(),
            TexturePath("does/not/exist.png".into()),
        ));

        let gave_up = |app: &bsengine_app::App| {
            app.world()
                .resource::<TextureCache>()
                .gave_up("does/not/exist.png")
        };

        let mut settled = false;
        for _ in 0..80 {
            app.update();
            if gave_up(&app) {
                settled = true;
                break;
            }
        }
        assert!(
            settled,
            "a texture that cannot load has to end up in GaveUp; staying in Loading \
             is indistinguishable from still trying, which is the bug"
        );

        // And stays there on every frame, not merely on the one this happens to
        // sample. A loop that re-requests the failed path passes back through
        // GaveUp repeatedly, so a single late reading cannot tell a give-up
        // from an infinite retry -- the very distinction being tested.
        for frame in 0..40 {
            app.update();
            assert!(
                gave_up(&app),
                "the texture left GaveUp on frame {frame}, which means something \
                 re-requested the failed path"
            );
        }
    }

    #[test]
    fn a_material_that_already_has_a_texture_is_left_alone() {
        // The request is not removed once satisfied -- the editor reads it back
        // when saving -- so the guard against redoing the work is the id being
        // present. If that guard goes, every frame re-runs the lookup.
        let mut app = test_app();
        let e = app
            .world_mut()
            .spawn((
                Material {
                    texture_id: Some(4242),
                    ..Default::default()
                },
                TexturePath("shared.png".into()),
            ))
            .id();

        for _ in 0..10 {
            app.update();
        }

        assert_eq!(
            app.world().get::<Material>(e).unwrap().texture_id,
            Some(4242),
            "an id already set must not be overwritten"
        );
        assert_eq!(
            app.world().resource::<TextureCache>().uploaded_count(),
            0,
            "and nothing should have been requested for it"
        );
    }
}
