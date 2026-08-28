use bevy_app::{App, Plugin, PostUpdate, Update};
use bevy_ecs::prelude::{EventReader, IntoSystemConfigs, ParamSet, Query, ResMut};
use bsengine_core::{
    AmbientOcclusion, Bloom, Camera, CustomShader, DirectionalLight, EditorPanelRegistry,
    EditorPlayState, GlobalTransform, HudTexts, InspectorState, Material, PointLight, SkyboxPath,
    SpotLight, Time, ToneMap, Transform, UiState, Visible,
};
use bsengine_ecs::Res;
use bsengine_input::{Input, KeyCode, KeyInput, MouseButton, MouseState};
use bsengine_rhi_wgpu::{
    GpuMeshRegistry, GpuTextureRegistry, LightData, MaterialParams, PointLightEntry,
    SpotLightEntry, WgpuSurfaceResource,
};
use bsengine_window::WindowResized;
use glam::{Mat4, Vec3, Vec4};

use crate::components::{LodLevels, MeshRenderer, Occluder, TerrainSplat};
use crate::lod::select_lod_level;

/// Returns false if the sphere is completely outside the view frustum.
/// Uses Gribb-Hartmann plane extraction from the view-projection matrix
/// (assumes perspective_rh / −1..1 clip depth convention).
fn sphere_visible_in_frustum(view_proj: Mat4, world_center: Vec3, world_radius: f32) -> bool {
    let r0 = view_proj.row(0);
    let r1 = view_proj.row(1);
    let r2 = view_proj.row(2);
    let r3 = view_proj.row(3);
    let planes = [
        r3 + r0, // left
        r3 - r0, // right
        r3 + r1, // bottom
        r3 - r1, // top
        r3 + r2, // near  (perspective_rh: near maps to −1)
        r3 - r2, // far
    ];
    let p = world_center.extend(1.0);
    for plane in &planes {
        if plane.dot(p) < -world_radius * plane.truncate().length() {
            return false;
        }
    }
    true
}

/// Computes an orthographic view-projection from the light's direction for shadow mapping.
/// Uses rh_zo (0..1 depth) to match wgpu's depth buffer convention.
fn compute_light_view_proj(light_dir: Vec3) -> Mat4 {
    let dir = light_dir.normalize();
    let up = if dir.y.abs() < 0.999 {
        Vec3::Y
    } else {
        Vec3::Z
    };
    let eye = -dir * 50.0;
    let view = Mat4::look_at_rh(eye, Vec3::ZERO, up);
    let proj = Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 0.1, 200.0);
    proj * view
}

fn spot_light_entry(sl: &SpotLight, gt: Option<&GlobalTransform>, t: &Transform) -> SpotLightEntry {
    let pos = gt
        .map(|g| g.to_matrix().w_axis.truncate())
        .unwrap_or(t.position.0);
    let dir = gt
        .map(|g| -glam::Mat3::from_mat4(g.to_matrix()).z_axis)
        .unwrap_or_else(|| t.rotation.0 * Vec3::NEG_Z);
    SpotLightEntry {
        position: pos,
        direction: dir,
        color: *sl.color,
        intensity: sl.intensity,
        range: sl.range,
        inner_angle: sl.inner_angle_degrees.to_radians(),
        outer_angle: sl.outer_angle_degrees.to_radians(),
    }
}

/// What `compile_pending_shaders` knows about one shader path.
///
/// Internal to this system, like `bsengine_gltf`'s `PendingGltf`:
/// `CustomShader.path` stays a plain `String`, so scene RON, the scripting
/// API and the MCP tools are unaffected by how a load is tracked.
#[derive(Debug)]
struct PendingShader {
    /// The source load. See [`bsengine_asset::AssetSlot`].
    ///
    /// `Ready` here says the WGSL text arrived and nothing more. Whether it
    /// compiles is [`Self::compile`]'s business, because compiling needs a GPU
    /// the load does not -- and because `rebuild_modified_shaders` keys off the
    /// *load* having arrived, a slot that waited for the compile would switch
    /// hot reload off for as long as no surface existed.
    slot: bsengine_asset::AssetSlot<crate::shader_asset::ShaderSource>,
    /// What the compiler made of the source, if it has seen it.
    compile: CompileStatus,
}

/// Whether a loaded shader source has been compiled, and what came of it.
#[derive(Debug, PartialEq)]
enum CompileStatus {
    /// Loaded, but no GPU has seen it yet. The compile is deferred, not failed.
    NotYet,
    /// Compiled; the pipeline is in the surface.
    Ok,
    /// The source loaded but does not compile.
    ///
    /// Sticky, so the compile is *not* retried every frame: the file is broken,
    /// recompiling it next frame produces the same failure and the same
    /// warning, which is the per-frame noise this loop is shaped to avoid. Not
    /// the same as the load having failed -- the slot still holds the handle,
    /// so editing the file emits `AssetEvent::Modified` and
    /// `rebuild_modified_shaders` retries from here. Retry is driven by the
    /// content changing, not by the frame clock.
    ///
    /// Says nothing about what is on screen: `compile_and_store_shader` leaves
    /// `custom_pipelines` untouched when it fails, so a shader that compiled
    /// once and was then edited into a broken state keeps drawing with its last
    /// working pipeline.
    Failed,
}

/// `Ok` on success, `Failed` otherwise. The handle is untouched either way --
/// dropping it on failure would stop `AssetEvent::Modified` from ever firing
/// again for the path, which is exactly the event the fix has to arrive on.
fn compile_status(result: Result<(), String>) -> CompileStatus {
    match result {
        Ok(()) => CompileStatus::Ok,
        Err(_) => CompileStatus::Failed,
    }
}

/// Shader loads in flight, keyed by path.
///
/// Keyed by path rather than entity because several entities may name the
/// same shader, and because `CustomShader.path` stays a plain `String` -- the
/// boundary item 23 established, so scene RON, the scripting API and the MCP
/// tools are unaffected.
#[derive(bevy_ecs::prelude::Resource, Default)]
struct PendingShaders(std::collections::HashMap<String, PendingShader>);

/// Lazy-compiles any `CustomShader` not yet cached in the surface, reading
/// its WGSL source through `bsengine_asset::load` (`LoadMode::Async`) and
/// handing the text to `compile_and_store_shader`. Split out of
/// `render_frame` (rather than folded in as two more top-level params)
/// because that function is already at Bevy 0.14's 16-top-level-param
/// `SystemParamFunction` ceiling — see the comment on `render_frame`'s
/// `render_queries` param. Registered in the same `PostUpdate` `.chain()`
/// as `render_frame` (see `RenderPlugin::build`), immediately before it, so
/// compiled shaders are available the same frame `render_frame` needs them
/// — an explicit, compiler-checked ordering constraint rather than relying
/// on this being a separate schedule that merely happens to run earlier.
///
/// Its `Query<&CustomShader>` is intentionally broader than the old inline
/// loop it replaces: it fires for *any* entity with a `CustomShader`
/// component, not just ones that also match `render_frame`'s mandatory
/// `&MeshRenderer, &Transform` query. Harmless (a shader that's never drawn
/// just sits compiled-and-unused in the surface's cache) and arguably an
/// improvement (a shader is ready the instant `MeshRenderer`/`Transform`
/// are added later, rather than one frame behind).
///
/// The missing `WgpuSurfaceResource` case is *not* an early return: only the
/// final `compile_and_store_shader` step needs the GPU, so requesting,
/// polling and failure detection run regardless. A real surface needs a real
/// winit window (see `compile_pending_shaders_runs_before_render_frame`), so
/// an early return would make the give-up path unreachable in every test
/// this workspace can write.
fn compile_pending_shaders(
    mut surface: Option<ResMut<WgpuSurfaceResource>>,
    custom_shaders: Query<&CustomShader>,
    mut shader_assets: bevy_ecs::prelude::ResMut<
        bevy_asset::Assets<crate::shader_asset::ShaderSource>,
    >,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
    mut pending: ResMut<PendingShaders>,
) {
    for cs in custom_shaders.iter() {
        if surface
            .as_ref()
            .is_some_and(|s| s.0.has_custom_shader(&cs.path))
        {
            continue;
        }

        // Requested exactly once, the first frame this path is seen, and polled
        // from then on. `bsengine_asset::load` rather than `load_async` because
        // WGSL has a `LoadMode::Sync` loader (`load_shader_source`) this
        // dispatcher can reach; the slot wraps whichever handle comes back.
        if !pending.0.contains_key(&cs.path) {
            let slot = match bsengine_asset::load(
                bsengine_asset::LoadMode::Async,
                &asset_server,
                &mut shader_assets,
                &cs.path,
                crate::shader_asset::load_shader_source,
            ) {
                Ok(handle) => bsengine_asset::AssetSlot::from_handle(handle),
                Err(e) => {
                    // Unreachable: `LoadMode::Async` is infallible. Present
                    // only because the shared `load()` signature returns
                    // `Result` for `Sync` callers -- and with no handle back
                    // there is no slot to record, so nothing is inserted and
                    // the path is simply tried again next frame.
                    tracing::warn!("[custom_shader] cannot request '{}': {e}", cs.path);
                    continue;
                }
            };
            pending.0.insert(
                cs.path.clone(),
                PendingShader {
                    slot,
                    compile: CompileStatus::NotYet,
                },
            );
        }
        let Some(entry) = pending.0.get_mut(&cs.path) else {
            continue;
        };

        if let bsengine_asset::Polled::Failed(e) = entry.slot.poll(&asset_server, &shader_assets) {
            tracing::warn!("[custom_shader] cannot read '{}': {e}", cs.path);
            continue;
        }

        // The source is here; only the compile needs the GPU. With no surface
        // the compile is merely deferred, not failed: the status stays `NotYet`
        // and this runs again once a surface appears. Reaching this at all
        // means the early skip above found no compiled pipeline for the path, so
        // a successful compile makes that skip fire from the next frame on and
        // this runs exactly once. A `Failed` status is left alone, so a broken
        // file is not recompiled -- and re-warned about -- every frame either.
        if entry.compile != CompileStatus::NotYet {
            continue;
        }
        // Cloned so the verdict can be written back below; a `Handle` is
        // refcounted, so this is a bump rather than a copy of the source.
        let handle = entry.slot.handle().clone();
        let (Some(src), Some(surface)) = (shader_assets.get(&handle), surface.as_mut()) else {
            continue;
        };
        entry.compile = compile_status(surface.0.compile_and_store_shader(&cs.path, &src.0));
    }
}

