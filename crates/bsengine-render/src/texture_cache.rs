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
use bsengine_core::{
    Camera, EditorPlayState, GlobalTransform, InspectorState, Material, ScreenSize, TexturePath,
    TextureStreamingSettings, Transform,
};
use bsengine_rhi_wgpu::{GpuMeshRegistry, GpuTextureRegistry};
use glam::Vec3;

use crate::components::MeshRenderer;

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

/// The largest on-screen extent, in pixels, of a sphere of `radius` at
/// `distance` from a camera whose vertical field of view has `tan_half_fov`
/// and whose viewport is `screen_height` pixels tall. Infinite when the
/// camera is inside the sphere.
///
/// The number a texture's wanted level is read from: drawn across this
/// many pixels, a level with this many texels has one per pixel and more
/// are not visible. It assumes the texture is laid once over the object,
/// which is what a model's UVs usually do and what Unity assumes for a
/// renderer without a texel-density override.
fn projected_pixels(radius: f32, distance: f32, tan_half_fov: f32, screen_height: f32) -> f32 {
    let clear = distance - radius;
    if clear <= 0.0 {
        return f32::INFINITY;
    }
    radius * screen_height / (clear * tan_half_fov)
}

/// Moves one streamed texture one mip level towards what the screen asks
/// for, every frame.
///
/// A texture whose sidecar says `streaming: true` uploads with only its
/// small levels resident (`GpuTextureRegistry::load_with`), so a scene full
/// of them draws on the first frame at a fraction of the memory and the
/// upload time. This is the other half. Each frame it measures, for every
/// entity drawing a streamed texture, how many pixels tall that entity's
/// bounding sphere is from the camera, records the largest per texture as
/// its want (`GpuTextureRegistry::set_wants`), and lets the registry take
/// one step (`step_streaming`): a level in for the texture furthest below
/// its want, or a level out for one holding more than it wants or when the
/// project's budget is exceeded. This is the shape Unity's mipmap streaming
/// and Unreal's texture pool share: a wanted mip per texture from its
/// screen size, a memory budget that overrides it, one change at a time.
///
/// One level per frame rather than every pending level because each change
/// is a texture allocation plus a re-upload on the queue, and the scenes
/// this is for have many textures arriving together; spreading them is
/// what keeps the frame that follows a scene load from stalling.
///
/// Without a camera or a screen size -- a headless test, the frame before
/// the window reports its size -- nothing can be measured, and every
/// streamed texture is wanted whole, as it was before wants existed. A
/// streamed texture no mesh draws (a UI image, a particle sheet) is wanted
/// whole for the same reason. While the editor is stopped its orbit camera
/// is the one looking, as it is for culling.
#[allow(clippy::too_many_arguments)]
pub fn stream_textures(
    registry: Option<ResMut<GpuTextureRegistry>>,
    meshes: Option<Res<GpuMeshRegistry>>,
    settings: Option<Res<TextureStreamingSettings>>,
    screen: Option<Res<ScreenSize>>,
    inspector: Option<Res<InspectorState>>,
    cameras: Query<(&Camera, &Transform, Option<&GlobalTransform>)>,
    users: Query<(
        &MeshRenderer,
        &Transform,
        Option<&GlobalTransform>,
        &Material,
    )>,
) {
    let Some(mut registry) = registry else {
        return;
    };
    let settings = settings.map(|s| *s).unwrap_or_default();
    let budget = (settings.budget_bytes > 0).then_some(settings.budget_bytes);

    let mut wants: HashMap<u64, f32> = HashMap::new();
    let eye = camera_eye(inspector.as_deref(), &cameras);
    if let (Some((cam_pos, tan_half_fov)), Some(screen)) = (eye, screen) {
        let screen_height = screen.height as f32;
        for (mr, t, gt, material) in users.iter() {
            let Some(id) = material.texture_id else {
                continue;
            };
            if !registry.is_streaming(id) {
                continue;
            }
            let model = gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix());
            let max_scale = model
                .x_axis
                .truncate()
                .length()
                .max(model.y_axis.truncate().length())
                .max(model.z_axis.truncate().length());
            // The mesh's own sphere when it is registered; otherwise the unit
            // sphere, which is what the primitives and any mesh not yet on
            // the GPU are taken to be. Erring large wants a larger level,
            // never a blurrier one.
            let (local_center, local_radius) = meshes
                .as_deref()
                .and_then(|m| m.get_bounds(mr.mesh_id))
                .unwrap_or((Vec3::ZERO, 1.0));
            let center = (model * local_center.extend(1.0)).truncate();
            let radius = local_radius * max_scale;
            let pixels = projected_pixels(
                radius,
                (center - cam_pos).length(),
                tan_half_fov,
                screen_height,
            );
            let want = wants.entry(id).or_insert(0.0);
            *want = want.max(pixels);
        }
    }
    registry.set_wants(&wants, settings.mip_bias);
    registry.step_streaming(budget);
    bsengine_rhi_wgpu::profiler::record_streaming(
        registry.streamed_resident_bytes(),
        settings.budget_bytes,
        registry.textures_below_wanted(),
    );
}

