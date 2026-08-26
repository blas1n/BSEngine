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
        let path = wanted.0.as_str();
        // Requested here, the first time any entity asks for this path, so
        // "request exactly once" is a property of the map rather than of the
        // control flow below.
        let entry = cache
            .by_path
            .entry(path.to_string())
            .or_insert_with(|| CachedTexture {
                slot: bsengine_asset::AssetSlot::requesting(&asset_server, path),
                id: None,
            });

        match entry.slot.poll(&asset_server, &textures) {
            bsengine_asset::Polled::Arrived => {
                if let Some(tex) = textures.get(entry.slot.handle()) {
                    entry.id = Some(registry.load_from_rgba(tex.width, tex.height, &tex.data));
                }
                // The id is deliberately not written to `material` here. The
                // block below does it on the next frame, and doing it in both
                // places is a line whose removal changes nothing observable --
                // which is exactly what a mutation test found when it was there.
            }
            bsengine_asset::Polled::Failed(e) => {
                tracing::warn!("[texture] '{path}' failed to load: {e}");
            }
            bsengine_asset::Polled::Nothing => {}
        }

        if let Some(id) = entry.id {
            material.texture_id = Some(id);
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
        app.add_systems(bevy_app::Update, resolve_texture_paths);
        app
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