/// Whether `state` should be recompiled now that `id`'s source has been
/// replaced.
///
/// Deliberately indifferent to [`CompileStatus`]: a shader that failed to
/// compile has to be rebuilt just as readily as one that succeeded, because an
/// edit is the only signal its content changed and `compile_pending_shaders`
/// never retries a `Failed` path on its own. Skipping them here would make a
/// single typo permanent for the rest of the run.
///
/// What it does require is that the *source* arrived — `Loading` and `GaveUp`
/// slots hold nothing to recompile.
fn wants_rebuild(
    state: &PendingShader,
    id: bevy_asset::AssetId<crate::shader_asset::ShaderSource>,
) -> bool {
    state.slot.is_ready() && state.slot.handle().id() == id
}

/// Recompiles a custom shader whose source was replaced.
///
/// `compile_and_store_shader` inserts into `custom_pipelines` keyed by path, so
/// recompiling overwrites the old pipeline; no explicit invalidation is needed.
/// That also makes one recompile per path enough no matter how many entities
/// name it -- [`PendingShaders`] is keyed by path, and `render_frame` looks the
/// pipeline up by path too.
///
/// Separate from `compile_pending_shaders` rather than folded into it because
/// that function skips any path the surface has already compiled -- which is
/// every reloadable one. A reload is the one case where recompiling an
/// already-compiled path is the whole point.
///
/// Without a `WgpuSurfaceResource` this does nothing and leaves the state
/// alone: the source is already in `Assets` and the handle is still retained,
/// so the next reload is reached just the same.
///
/// `CompileFailed` paths are rebuilt as readily as `Ready` ones -- this is the
/// only place a broken shader can recover, because it is the only signal that
/// the file's *content* changed. `compile_pending_shaders` deliberately never
/// retries them, so skipping them here would make a single typo permanent for
/// the rest of the run.
fn rebuild_modified_shaders(
    mut events: bevy_ecs::prelude::EventReader<
        bevy_asset::AssetEvent<crate::shader_asset::ShaderSource>,
    >,
    mut surface: Option<ResMut<WgpuSurfaceResource>>,
    shader_assets: Res<bevy_asset::Assets<crate::shader_asset::ShaderSource>>,
    mut pending: ResMut<PendingShaders>,
) {
    for event in events.read() {
        let bevy_asset::AssetEvent::Modified { id } = event else {
            continue;
        };
        // Matched paths are collected before compiling: the compile's verdict
        // is written back into `pending`, which cannot happen while iterating
        // it. One event names one asset, so this is a one-element vector in
        // every realistic case, and it is allocated per *edit*, not per frame.
        let rebuilt: Vec<(
            String,
            bevy_asset::Handle<crate::shader_asset::ShaderSource>,
        )> = pending
            .0
            .iter()
            .filter(|(_, state)| wants_rebuild(state, *id))
            .map(|(path, state)| (path.clone(), state.slot.handle().clone()))
            .collect();
        for (path, handle) in rebuilt {
            let Some(src) = shader_assets.get(&handle) else {
                continue;
            };
            let Some(surface) = surface.as_mut() else {
                continue;
            };
            let verdict = compile_status(surface.0.compile_and_store_shader(&path, &src.0));
            if let Some(entry) = pending.0.get_mut(&path) {
                entry.compile = verdict;
            }
        }
    }
}

/// The skybox image this engine asked for, and how far it has got.
#[derive(Debug)]
struct PendingSkyboxLoad {
    /// The path this request was made for, so a `SkyboxPath` change mid-load
    /// abandons the old request instead of uploading a texture nobody asked
    /// for any more.
    path: String,
    /// The load itself. See [`bsengine_asset::AssetSlot`].
    slot: bsengine_asset::AssetSlot<bsengine_asset::TextureAsset>,
}

/// The skybox load in flight, if any.
///
/// One slot rather than a map: there is only ever one skybox.
///
/// Internal to this system, like `bsengine_gltf`'s `PendingGltf`: `SkyboxPath`
/// stays a plain `String`, so scene RON, the scripting API and the MCP tools
/// are unaffected by how a load is tracked.
#[derive(bevy_ecs::prelude::Resource, Default)]
struct PendingSkybox(Option<PendingSkyboxLoad>);

/// Keeps the surface's skybox in sync with `SkyboxPath`, reading the image
/// through `Assets<TextureAsset>` and uploading it with
/// `set_skybox_from_rgba`.
///
/// Item 23 split the surface's old blocking `set_skybox` (path in, `image::open`,
/// upload) into decode and upload halves; this is the consumer that split was
/// for, and the blocking half has since been deleted outright — nothing in the
/// engine may stall a frame on a file read any more. It gets its own system
/// rather than staying in
/// `render_frame` because that function is already at Bevy 0.14's
/// 16-top-level-param ceiling (see the comment on its `render_queries` param),
/// and because waiting across frames is not a render pass's job.
///
/// The missing `WgpuSurfaceResource` case is *not* an early return: only the
/// upload needs the GPU, so requesting, polling and failure detection run
/// regardless. A real surface needs a real winit window (see
/// `compile_pending_shaders_runs_before_render_frame`), so an early return
/// would make the give-up path unreachable in every test this workspace can
/// write.
fn upload_pending_skybox(
    mut surface: Option<ResMut<WgpuSurfaceResource>>,
    skybox_path: Option<Res<SkyboxPath>>,
    texture_assets: bevy_ecs::prelude::Res<bevy_asset::Assets<bsengine_asset::TextureAsset>>,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
    mut pending: ResMut<PendingSkybox>,
) {
    let Some(skybox_path) = skybox_path else {
        return;
    };
    let wanted = skybox_path.0.as_deref();

    // Already showing exactly what's asked for. A `Ready` slot naming that same
    // path is the uploaded skybox's own retained handle -- clearing it frees
    // the image and turns the next `AssetServer::reload` into a silent no-op,
    // so it stays. Anything else in the slot is a request for a path nobody
    // wants any more, so let it go.
    if surface
        .as_ref()
        .is_some_and(|s| s.0.loaded_skybox_path() == wanted)
    {
        let holds_the_wanted_skybox = match (&pending.0, wanted) {
            (Some(load), Some(wanted)) => load.slot.is_ready() && load.path == wanted,
            _ => false,
        };
        if pending.0.is_some() && !holds_the_wanted_skybox {
            pending.0 = None;
        }
        return;
    }

    let Some(wanted) = wanted else {
        // The skybox was turned off; drop the in-flight request with it.
        if let Some(surface) = surface.as_mut() {
            surface.0.clear_skybox();
        }
        if pending.0.is_some() {
            pending.0 = None;
        }
        return;
    };

    // `SkyboxPath` changed mid-load: abandon the old request rather than
    // upload a texture nobody asked for any more.
    if pending.0.as_ref().is_some_and(|load| load.path != wanted) {
        pending.0 = None;
    }

    // Requested exactly once, on the first frame this path is wanted, and
    // polled from then on. `AssetSlot::requesting` goes through `load_async`
    // rather than `bsengine_asset::load` because that dispatcher takes a
    // `sync_loader` closure for its `LoadMode::Sync` arm and there is no
    // synchronous texture loader in this codebase: item 23 only ever wrote
    // `TextureAssetLoader`, for the async path. This used to call
    // `AssetServer::load` directly for that reason, which is precisely how the
    // skybox stayed invisible to `AssetStatuses` until it failed -- `load_async`
    // is the recording half of `load` with the unreachable `Sync` arm left out.
    let load = pending.0.get_or_insert_with(|| PendingSkyboxLoad {
        path: wanted.to_string(),
        slot: bsengine_asset::AssetSlot::requesting(&asset_server, wanted),
    });

    if let bsengine_asset::Polled::Failed(e) = load.slot.poll(&asset_server, &texture_assets) {
        tracing::warn!("skybox: cannot read '{wanted}': {e}");
        return;
    }

    // The pixels are here; only the upload needs the GPU. Reaching this at all
    // means the dedupe check above found the surface *not* showing this path --
    // i.e. the image arrived before `WgpuSurfaceResource` did -- so upload now
    // if a surface has since appeared. That makes the dedupe check match from
    // the next frame on, so this runs exactly once.
    // No "already uploaded" flag guards this: `set_loaded_skybox_path` below
    // makes the dedupe check at the top of this function match from the next
    // frame on, which is what stops a re-upload -- and it is also what lets a
    // surface that was rebuilt get its skybox back. A flag here would pass the
    // tests and quietly break that second case. (Measured: adding one changed
    // nothing any test could see.)
    if !load.slot.is_ready() {
        return;
    }
    let Some(surface) = surface.as_mut() else {
        return;
    };
    let Some(tex) = texture_assets.get(load.slot.handle()) else {
        return;
    };
    surface
        .0
        .set_skybox_from_rgba(tex.width, tex.height, &tex.data);
    // `set_skybox_from_rgba` doesn't record the path (that bookkeeping lived in
    // the now-deleted blocking `set_skybox`), so do it here — the dedupe check
    // above is what stops this re-uploading every frame.
    surface.0.set_loaded_skybox_path(wanted);
}