/// Where the frame is looked at from and how wide: `(position, tan(fov/2))`.
/// The editor's orbit camera while the editor is stopped, otherwise the
/// first `Camera` entity; `None` without either.
fn camera_eye(
    inspector: Option<&InspectorState>,
    cameras: &Query<(&Camera, &Transform, Option<&GlobalTransform>)>,
) -> Option<(Vec3, f32)> {
    if let Some(insp) = inspector {
        if insp.editor_mode && insp.play_state == EditorPlayState::Stopped {
            // A perspective projection's [1][1] is 1 / tan(fov_y / 2).
            let m11 = insp.editor_proj[1][1];
            if m11.is_finite() && m11 > 0.0 {
                return Some((Vec3::from(insp.editor_cam_pos), 1.0 / m11));
            }
        }
    }
    let (cam, t, gt) = cameras.iter().next()?;
    let pos = gt
        .map(|g| g.to_matrix().w_axis.truncate())
        .unwrap_or(t.position.0);
    Some((pos, (cam.fov_y_degrees.to_radians() * 0.5).tan()))
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
            (
                resolve_texture_paths,
                reupload_modified_textures,
                stream_textures,
            ),
        );
        app
    }

    /// A streamed file texture reaches the GPU small and then grows a level
    /// a frame until it is whole -- counted in frames, so a system that
    /// raised everything at once, or nothing, both fail. The 256x256
    /// checker starts at 64x64 (two levels waiting), so it takes exactly two
    /// updates after the upload to become whole.
    #[test]
    fn a_streamed_texture_grows_one_level_per_frame_until_whole() {
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
        assert!(
            !app.world()
                .resource::<GpuTextureRegistry>()
                .is_streaming(gpu_id),
            "premise: without a sidecar the texture is not streamed"
        );

        // A sidecar edit turning streaming on, as the watcher delivers it:
        // the same handle, modified, and re-uploaded by the reload system.
        // The pixels are replaced with a 256x256 image at the same time so
        // the number of levels waiting (two) does not depend on the size of
        // the file on disk.
        {
            let mut textures = app
                .world_mut()
                .resource_mut::<bevy_asset::Assets<TextureAsset>>();
            let tex = textures.get_mut(&handle).expect("the asset is loaded");
            tex.width = 256;
            tex.height = 256;
            tex.data = vec![90u8; 256 * 256 * 4];
            tex.settings = TextureImportSettings {
                streaming: true,
                ..Default::default()
            };
        }
        // Sampled after every update. The event is read the frame after it
        // is written, and within that frame the reupload and the streaming
        // system are unordered, so the first streamed sample is 2 or 1
        // depending on which ran first; from there it must step down by
        // exactly one per frame. A system that raised every pending level
        // at once would show 0 as its first sample; one that never raised
        // would stay at 2.
        let mut residency_per_frame = Vec::new();
        for _ in 0..6 {
            app.update();
            let registry = app.world().resource::<GpuTextureRegistry>();
            residency_per_frame.push(registry.residency(gpu_id).map(|r| r.0));
        }
        let streamed: Vec<u32> = residency_per_frame.iter().flatten().copied().collect();
        let first = *streamed
            .first()
            .expect("the modified settings must reach the GPU");
        assert!(
            first >= 1,
            "the first streamed frame must still have a level waiting; got {residency_per_frame:?}"
        );
        let expected: Vec<u32> = (0..=first).rev().collect();
        assert_eq!(
            &streamed[..expected.len()],
            &expected[..],
            "one level per frame down to 0; got {residency_per_frame:?}"
        );
        assert!(
            streamed[expected.len()..].iter().all(|r| *r == 0),
            "and it stays whole; got {residency_per_frame:?}"
        );
        assert_eq!(
            app.world()
                .resource::<GpuTextureRegistry>()
                .get_gpu_footprint(gpu_id)
                .map(|f| (f.0, f.1)),
            Some((256, 256)),
            "and it ends whole"
        );
    }

    /// Makes the uploaded checker a 256x256 streamed texture through the
    /// reload path, as `a_streamed_texture_grows_one_level_per_frame_until_whole`
    /// does, and returns its GPU id. Two levels wait above the 64 it starts
    /// with.
    fn restream_as_256(app: &mut bevy_app::App) -> u64 {
        use bsengine_core::TextureImportSettings;
        let gpu_id = app
            .world()
            .resource::<TextureCache>()
            .id_for(REAL_TEXTURE)
            .expect("premise: the texture uploaded");
        let handle = app.world().resource::<TextureCache>().by_path[REAL_TEXTURE]
            .slot
            .handle()
            .clone();
        let mut textures = app
            .world_mut()
            .resource_mut::<bevy_asset::Assets<TextureAsset>>();
        let tex = textures.get_mut(&handle).expect("the asset is loaded");
        tex.width = 256;
        tex.height = 256;
        tex.data = vec![90u8; 256 * 256 * 4];
        tex.settings = TextureImportSettings {
            streaming: true,
            ..Default::default()
        };
        gpu_id
    }

    /// The camera decides how far a streamed texture goes, and the budget
    /// overrides the camera. A unit sphere 33 units from a 60-degree camera
    /// on a 720-tall screen is 39 pixels across, so its texture wants the
    /// 64 level it already holds and **stays there** -- the assertion a
    /// streamer that raised everything to full (stage 1) fails. Brought to
    /// 3 units it is 620 pixels and the texture goes whole, a level a
    /// frame; sent to 200 units it is 6 pixels and the texture gives levels
    /// back down to the slack below the 8 level. A budget of one byte then
    /// drives it to its 1x1 level however close the camera is, and a mip
    /// bias of 2 with no budget lets it back up to two levels short.
    #[test]
    fn residency_follows_the_cameras_distance_and_the_budget() {
        use bsengine_core::{Camera, ScreenSize, TextureStreamingSettings};

        let mut app = test_app();
        app.insert_resource(ScreenSize {
            width: 1280,
            height: 720,
        });
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform {
                    position: Vec3::new(0.0, 0.0, 33.0).into(),
                    ..Default::default()
                },
            ))
            .id();
        // A mesh no registry knows: the unit sphere stands in for it.
        app.world_mut().spawn((
            MeshRenderer { mesh_id: 4242 },
            Transform::default(),
            Material::default(),
            TexturePath(REAL_TEXTURE.into()),
        ));
        for _ in 0..60 {
            app.update();
        }
        let gpu_id = restream_as_256(&mut app);
        let residency = |app: &bevy_app::App| {
            app.world()
                .resource::<GpuTextureRegistry>()
                .residency(gpu_id)
                .map(|r| r.0)
        };
        let wanted =
            |app: &bevy_app::App| app.world().resource::<GpuTextureRegistry>().wanted(gpu_id);
        let move_camera = |app: &mut bevy_app::App, z: f32| {
            app.world_mut()
                .get_mut::<Transform>(camera)
                .unwrap()
                .position
                .0 = Vec3::new(0.0, 0.0, z);
        };

        let far: Vec<Option<u32>> = (0..6)
            .map(|_| {
                app.update();
                residency(&app)
            })
            .collect();
        assert_eq!(wanted(&app), Some(2), "39 px wants the 64 level");
        // The first sample may predate the re-upload (the modified event is
        // read the frame after it is written), as in the growth test above;
        // every streamed sample must be the 64 level.
        let streamed: Vec<u32> = far.iter().flatten().copied().collect();
        assert!(
            streamed.len() >= 4 && streamed.iter().all(|r| *r == 2),
            "at 33 units the texture must hold the 64 level and no more: {far:?}"
        );

        move_camera(&mut app, 3.0);
        let near: Vec<Option<u32>> = (0..4)
            .map(|_| {
                app.update();
                residency(&app)
            })
            .collect();
        assert_eq!(wanted(&app), Some(0), "620 px wants everything");
        assert_eq!(
            near,
            vec![Some(1), Some(0), Some(0), Some(0)],
            "one level a frame up to whole, then rest"
        );

        move_camera(&mut app, 200.0);
        let away: Vec<Option<u32>> = (0..7)
            .map(|_| {
                app.update();
                residency(&app)
            })
            .collect();
        assert_eq!(wanted(&app), Some(5), "6 px wants the 8 level");
        assert_eq!(
            away,
            vec![
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(4),
                Some(4),
                Some(4)
            ],
            "one level a frame down to the slack below the want"
        );

        // A budget of one byte: over it however little is resident, so the
        // levels go until only the 1x1 is left, and a close camera cannot
        // bring any back.
        app.insert_resource(TextureStreamingSettings {
            budget_bytes: 1,
            mip_bias: 0,
        });
        move_camera(&mut app, 3.0);
        for _ in 0..8 {
            app.update();
        }
        assert_eq!(wanted(&app), Some(0));
        assert_eq!(
            residency(&app),
            Some(8),
            "the budget holds the texture at its smallest level"
        );

        // No budget, a bias of two levels: the close camera asks for level
        // 0 and gets level 2 -- 64 -- and no more.
        app.insert_resource(TextureStreamingSettings {
            budget_bytes: 0,
            mip_bias: 2,
        });
        for _ in 0..10 {
            app.update();
        }
        assert_eq!(wanted(&app), Some(2));
        assert_eq!(
            residency(&app),
            Some(2),
            "raised back up to the biased want"
        );
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

    /// The progressive loader is wired into `RenderPlugin`, which is what
    /// every runtime and the editor add -- a system only the test above
    /// registers by hand would be a system nothing ships. Driven through the
    /// registry directly rather than a file, since the wiring is the
    /// question here and the file path is proven above.
    #[test]
    fn render_plugin_brings_a_streamed_textures_levels_in_frame_by_frame() {
        use bsengine_core::TextureImportSettings;

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(crate::RenderPlugin);
        with_gpu(&mut app);
        let id = app
            .world_mut()
            .resource_mut::<GpuTextureRegistry>()
            .load_with(
                256,
                256,
                &vec![0u8; 256 * 256 * 4],
                TextureImportSettings {
                    streaming: true,
                    ..Default::default()
                },
            );
        let residency = |app: &bevy_app::App| {
            app.world()
                .resource::<GpuTextureRegistry>()
                .residency(id)
                .map(|r| r.0)
        };
        assert_eq!(residency(&app), Some(2), "premise: two levels waiting");
        app.update();
        assert_eq!(
            residency(&app),
            Some(1),
            "the plugin's Update brings one level in per frame"
        );
        app.update();
        assert_eq!(residency(&app), Some(0));
        app.update();
        assert_eq!(residency(&app), Some(0), "and rests once whole");
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
