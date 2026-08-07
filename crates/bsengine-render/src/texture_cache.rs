//! Turning a texture path into a `Material.texture_id`.
//!
//! `GpuTextureRegistry` hands out ids and never asks where a texture came from,
//! so something has to own the question "is this path already on the GPU?".
//! That is this cache, and answering it is what stops ten entities sharing one
//! image from uploading it ten times.
//!
//! The state machine is deliberately the same shape as the two already in
//! `plugin.rs` (the skybox and custom shaders). Not because three is a good
//! number -- it is not, and unifying them is worth its own item -- but because
//! whoever unifies them should find three that look alike.

use std::collections::HashMap;

use bevy_ecs::prelude::*;
use bsengine_asset::TextureAsset;
use bsengine_core::{Material, TexturePath};
use bsengine_rhi_wgpu::GpuTextureRegistry;

/// What has happened to one texture path.
enum TextureState {
    /// Requested once. The handle is polled, never re-requested.
    Loading(bevy_asset::Handle<TextureAsset>),
    /// Uploaded. The handle is retained because dropping it releases the asset
    /// and switches hot reload off silently -- the same reason the skybox keeps
    /// its own.
    Ready {
        id: u64,
        _handle: bevy_asset::Handle<TextureAsset>,
    },
    /// The load failed.
    ///
    /// Held as a state rather than forgotten, for the reason the custom-shader
    /// path spells out: re-requesting a failed path resets it to `Loading` and
    /// starts the load over, so a re-requesting poll loop can never see the
    /// failure at all. Keeping it also makes the warning fire once instead of
    /// every frame.
    GaveUp,
}

/// One state per texture path.
#[derive(Resource, Default)]
pub struct TextureCache {
    by_path: HashMap<String, TextureState>,
}

impl TextureCache {
    /// The GPU id for a path, once it has finished uploading.
    pub fn id_for(&self, path: &str) -> Option<u64> {
        match self.by_path.get(path) {
            Some(TextureState::Ready { id, .. }) => Some(*id),
            _ => None,
        }
    }

    /// Whether this path reached a terminal failure.
    ///
    /// Distinct from "has no id": a path still loading also has no id, and a
    /// test that cannot tell those apart cannot tell a give-up from an infinite
    /// retry, which is the failure mode this whole state exists to prevent.
    pub fn gave_up(&self, path: &str) -> bool {
        matches!(self.by_path.get(path), Some(TextureState::GaveUp))
    }

    /// How many distinct paths have reached the GPU. Uploading each one exactly
    /// once is what this cache is for, so tests count it.
    pub fn uploaded_count(&self) -> usize {
        self.by_path
            .values()
            .filter(|s| matches!(s, TextureState::Ready { .. }))
            .count()
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
        match cache.by_path.get(path) {
            Some(TextureState::Ready { id, .. }) => {
                material.texture_id = Some(*id);
            }
            Some(TextureState::GaveUp) => {}
            Some(TextureState::Loading(handle)) => {
                if let Some(tex) = textures.get(handle) {
                    let id = registry.load_from_rgba(tex.width, tex.height, &tex.data);
                    let handle = handle.clone();
                    cache.by_path.insert(
                        path.to_string(),
                        TextureState::Ready {
                            id,
                            _handle: handle,
                        },
                    );
                    // Deliberately not written here. The `Ready` arm above does
                    // it on the next frame, and doing it in both places is a
                    // line whose removal changes nothing observable -- which is
                    // exactly what a mutation test found when it was there.
                } else if let bevy_asset::LoadState::Failed(e) = asset_server.load_state(handle) {
                    tracing::warn!("[texture] '{path}' failed to load: {e}");
                    cache.by_path.insert(path.to_string(), TextureState::GaveUp);
                }
            }
            None => {
                let handle = bsengine_asset::load_async::<TextureAsset>(&asset_server, path);
                cache
                    .by_path
                    .insert(path.to_string(), TextureState::Loading(handle));
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
            16, 16,
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

        for _ in 0..80 {
            app.update();
        }

        assert!(
            app.world()
                .resource::<TextureCache>()
                .gave_up("does/not/exist.png"),
            "a texture that cannot load has to end up in GaveUp; staying in Loading \
             is indistinguishable from still trying, which is the bug"
        );
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