/// Re-uploads the skybox when its image is replaced.
///
/// `set_skybox_from_rgba` rebuilds the texture, sampler, bind groups and
/// pipeline around the new pixels and replaces `WgpuSurface::skybox` wholesale,
/// so no explicit invalidation is needed.
///
/// Separate from `upload_pending_skybox` rather than folded into it because
/// that function returns early the moment the surface already shows the wanted
/// path -- which is every reloadable skybox. A reload is the one case where
/// re-uploading the path already on screen is the whole point.
///
/// Without a `WgpuSurfaceResource` this does nothing and leaves the `Ready`
/// state alone: the image is already in `Assets` and the handle is still
/// retained, so the next reload is reached just the same.
fn rebuild_modified_skybox(
    mut events: bevy_ecs::prelude::EventReader<
        bevy_asset::AssetEvent<bsengine_asset::TextureAsset>,
    >,
    mut surface: Option<ResMut<WgpuSurfaceResource>>,
    texture_assets: Res<bevy_asset::Assets<bsengine_asset::TextureAsset>>,
    pending: Res<PendingSkybox>,
) {
    for event in events.read() {
        let bevy_asset::AssetEvent::Modified { id } = event else {
            continue;
        };
        let Some(load) = pending.0.as_ref().filter(|load| load.slot.is_ready()) else {
            continue;
        };
        let (path, handle) = (&load.path, load.slot.handle());
        if handle.id() != *id {
            continue;
        }
        let Some(tex) = texture_assets.get(handle) else {
            continue;
        };
        let Some(surface) = surface.as_mut() else {
            continue;
        };
        surface
            .0
            .set_skybox_from_rgba(tex.width, tex.height, &tex.data);
        surface.0.set_loaded_skybox_path(path);
    }
}

#[allow(clippy::too_many_arguments)] // Bevy system params; splitting into a struct is a larger refactor
fn render_frame(
    surface: Option<ResMut<WgpuSurfaceResource>>,
    time: Option<Res<Time>>,
    registry: Option<Res<GpuMeshRegistry>>,
    tex_registry: Option<Res<GpuTextureRegistry>>,
    hud_texts: Option<Res<HudTexts>>,
    mut ui_state: Option<ResMut<UiState>>,
    mut inspector: Option<ResMut<InspectorState>>,
    mouse_state: Option<Res<MouseState>>,
    mouse_buttons: Option<Res<Input<MouseButton>>>,
    mut key_events: EventReader<KeyInput>,
    keys: Option<Res<Input<KeyCode>>>,
    // Bundled into one ParamSet: Bevy 0.14's SystemParamFunction impls only
    // go up to 16 top-level params, and adding editor_panels as a 17th
    // plain parameter broke `IntoSystem` resolution for this function
    // (surfaced as a `.chain()` trait-bound error in RenderPlugin::build).
    // Folding these five Querys into one param keeps the total at 13.
    mut render_queries: ParamSet<(
        Query<(
            &Camera,
            &Transform,
            Option<&Bloom>,
            Option<&ToneMap>,
            Option<&AmbientOcclusion>,
        )>,
        Query<(
            &MeshRenderer,
            &Transform,
            Option<&GlobalTransform>,
            Option<&Material>,
            Option<&Visible>,
            Option<&CustomShader>,
            Option<&mut LodLevels>,
        )>,
        Query<(&DirectionalLight, Option<&GlobalTransform>, &Transform)>,
        Query<(&PointLight, Option<&GlobalTransform>, &Transform)>,
        Query<(&SpotLight, Option<&GlobalTransform>, &Transform)>,
        Query<(
            &bsengine_core::ParticleEmitter,
            Option<&bsengine_core::TexturePath>,
        )>,
        Query<(
            &MeshRenderer,
            &Transform,
            Option<&GlobalTransform>,
            &TerrainSplat,
        )>,
    )>,
    editor_panels: Option<Res<EditorPanelRegistry>>,
    type_registry: Option<Res<bevy_ecs::reflect::AppTypeRegistry>>,
    // Emitters name a texture by path, like materials do; this is where that
    // path becomes the id the GPU knows. Absent until item 38's cache exists,
    // in which case particles draw against the default white texture.
    texture_cache: Option<Res<crate::texture_cache::TextureCache>>,
) {
    let (Some(mut surface), Some(registry)) = (surface, registry) else {
        return;
    };
    let empty = std::collections::HashMap::new();
    let hud_map = hud_texts.as_deref().map(|h| &h.0).unwrap_or(&empty);
    let empty_ui = UiState::default();
    let ui = ui_state.as_deref().unwrap_or(&empty_ui);
    let (cursor_x, cursor_y) = mouse_state
        .as_deref()
        .map(|ms| (ms.position.0 as f32, ms.position.1 as f32))
        .unwrap_or((0.0, 0.0));
    let left_just_pressed = mouse_buttons
        .as_deref()
        .map(|b| b.just_pressed(&MouseButton::Left))
        .unwrap_or(false);
    let left_just_released = mouse_buttons
        .as_deref()
        .map(|b| b.just_released(&MouseButton::Left))
        .unwrap_or(false);
    let key_events_this_frame: Vec<KeyInput> = key_events.read().cloned().collect();
    let ctrl_held = keys
        .as_deref()
        .map(|k| k.is_pressed(&KeyCode::ControlLeft) || k.is_pressed(&KeyCode::ControlRight))
        .unwrap_or(false);
    let shift_held = keys
        .as_deref()
        .map(|k| k.is_pressed(&KeyCode::ShiftLeft) || k.is_pressed(&KeyCode::ShiftRight))
        .unwrap_or(false);
    let alt_held = keys
        .as_deref()
        .map(|k| k.is_pressed(&KeyCode::AltLeft) || k.is_pressed(&KeyCode::AltRight))
        .unwrap_or(false);

    let (mut view_proj, mut cam_pos, mut cam_proj, bloom, tone_map, ambient_occlusion) =
        render_queries
            .p0()
            .iter()
            .next()
            .map(|(cam, t, b, tm, ao)| {
                let proj = cam.projection_matrix();
                (
                    proj * t.view_matrix(),
                    t.position.0,
                    proj,
                    b.copied(),
                    tm.copied(),
                    ao.copied(),
                )
            })
            .unwrap_or((Mat4::IDENTITY, Vec3::ZERO, Mat4::IDENTITY, None, None, None));

    // While editing (not Playing), override camera matrices from the orbit
    // camera computed by EditorPlugin. Once Play starts, the viewport should
    // show what the game's own Camera entity sees, same as a build would.
    if let Some(insp) = inspector.as_deref() {
        if insp.editor_mode && insp.play_state == EditorPlayState::Stopped {
            if let Some(vp) = insp.editor_view_proj {
                view_proj = Mat4::from_cols_array_2d(&vp);
            }
            cam_pos = Vec3::from(insp.editor_cam_pos);
            cam_proj = Mat4::from_cols_array_2d(&insp.editor_proj);
        }
    }

    // Rotation-only VP inverse for skybox (no translation → direction-only)
    let sky_vp_inv: Option<Mat4> = if surface.0.has_skybox() {
        render_queries.p0().iter().next().map(|(cam, t, _, _, _)| {
            let proj = cam.projection_matrix();
            let view = t.view_matrix();
            let view_rot = Mat4::from_cols(view.x_axis, view.y_axis, view.z_axis, Vec4::W);
            (proj * view_rot).inverse()
        })
    } else {
        None
    };

    let draw_calls: Vec<(u64, Mat4, Option<u64>, MaterialParams, Option<String>)> = render_queries
        .p1()
        .iter_mut()
        .filter_map(|(mr, t, gt, mat, vis, cs, mut lod)| {
            if !vis.map(|v| v.is_visible).unwrap_or(true) {
                return None;
            }
            let model = gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix());
            let mut world_center: Option<Vec3> = None;
            if let Some((local_center, local_radius)) = registry.get_bounds(mr.mesh_id) {
                let center = (model * local_center.extend(1.0)).truncate();
                world_center = Some(center);
                let max_scale = model
                    .x_axis
                    .truncate()
                    .length()
                    .max(model.y_axis.truncate().length())
                    .max(model.z_axis.truncate().length());
                let world_radius = local_radius * max_scale.max(1.0);
                if !sphere_visible_in_frustum(view_proj, center, world_radius) {
                    return None;
                }
            }
            let effective_mesh_id = if let Some(lod) = lod.as_deref_mut() {
                let distance = world_center
                    .map(|wc| (wc - cam_pos).length())
                    .unwrap_or(f32::MAX);
                lod.current_index = select_lod_level(
                    lod.current_index,
                    distance,
                    &lod.switch_distances,
                    lod.hysteresis_band,
                );
                lod.current_index
                    .and_then(|i| lod.mesh_ids.get(i).copied())
                    .unwrap_or(mr.mesh_id)
            } else {
                mr.mesh_id
            };
            let tex_id = mat.and_then(|m| m.texture_id);
            let mat_params = mat
                .map(|m| MaterialParams {
                    metallic: m.metallic,
                    roughness: m.roughness,
                    emissive: *m.emissive,
                    base_color: *m.base_color,
                    opacity: m.opacity,
                })
                .unwrap_or_default();
            Some((
                effective_mesh_id,
                model,
                tex_id,
                mat_params,
                cs.map(|c| c.path.clone()),
            ))
        })
        .collect();

    let terrain_draw_calls: Vec<(u64, Mat4, [u64; 4], u64)> = render_queries
        .p6()
        .iter()
        .map(|(mr, t, gt, splat)| {
            let model = gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix());
            (
                mr.mesh_id,
                model,
                splat.layer_texture_ids,
                splat.weight_texture_id,
            )
        })
        .collect();

    let collected_point_lights: Vec<PointLightEntry> = render_queries
        .p3()
        .iter()
        .map(|(pl, gt, t)| {
            let pos = gt
                .map(|g| g.to_matrix().w_axis.truncate())
                .unwrap_or(t.position.0);
            PointLightEntry {
                position: pos,
                color: *pl.color,
                intensity: pl.intensity,
                range: pl.range,
            }
        })
        .collect();

    let collected_spot_lights: Vec<SpotLightEntry> = render_queries
        .p4()
        .iter()
        .map(|(sl, gt, t)| spot_light_entry(sl, gt, t))
        .collect();

    let light = if let Some((l, gt, t)) = render_queries.p2().iter().next() {
        let direction = gt
            .map(|g| -glam::Mat3::from_mat4(g.to_matrix()).z_axis)
            .unwrap_or_else(|| t.rotation.0 * Vec3::NEG_Z);
        LightData {
            direction,
            color: *l.color,
            ambient: *l.ambient,
            point_lights: collected_point_lights,
            spot_lights: collected_spot_lights,
        }
    } else {
        LightData {
            point_lights: collected_point_lights,
            spot_lights: collected_spot_lights,
            ..Default::default()
        }
    };

    // One batch per emitter: the texture is bound once per draw, and the
    // particles inside a batch cost one instance each.
    let particle_batches: Vec<bsengine_rhi_wgpu::particles::ParticleBatch> = render_queries
        .p5()
        .iter()
        .filter(|(emitter, _)| !emitter.live.is_empty())
        .map(|(emitter, texture)| {
            let start = *emitter.start_color;
            let end = *emitter.end_color;
            let instances = emitter
                .live
                .iter()
                .map(|p| {
                    let t = (p.age / emitter.particle_lifetime.max(1e-6)).clamp(0.0, 1.0);
                    let colour = start.lerp(end, t);
                    bsengine_rhi_wgpu::particles::ParticleInstance {
                        position: p.position.to_array(),
                        size: emitter.start_size + (emitter.end_size - emitter.start_size) * t,
                        // Alpha fades to nothing over the life, so a particle
                        // thins out instead of blinking away at full opacity.
                        color: [colour.x, colour.y, colour.z, 1.0 - t],
                    }
                })
                .collect();
            bsengine_rhi_wgpu::particles::ParticleBatch {
                texture_id: texture
                    .and_then(|t| texture_cache.as_deref().and_then(|c| c.id_for(&t.0))),
                instances,
            }
        })
        .collect();

    let light_view_proj = compute_light_view_proj(light.direction);
    let tex_reg_ref = tex_registry.as_deref();

    match surface.0.render_frame(
        view_proj,
        cam_pos,
        light_view_proj,
        sky_vp_inv,
        &draw_calls,
        &terrain_draw_calls,
        &registry,
        light,
        tex_reg_ref,
        hud_map,
        ui,
        cursor_x,
        cursor_y,
        left_just_pressed,
        left_just_released,
        cam_proj,
        bloom,
        tone_map,
        ambient_occlusion,
        inspector.as_deref_mut(),
        &key_events_this_frame,
        ctrl_held,
        shift_held,
        alt_held,
        editor_panels.as_deref(),
        type_registry.as_deref(),
        time.as_deref().map(|t| t.elapsed_seconds).unwrap_or(0.0),
        &particle_batches,
    ) {
        Ok(clicked) => {
            if let Some(ref mut state) = ui_state {
                state.clicked = clicked;
            }
        }
        Err(e) => tracing::warn!("render_frame error: {e}"),
    }
}

fn update_camera_aspect(mut events: EventReader<WindowResized>, mut cameras: Query<&mut Camera>) {
    for ev in events.read() {
        for mut cam in cameras.iter_mut() {
            cam.update_aspect_ratio(ev.width, ev.height);
        }
    }
}

/// Bevy plugin that registers the render-related resources, events, and per-frame
/// systems (transform propagation, camera aspect updates, frame rendering).
pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        use bevy_asset::AssetApp;
        // R1: public components must be registered for reflection. Here rather
        // than in `bsengine_scene::register_gameplay_reflect_types` because
        // `bsengine-scene` has no edge to this crate. Unlike the physics and
        // audio plugins, `RenderPlugin` is windowed-only — which is the right
        // scope for `MeshRenderer`: its `mesh_id` names an entry in a GPU mesh
        // registry that only exists once there is a device to upload to, so
        // there is nothing for a headless app to inspect or attach.
        app.register_type::<MeshRenderer>();
        app.register_type::<TerrainSplat>();
        app.register_type::<LodLevels>();
        app.register_type::<Occluder>();
        app.init_asset::<crate::shader_asset::ShaderSource>()
            .register_asset_loader(crate::shader_asset::ShaderSourceLoader)
            .init_resource::<UiState>()
            .init_resource::<PendingShaders>()
            .init_resource::<PendingSkybox>()
            .init_resource::<crate::texture_cache::TextureCache>()
            .add_event::<WindowResized>()
            .add_event::<KeyInput>()
            .add_systems(
                Update,
                (
                    update_camera_aspect,
                    crate::texture_cache::resolve_texture_paths,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    bsengine_core::propagate_global_transforms,
                    compile_pending_shaders,
                    rebuild_modified_shaders,
                    upload_pending_skybox,
                    rebuild_modified_skybox,
                    render_frame,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::{CompileStatus, PendingShader, PendingShaders, PendingSkybox, RenderPlugin};
    use crate::components::{LodLevels, MeshRenderer};
    use bsengine_app::new_app;
    use bsengine_core::{Camera, GlobalTransform, Material, Parent, PointLight, Transform};
    use bsengine_rhi_wgpu::{GpuMeshRegistry, Vertex, WgpuRHIPlugin};
    use bsengine_window::WindowResized;
    use glam::Vec3;

    #[test]
    fn render_plugin_runs_without_surface() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.update();
    }

    // Proves "compile_pending_shaders runs before render_frame, same frame"
    // structurally, via the real `PostUpdate` schedule's topological system
    // order — not via observing `WgpuSurfaceResource` state (has_custom_shader
    // / compile_and_store_shader), which would need a real GPU surface.
    // `WgpuSurface::new` requires a real `Arc<winit::window::Window>`, and
    // `WgpuRHIPlugin`'s surface-creation system only runs given a
    // `WindowHandle` resource, which is only ever produced by
    // `bsengine_window`'s real winit event loop (`App::run`, not `#[test]`);
    // no test anywhere in this workspace constructs a real
    // `WgpuSurfaceResource`, and CI runners have no display. So this test
    // verifies the same thing at the level this codebase can actually reach:
    // `Schedule::systems()`'s iteration order is the executor's genuine
    // topologically-sorted execution order (bevy_ecs's `ScheduleGraph`
    // builds it from `.chain()`'s dependency edges), so finding
    // `compile_pending_shaders` before `render_frame` in that order is a
    // real assertion about execution order, not a restatement of the code.
    #[test]
    fn compile_pending_shaders_runs_before_render_frame() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        // Schedules are only populated with their executable system list
        // after at least one run.
        app.update();

        let schedule = app
            .get_schedule(bevy_app::PostUpdate)
            .expect("RenderPlugin registers systems into PostUpdate");
        let names: Vec<String> = schedule
            .systems()
            .expect("schedule is initialized after app.update()")
            .map(|(_, system)| system.name().to_string())
            .collect();

        let compile_idx = names
            .iter()
            .position(|n| n.contains("compile_pending_shaders"))
            .unwrap_or_else(|| {
                panic!("compile_pending_shaders not found in PostUpdate: {names:?}")
            });
        let skybox_idx = names
            .iter()
            .position(|n| n.contains("upload_pending_skybox"))
            .unwrap_or_else(|| panic!("upload_pending_skybox not found in PostUpdate: {names:?}"));
        let render_idx = names
            .iter()
            .position(|n| n.contains("render_frame"))
            .unwrap_or_else(|| panic!("render_frame not found in PostUpdate: {names:?}"));

        assert!(
            compile_idx < render_idx,
            "compile_pending_shaders (index {compile_idx}) must run before render_frame \
             (index {render_idx}) so shaders compiled this frame are available to it; \
             actual PostUpdate order: {names:?}"
        );
        assert!(
            skybox_idx < render_idx,
            "upload_pending_skybox (index {skybox_idx}) must run before render_frame \
             (index {render_idx}) so a skybox uploaded this frame is available to its \
             has_skybox check; actual PostUpdate order: {names:?}"
        );
    }

    // Same structural argument as the test above, for the reload half.
    // `rebuild_modified_shaders` must sit *after* `compile_pending_shaders`
    // (which is what puts a path into `Ready`, the only state the rebuild
    // looks at -- ahead of it, the very first Modified event would find an
    // empty map) and *before* `render_frame` (or the frame draws with the
    // stale pipeline the reload was meant to replace). Both are `.chain()`
    // edges today; a reorder that starves the rebuild has to fail here.
    #[test]
    fn rebuild_modified_shaders_runs_between_compile_and_render_frame() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        // Schedules are only populated with their executable system list
        // after at least one run.
        app.update();

        let schedule = app
            .get_schedule(bevy_app::PostUpdate)
            .expect("RenderPlugin registers systems into PostUpdate");
        let names: Vec<String> = schedule
            .systems()
            .expect("schedule is initialized after app.update()")
            .map(|(_, system)| system.name().to_string())
            .collect();

        let find = |needle: &str| {
            names
                .iter()
                .position(|n| n.contains(needle))
                .unwrap_or_else(|| panic!("{needle} not found in PostUpdate: {names:?}"))
        };
        let compile_idx = find("compile_pending_shaders");
        let rebuild_idx = find("rebuild_modified_shaders");
        let render_idx = find("render_frame");

        assert!(
            compile_idx < rebuild_idx,
            "compile_pending_shaders (index {compile_idx}) must run before \
             rebuild_modified_shaders (index {rebuild_idx}): it is what records \
             the Ready handle the rebuild matches Modified events against; \
             actual PostUpdate order: {names:?}"
        );
        assert!(
            rebuild_idx < render_idx,
            "rebuild_modified_shaders (index {rebuild_idx}) must run before \
             render_frame (index {render_idx}) so a shader recompiled this frame \
             is the one drawn with; actual PostUpdate order: {names:?}"
        );
    }

    // Same structural argument again, for the skybox's reload half.
    // `rebuild_modified_skybox` must sit *after* `upload_pending_skybox`
    // (which is what puts the slot into `Ready`, the only state the rebuild
    // looks at -- ahead of it, the very first Modified event would find an
    // empty slot) and *before* `render_frame` (or the frame draws the stale
    // sky the reload was meant to replace). Both are `.chain()` edges today;
    // a reorder that starves the rebuild has to fail here.
    #[test]
    fn rebuild_modified_skybox_runs_between_upload_and_render_frame() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        // Schedules are only populated with their executable system list
        // after at least one run.
        app.update();

        let schedule = app
            .get_schedule(bevy_app::PostUpdate)
            .expect("RenderPlugin registers systems into PostUpdate");
        let names: Vec<String> = schedule
            .systems()
            .expect("schedule is initialized after app.update()")
            .map(|(_, system)| system.name().to_string())
            .collect();

        let find = |needle: &str| {
            names
                .iter()
                .position(|n| n.contains(needle))
                .unwrap_or_else(|| panic!("{needle} not found in PostUpdate: {names:?}"))
        };
        let upload_idx = find("upload_pending_skybox");
        let rebuild_idx = find("rebuild_modified_skybox");
        let render_idx = find("render_frame");

        assert!(
            upload_idx < rebuild_idx,
            "upload_pending_skybox (index {upload_idx}) must run before \
             rebuild_modified_skybox (index {rebuild_idx}): it is what records \
             the Ready handle the rebuild matches Modified events against; \
             actual PostUpdate order: {names:?}"
        );
        assert!(
            rebuild_idx < render_idx,
            "rebuild_modified_skybox (index {rebuild_idx}) must run before \
             render_frame (index {render_idx}) so a skybox re-uploaded this \
             frame is the one drawn; actual PostUpdate order: {names:?}"
        );
    }

    #[test]
    fn grandchild_transform_reflects_grandparent_and_parent_after_one_update() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);

        let grandparent = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::new(10.0, 0.0, 0.0)),
                GlobalTransform::default(),
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::new(0.0, 1.0, 0.0)),
                GlobalTransform::default(),
                Parent(grandparent),
            ))
            .id();
        let child = app
            .world_mut()
            .spawn((
                Transform::from_position(Vec3::new(0.0, 0.0, 1.0)),
                GlobalTransform::default(),
                Parent(parent),
            ))
            .id();

        app.update();

        let child_gt = app.world().get::<GlobalTransform>(child).unwrap();
        let pos = child_gt.0.w_axis.truncate();
        assert!(
            (pos - Vec3::new(10.0, 1.0, 1.0)).length() < 1e-4,
            "grandchild GlobalTransform should reflect both ancestors after one RenderPlugin \
             update, got {pos:?} (this fails today because RenderPlugin's transform \
             propagation is one-level-only)"
        );
    }

    // The precondition for shader hot reload, and the only part of it a
    // headless test can reach. `AssetEvent::Modified` only fires while a
    // strong handle to the asset still exists: drop the last one and
    // `Assets::track_assets` frees the asset in PreUpdate, after which
    // `AssetServer::reload` on that path is a silent no-op (measured by
    // `reload_emits_modified_only_while_a_handle_is_retained` in
    // bsengine-gltf). So `Ready` -- which retains the handle -- is what makes
    // `rebuild_modified_shaders` reachable at all.
    //
    // There is no `WgpuSurfaceResource` here (a real one needs a real winit
    // window; see `compile_pending_shaders_runs_before_render_frame`), so the
    // compile itself cannot run and the recompiled pipeline cannot be
    // observed. `Ready` therefore means "source loaded and handle retained",
    // reached whether or not the compile happened -- and the reload assertion
    // below, not the state alone, is what proves the retention is real: a
    // `clone_weak` handle would satisfy `Ready(_)` while still letting the
    // asset be freed.
    #[test]
    fn a_compiled_shader_keeps_its_handle_so_a_reload_can_reach_it() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/shaders/glow.wgsl");
        let path = fixture.to_str().unwrap().to_owned();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.world_mut()
            .spawn(bsengine_core::CustomShader { path: path.clone() });

        let mut ready = false;
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .resource::<PendingShaders>()
                .0
                .get(&path)
                .is_some_and(|state| state.slot.is_ready())
            {
                ready = true;
                break;
            }
        }
        assert!(
            ready,
            "a shader whose source has loaded must end up Ready, holding its \
             handle -- without it AssetEvent::Modified can never fire for it"
        );

        let asset_id = {
            let pending = app.world().resource::<PendingShaders>();
            let Some(state) = pending.0.get(&path) else {
                unreachable!("just asserted Ready")
            };
            state.slot.handle().id()
        };

        // A few more frames so `track_assets` (PreUpdate) has had every chance
        // to free the source. It only survives this if something still holds a
        // *strong* handle to it.
        for _ in 0..5 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<Assets<crate::shader_asset::ShaderSource>>()
                .get(asset_id)
                .is_some(),
            "the retained handle must keep the source alive; a weak one lets \
             track_assets free it, and reload then has nothing to reload"
        );

        // Read `Modified` specifically, and only events emitted after this
        // point: the buffer still holds the `Added`/`LoadedWithDependencies`
        // events the initial load emitted, so a bare length check would pass
        // even if the reload reached nothing at all.
        let mut reader: ManualEventReader<AssetEvent<crate::shader_asset::ShaderSource>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<crate::shader_asset::ShaderSource>>>()
            .get_reader();
        {
            let events = app
                .world()
                .resource::<Events<AssetEvent<crate::shader_asset::ShaderSource>>>();
            let _ = reader.read(events).count();
        }

        app.world().resource::<AssetServer>().reload(path);
        let mut saw_modified = false;
        for _ in 0..60 {
            app.update();
            let events = app
                .world()
                .resource::<Events<AssetEvent<crate::shader_asset::ShaderSource>>>();
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
            "reloading a shader whose handle is retained must emit \
             AssetEvent::Modified for it; none means the handle was dropped \
             and hot reload is impossible for custom shaders"
        );
    }

    // A shader whose *source* loads but does not compile must not be
    // recompiled every frame: the file is broken, the next attempt fails
    // identically, and the only visible effect is one warning per frame. The
    // pipeline is what `compile_and_store_shader` refuses to store, and that
    // cannot be observed here (no `WgpuSurfaceResource`; a real one needs a
    // real winit window -- see `compile_pending_shaders_runs_before_render_frame`),
    // so this pins the two properties on this side of the boundary that make
    // "retry on content change, never on the frame clock" work: the state is
    // stable across frames, and the handle it holds still routes
    // `AssetEvent::Modified` so the fixed file can reach
    // `rebuild_modified_shaders`.
    #[test]
    fn a_shader_that_failed_to_compile_is_not_retried_every_frame_but_stays_reloadable() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};

        let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../games/mini-arena/assets/shaders/glow.wgsl");
        let path = fixture.to_str().unwrap().to_owned();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.world_mut()
            .spawn(bsengine_core::CustomShader { path: path.clone() });

        let mut asset_id = None;
        for _ in 0..200 {
            app.update();
            if let Some(handle) = app
                .world()
                .resource::<PendingShaders>()
                .0
                .get(&path)
                .filter(|state| state.slot.is_ready())
                .map(|state| state.slot.handle())
            {
                asset_id = Some(handle.id());
                break;
            }
        }
        let asset_id = asset_id.expect("the fixture shader's source must load");

        // Stand in for what a surface would have done with a broken file: the
        // same handle, moved to the state a failed compile records.
        {
            let mut pending = app.world_mut().resource_mut::<PendingShaders>();
            let Some(state) = pending.0.get_mut(&path) else {
                unreachable!("just asserted the source loaded")
            };
            state.compile = CompileStatus::Failed;
        }

        for _ in 0..20 {
            app.update();
        }
        let state = app.world().resource::<PendingShaders>().0.get(&path);
        assert!(
            state.is_some_and(|state| {
                state.compile == CompileStatus::Failed && state.slot.handle().id() == asset_id
            }),
            "a shader that failed to compile must stay Failed, holding the same \
             handle: anything that moves it back to NotYet makes the next frame \
             compile the same broken file again, one warning per frame forever; \
             got {state:?}"
        );
        assert!(
            app.world()
                .resource::<Assets<crate::shader_asset::ShaderSource>>()
                .get(asset_id)
                .is_some(),
            "CompileFailed must retain a strong handle; dropping it lets \
             track_assets free the source, and the fix the user is about to \
             type can never arrive as AssetEvent::Modified"
        );

        // Only events emitted from here on: the buffer still holds the load's
        // own Added/LoadedWithDependencies events.
        let mut reader: ManualEventReader<AssetEvent<crate::shader_asset::ShaderSource>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<crate::shader_asset::ShaderSource>>>()
            .get_reader();
        {
            let events = app
                .world()
                .resource::<Events<AssetEvent<crate::shader_asset::ShaderSource>>>();
            let _ = reader.read(events).count();
        }

        app.world().resource::<AssetServer>().reload(path);
        let mut saw_modified = false;
        for _ in 0..60 {
            app.update();
            let events = app
                .world()
                .resource::<Events<AssetEvent<crate::shader_asset::ShaderSource>>>();
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
            "editing a shader that previously failed to compile must still emit \
             AssetEvent::Modified for it -- that event is the only thing \
             rebuild_modified_shaders acts on, and without it a single typo \
             would be permanent for the rest of the run"
        );
    }

    // The pure half of the same rule, checked directly because no test in this
    // workspace can run a real compile (that needs a GPU surface). Recording a
    // failure as `Ready` is what would put the compile back on the frame clock;
    // dropping the handle is what would make the failure permanent.
    #[test]
    fn a_failed_compile_is_recorded_as_compile_failed_and_keeps_its_handle() {
        use bevy_asset::{AssetServer, Handle};

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        let handle: Handle<crate::shader_asset::ShaderSource> = app
            .world()
            .resource::<AssetServer>()
            .load("some/shader.wgsl".to_owned());

        // The handle half of this test is gone because the bug it guarded is
        // gone: a compile verdict is its own field now and cannot reach the
        // handle at all, where the old `state_after_compile` returned a whole
        // state and could have dropped one. `PendingShader` still holds it, and
        // `a_shader_that_failed_to_compile_is_not_retried_every_frame_but_stays_reloadable`
        // is the behavioural check that it survives a failed compile.
        let mut state = PendingShader {
            slot: bsengine_asset::AssetSlot::from_handle(handle.clone()),
            compile: super::compile_status(Ok(())),
        };
        assert_eq!(
            state.compile,
            CompileStatus::Ok,
            "a successful compile must be recorded as Ok"
        );
        assert!(
            state.slot.handle().id() == handle.id(),
            "recording a verdict must not disturb the load's handle"
        );

        state.compile = super::compile_status(Err("bad wgsl".to_string()));
        assert_eq!(
            state.compile,
            CompileStatus::Failed,
            "a failed compile must be recorded as Failed -- Ok would have the \
             next frame recompile the same broken source"
        );
        assert!(
            state.slot.handle().id() == handle.id(),
            "a failed compile must leave the handle alone, or the fix can never \
             arrive as AssetEvent::Modified"
        );
    }

    #[test]
    fn a_broken_shader_is_still_selected_for_rebuild_when_its_file_changes() {
        use bevy_asset::{AssetServer, Handle};

        // The "stays reloadable" half of
        // `a_shader_that_failed_to_compile_is_not_retried_every_frame_but_stays_reloadable`,
        // which that test does not actually reach: it never edits the file, so
        // `rebuild_modified_shaders` never runs, and skipping broken shaders
        // there leaves it green. Driving the real system instead needs a real
        // `WgpuSurfaceResource`, which needs a real winit window -- so what is
        // measured here is the selection itself, the one decision that would
        // make a typo permanent for the rest of the run.
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        let handle: Handle<crate::shader_asset::ShaderSource> = app
            .world()
            .resource::<AssetServer>()
            .load("some/shader.wgsl".to_owned());
        let id = handle.id();

        for compile in [
            CompileStatus::Ok,
            CompileStatus::Failed,
            CompileStatus::NotYet,
        ] {
            let state = PendingShader {
                slot: bsengine_asset::AssetSlot::Ready(handle.clone()),
                compile,
            };
            assert!(
                super::wants_rebuild(&state, id),
                "an edit must rebuild a loaded shader whatever the compiler last \
                 said about it, including {:?}",
                state.compile
            );
        }

        // A source that never arrived has nothing to recompile.
        let still_loading = PendingShader {
            slot: bsengine_asset::AssetSlot::Loading(handle.clone()),
            compile: CompileStatus::NotYet,
        };
        assert!(!super::wants_rebuild(&still_loading, id));
        let gave_up = PendingShader {
            slot: bsengine_asset::AssetSlot::GaveUp(handle.clone()),
            compile: CompileStatus::NotYet,
        };
        assert!(!super::wants_rebuild(&gave_up, id));

        // And an edit to some *other* shader is not this one's business.
        let other: Handle<crate::shader_asset::ShaderSource> = app
            .world()
            .resource::<AssetServer>()
            .load("another/shader.wgsl".to_owned());
        let ready = PendingShader {
            slot: bsengine_asset::AssetSlot::Ready(handle),
            compile: CompileStatus::Ok,
        };
        assert!(!super::wants_rebuild(&ready, other.id()));
    }

    // A shader path that cannot load must be given up on. Re-requesting a
    // failed path every frame resets it to Loading and respawns the load, so
    // the failure is never observable and the warning never fires -- the
    // blocking path this replaced warned once and stopped.
    #[test]
    fn missing_shader_is_given_up_on_instead_of_retried_forever() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn(bsengine_core::CustomShader {
            path: "definitely/not/a/real/shader.wgsl".to_string(),
        });

        let gave_up = |app: &bsengine_app::App| {
            app.world()
                .resource::<PendingShaders>()
                .0
                .get("definitely/not/a/real/shader.wgsl")
                .is_some_and(|state| state.slot.gave_up())
        };

        let mut settled = false;
        for _ in 0..200 {
            app.update();
            if gave_up(&app) {
                settled = true;
                break;
            }
        }
        assert!(settled, "an unloadable shader path must end up given up on");

        // And stays given up on every frame, not merely on the one this happens
        // to sample. A loop that re-requests the failed path also passes back
        // through the give-up state, so a single late reading cannot tell the
        // two apart -- which is the whole distinction this test is named for.
        for frame in 0..60 {
            app.update();
            assert!(
                gave_up(&app),
                "the shader left GaveUp on frame {frame}, which means something                  re-requested the failed path"
            );
        }
    }

    /// A valid 1×1 RGBA PNG. The repo ships no image files, and this test needs
    /// one that actually decodes -- a failed load ends in `GaveUp`, which holds
    /// no handle and would make the assertion below vacuous.
    const MINIMAL_PNG_1X1: &[u8] = &[
        0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1f,
        0x15, 0xc4, 0x89, 0x00, 0x00, 0x00, 0x0d, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9c, 0x63, 0xf8,
        0xcf, 0xc0, 0xf0, 0x1f, 0x00, 0x05, 0x00, 0x01, 0xff, 0x89, 0x99, 0x3d, 0x1d, 0x00, 0x00,
        0x00, 0x00, 0x49, 0x45, 0x4e, 0x44, 0xae, 0x42, 0x60, 0x82,
    ];

    // The skybox half of the same precondition
    // `a_compiled_shader_keeps_its_handle_so_a_reload_can_reach_it` pins for
    // shaders: `AssetEvent::Modified` only fires while a strong handle to the
    // asset still exists, so the moment the last one drops,
    // `Assets::track_assets` frees the image in PreUpdate and
    // `AssetServer::reload` on that path is a silent no-op. `Ready` -- which
    // retains the handle instead of clearing the slot on a successful upload --
    // is what makes `rebuild_modified_skybox` reachable at all.
    //
    // There is no `WgpuSurfaceResource` here (a real one needs a real winit
    // window; see `compile_pending_shaders_runs_before_render_frame`), so the
    // upload itself cannot run and the re-uploaded skybox cannot be observed.
    // `Ready` therefore means "image decoded and handle retained", reached
    // whether or not the upload happened -- and the two assertions after it,
    // not the state alone, are what prove the retention is real: a `clone_weak`
    // handle satisfies `Ready(_)` while still letting the image be freed.
    #[test]
    fn an_uploaded_skybox_keeps_its_handle_so_a_reload_can_reach_it() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};
        use bsengine_asset::TextureAsset;

        let dir = std::env::temp_dir().join(format!(
            "bsengine_test_skybox_reload_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("sky.png");
        std::fs::write(&png, MINIMAL_PNG_1X1).unwrap();
        let path = png.to_string_lossy().to_string();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.insert_resource(bsengine_core::SkyboxPath(Some(path.clone())));

        let mut ready = false;
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .resource::<PendingSkybox>()
                .0
                .as_ref()
                .is_some_and(|load| load.slot.is_ready())
            {
                ready = true;
                break;
            }
        }
        assert!(
            ready,
            "the skybox slot must end up Ready, still holding its handle -- \
             clearing it frees the asset and makes reload a silent no-op"
        );

        let asset_id = {
            let pending = &app.world().resource::<PendingSkybox>().0;
            let Some(load) = pending.as_ref() else {
                unreachable!("just asserted Ready")
            };
            load.slot.handle().id()
        };

        // A few more frames so `track_assets` (PreUpdate) has had every chance
        // to free the image. It only survives this if something still holds a
        // *strong* handle to it.
        for _ in 0..5 {
            app.update();
        }
        assert!(
            app.world()
                .resource::<Assets<TextureAsset>>()
                .get(asset_id)
                .is_some(),
            "the retained handle must keep the image alive; a weak one lets \
             track_assets free it, and reload then has nothing to reload"
        );

        // Read `Modified` specifically, and only events emitted after this
        // point: the buffer still holds the `Added`/`LoadedWithDependencies`
        // events the initial load emitted, so a bare length check would pass
        // even if the reload reached nothing at all.
        let mut reader: ManualEventReader<AssetEvent<TextureAsset>> = app
            .world_mut()
            .resource_mut::<Events<AssetEvent<TextureAsset>>>()
            .get_reader();
        {
            let events = app.world().resource::<Events<AssetEvent<TextureAsset>>>();
            let _ = reader.read(events).count();
        }

        app.world().resource::<AssetServer>().reload(path);
        let mut saw_modified = false;
        for _ in 0..60 {
            app.update();
            let events = app.world().resource::<Events<AssetEvent<TextureAsset>>>();
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
            "reloading a skybox whose handle is retained must emit \
             AssetEvent::Modified for it; none means the handle was dropped \
             and hot reload is impossible for the skybox"
        );

        let _ = std::fs::remove_file(&png);
    }

    // The skybox is the one consumer that never went through
    // `bsengine_asset::load` -- that dispatcher wants a `sync_loader` closure
    // for its `Sync` arm and this codebase has no synchronous texture loader --
    // so it is the one that could silently stay invisible to `AssetStatuses`.
    //
    // A *successful* load is what proves the routing. A failing one would be
    // reported anyway: `UntypedAssetLoadFailedEvent` reaches the collector
    // whether or not the request was ever recorded, so a missing-file version
    // of this test would pass with the recording removed.
    #[test]
    fn a_loaded_skybox_is_reported_by_asset_statuses() {
        use bsengine_asset::{AssetStatus, AssetStatusPlugin, AssetStatuses};

        let dir = std::env::temp_dir().join(format!(
            "bsengine_test_skybox_status_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("sky.png");
        std::fs::write(&png, MINIMAL_PNG_1X1).unwrap();
        let path = png.to_string_lossy().to_string();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(AssetStatusPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.insert_resource(bsengine_core::SkyboxPath(Some(path.clone())));

        let mut status = AssetStatus::Unknown;
        for _ in 0..200 {
            app.update();
            status = app.world().resource::<AssetStatuses>().get(&path);
            if status == AssetStatus::Loaded {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(1));
        }

        let _ = std::fs::remove_file(&png);
        assert_eq!(
            status,
            AssetStatus::Loaded,
            "a skybox that loaded must be reported as loaded -- requesting it \
             straight from the AssetServer is what kept it invisible until it failed"
        );
    }

    // The skybox equivalent of the shader test above, and for the same
    // reason: `upload_pending_skybox` requests the texture once and polls the
    // handle it kept. Re-requesting the path each frame would reset the failed
    // load to `Loading` and restart it, so the give-up state would never be
    // reached and this would spin forever.
    #[test]
    fn missing_skybox_is_given_up_on_instead_of_retried_forever() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.insert_resource(bsengine_core::SkyboxPath(Some(
            "definitely/not/a/real/sky.png".to_string(),
        )));

        let gave_up = |app: &bsengine_app::App| {
            app.world()
                .resource::<PendingSkybox>()
                .0
                .as_ref()
                .is_some_and(|load| load.slot.gave_up())
        };

        let mut settled = false;
        for _ in 0..200 {
            app.update();
            if gave_up(&app) {
                settled = true;
                break;
            }
        }
        assert!(settled, "an unloadable skybox path must end up given up on");

        // Then *stays* given up on, on every frame rather than merely on the
        // one this happens to sample. A loop that re-requests the failed path
        // also passes through `GaveUp` repeatedly, so a single reading taken
        // after N frames cannot tell a give-up from an infinite retry -- which
        // is the entire property this test is named for. Measured: re-clearing
        // the slot when it reports `gave_up` leaves the end-state assertion
        // green and fails this one.
        for frame in 0..60 {
            app.update();
            assert!(
                gave_up(&app),
                "a given-up skybox left GaveUp on frame {frame}, which means \
                 something re-requested the failed path"
            );
        }
    }

    // Changing `SkyboxPath` mid-load must abandon the in-flight request and
    // start the new one, or the old texture lands on screen a frame after the
    // user asked for a different sky. Two frames is the whole story: the first
    // reaches the request arm (empty slot -> `Loading`, returns immediately),
    // the second reaches the abandon-and-re-request branch.
    //
    // The handle is asserted on, not just the retained path string: the
    // give-up arm writes the *wanted* path next to the *old* handle, so an
    // implementation that kept polling the abandoned load can still end up
    // with "second/sky.png" in the slot. Only the handle says which load is
    // actually in flight.
    #[test]
    fn skybox_path_change_mid_load_requests_the_new_path_instead() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.insert_resource(bsengine_core::SkyboxPath(Some("first/sky.png".to_string())));

        app.update();
        let pending = &app.world().resource::<PendingSkybox>().0;
        assert!(
            pending.as_ref().is_some_and(|load| {
                matches!(load.slot, bsengine_asset::AssetSlot::Loading(_))
                    && load.path == "first/sky.png"
            }),
            "precondition: one frame must leave the first path in flight, got {pending:?}"
        );

        app.world_mut()
            .resource_mut::<bsengine_core::SkyboxPath>()
            .0 = Some("second/sky.png".to_string());
        app.update();

        let pending = &app.world().resource::<PendingSkybox>().0;
        let Some((path, handle)) = pending.as_ref().and_then(|load| match &load.slot {
            bsengine_asset::AssetSlot::Loading(handle) => Some((&load.path, handle)),
            _ => None,
        }) else {
            panic!("a changed SkyboxPath must leave a load for the new path in flight, got {pending:?}");
        };
        assert_eq!(
            path, "second/sky.png",
            "the retained path must be the newly wanted one"
        );
        assert_eq!(
            handle.path().map(ToString::to_string).as_deref(),
            Some("second/sky.png"),
            "the retained handle must be the one requested for the new path -- keeping the \
             old path's handle means the abandoned load is still being polled and its \
             texture can still be uploaded"
        );
    }

    // Setting `SkyboxPath.0` to `None` mid-load must drop the in-flight
    // request with it. Left in place, the load completes a frame or two later
    // and uploads a sky the user has already switched off. Observable without
    // a surface: with no `WgpuSurfaceResource` the dedupe check above can't
    // match, so control reaches the skybox-off arm, which clears `pending`
    // whether or not there is a surface to clear.
    #[test]
    fn turning_the_skybox_off_mid_load_drops_the_in_flight_request() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        app.insert_resource(bsengine_core::SkyboxPath(Some("some/sky.png".to_string())));

        app.update();
        let pending = &app.world().resource::<PendingSkybox>().0;
        assert!(
            pending
                .as_ref()
                .is_some_and(|load| matches!(load.slot, bsengine_asset::AssetSlot::Loading(_))),
            "precondition: one frame must leave a load in flight, got {pending:?}"
        );

        app.world_mut()
            .resource_mut::<bsengine_core::SkyboxPath>()
            .0 = None;
        app.update();

        let pending = &app.world().resource::<PendingSkybox>().0;
        assert!(
            pending.is_none(),
            "turning the skybox off must drop the in-flight request, got {pending:?}"
        );
    }

    #[test]
    fn render_plugin_runs_with_rhi_headless() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.update();
        app.update();
        app.update();
    }

    #[test]
    fn camera_aspect_updates_on_window_resize() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);

        let cam_entity = app.world_mut().spawn(Camera::default()).id();
        app.world_mut().send_event(WindowResized {
            width: 800,
            height: 600,
        });
        app.update();

        let cam = app.world().get::<Camera>(cam_entity).unwrap();
        let expected = 800.0_f32 / 600.0_f32;
        assert!((cam.aspect_ratio - expected).abs() < 1e-4);
    }

    #[test]
    fn render_plugin_accepts_point_lights() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            PointLight {
                color: Vec3::new(1.0, 0.5, 0.0).into(),
                intensity: 2.0,
                range: 5.0,
            },
            Transform::from_position(Vec3::new(0.0, 2.0, 0.0)),
        ));
        app.update();
    }

    #[test]
    fn render_plugin_uses_pbr_material() {
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            MeshRenderer { mesh_id: 999 },
            Transform::from_position(Vec3::ZERO),
            Material {
                metallic: 0.8,
                roughness: 0.2,
                emissive: Vec3::new(0.1, 0.0, 0.0).into(),
                ..Default::default()
            },
        ));
        app.update();
    }

    #[test]
    fn render_plugin_accepts_spot_lights() {
        use bsengine_core::SpotLight;
        let mut app = new_app();
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(RenderPlugin);
        app.world_mut().spawn((
            SpotLight {
                color: Vec3::new(0.9, 0.9, 1.0).into(),
                intensity: 3.0,
                range: 12.0,
                ..Default::default()
            },
            Transform::from_position(Vec3::new(0.0, 5.0, 0.0)),
        ));
        app.update();
    }

    #[test]
    fn spot_light_entry_converts_degrees_to_radians() {
        use bsengine_core::SpotLight;

        let sl = SpotLight {
            inner_angle_degrees: 45.0.into(),
            outer_angle_degrees: 60.0.into(),
            ..SpotLight::default()
        };
        let t = Transform::from_position(Vec3::new(0.0, 5.0, 0.0));

        let entry = super::spot_light_entry(&sl, None, &t);

        assert!((entry.inner_angle - 45_f32.to_radians()).abs() < 1e-6);
        assert!((entry.outer_angle - 60_f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn light_view_proj_is_invertible() {
        use super::compute_light_view_proj;
        let dir = Vec3::new(-0.4, -0.8, -0.4).normalize();
        let vp = compute_light_view_proj(dir);
        assert!(
            vp.determinant().abs() > 1e-6,
            "light VP should be invertible"
        );
    }

    #[test]
    fn light_view_proj_up_axis_does_not_degenerate() {
        use super::compute_light_view_proj;
        // straight-down light — should pick Z as up without NaN/zero-det
        let vp = compute_light_view_proj(Vec3::new(0.0, -1.0, 0.0));
        assert!(vp.determinant().abs() > 1e-6);
    }

    #[test]
    fn frustum_cull_sphere_in_front_is_visible() {
        use super::sphere_visible_in_frustum;
        use glam::Mat4;
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        assert!(sphere_visible_in_frustum(
            vp,
            Vec3::new(0.0, 0.0, -5.0),
            0.5
        ));
    }

    #[test]
    fn frustum_cull_sphere_behind_camera_is_culled() {
        use super::sphere_visible_in_frustum;
        use glam::Mat4;
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        assert!(!sphere_visible_in_frustum(
            vp,
            Vec3::new(0.0, 0.0, 5.0),
            0.5
        ));
    }

    #[test]
    fn frustum_cull_sphere_past_far_plane_is_culled() {
        use super::sphere_visible_in_frustum;
        use glam::Mat4;
        let vp = Mat4::perspective_rh(std::f32::consts::FRAC_PI_3, 1.0, 0.1, 100.0);
        assert!(!sphere_visible_in_frustum(
            vp,
            Vec3::new(0.0, 0.0, -150.0),
            0.5
        ));
    }

    // Proves `render_frame` actually drives LOD selection end to end: a real
    // (offscreen, no window needed) GPU registry gives the entity's mesh a
    // real bounding sphere, so `world_center` -- and therefore the distance
    // fed to `select_lod_level` -- comes from the genuine camera-to-object
    // distance, not the `f32::MAX` fallback an unregistered mesh id would
    // produce. `WgpuRHIPlugin::offscreen` is the same helper
    // `bsengine-runtime`'s headless test/replay runtime uses to get a real
    // renderer without a window (see `test_mode.rs`); `render_plugin_uses_pbr_material`
    // and friends above use `WgpuRHIPlugin::windowed()` instead, but windowed
    // mode never gets a `WindowHandle` in a test, so its `WgpuSurfaceResource`
    // -- and therefore `GpuMeshRegistry` -- never comes into existence, and
    // `render_frame` takes its early return before ever reaching the LOD
    // selection this test needs to exercise.
    #[test]
    fn lod_current_index_updates_based_on_camera_distance() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, true));
        app.add_plugins(RenderPlugin);
        // Startup (which builds the offscreen surface and GpuMeshRegistry)
        // only runs on the first update.
        app.update();

        let mesh_id = {
            let mut registry = app.world_mut().resource_mut::<GpuMeshRegistry>();
            registry.register(
                &[
                    Vertex {
                        position: [0.0, 0.0, 0.0],
                        color: [1.0, 1.0, 1.0],
                        normal: [0.0, 1.0, 0.0],
                        uv: [0.0, 0.0],
                    },
                    Vertex {
                        position: [1.0, 0.0, 0.0],
                        color: [1.0, 1.0, 1.0],
                        normal: [0.0, 1.0, 0.0],
                        uv: [1.0, 0.0],
                    },
                    Vertex {
                        position: [0.0, 1.0, 0.0],
                        color: [1.0, 1.0, 1.0],
                        normal: [0.0, 1.0, 0.0],
                        uv: [0.0, 1.0],
                    },
                ],
                &[0, 1, 2],
            )
        };

        // Camera far from the origin -- comfortably past both switch
        // thresholds (plus their hysteresis half-bands), so this must cross
        // at least the first one.
        app.world_mut().spawn((
            Camera::default(),
            Transform::from_position(Vec3::new(0.0, 0.0, 200.0)),
        ));

        let entity = app
            .world_mut()
            .spawn((
                MeshRenderer { mesh_id },
                Transform::from_position(Vec3::ZERO),
                LodLevels {
                    // These don't need to be registered meshes -- this test
                    // only asserts on `current_index`, never draws them.
                    mesh_ids: vec![mesh_id + 100, mesh_id + 200],
                    switch_distances: vec![10.0, 50.0],
                    hysteresis_band: 2.0,
                    current_index: None,
                },
            ))
            .id();

        app.update();

        let lod = app
            .world()
            .get::<LodLevels>(entity)
            .expect("entity still carries its LodLevels component");
        assert!(
            lod.current_index.is_some(),
            "an entity 200 units from the camera, with switch_distances \
             [10.0, 50.0], must have selected a LOD level beyond LOD0 -- got \
             current_index = None"
        );
    }
}
