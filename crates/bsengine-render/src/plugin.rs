use bevy_app::{App, Plugin, PostUpdate, Startup, Update};
use bevy_ecs::prelude::{Entity, EventReader, IntoSystemConfigs, Local, ParamSet, Query, ResMut};
use bsengine_core::{
    AmbientOcclusion, Bloom, Camera, CustomShader, DirectionalLight, EditorPanelRegistry,
    EditorPlayState, GlobalTransform, HudTexts, InspectorState, Material, PointLight, SkyboxPath,
    SpotLight, Taa, Time, ToneMap, Transform, UiState, Visible,
};
use bsengine_ecs::Res;
use bsengine_input::{Input, KeyCode, KeyInput, MouseButton, MouseState};
use bsengine_rhi_wgpu::{
    GpuMeshRegistry, GpuTextureRegistry, LightData, MaterialParams, PointLightEntry,
    SpotLightEntry, WgpuSurfaceResource,
};
use bsengine_window::WindowResized;
use glam::{Mat4, Vec3, Vec4};
use rayon::prelude::*;

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

/// The skybox the surface shows: the path it came from and the
/// `GpuTextureRegistry::generation` of the registry object it was copied
/// from, so a rebuilt object -- a hot reload, a streamed level arriving -- is
/// copied again and a frame never keeps drawing a sky the registry has moved
/// on from. `None` while nothing is shown.
///
/// Internal to [`sync_skybox`], like `bsengine_gltf`'s `PendingGltf`:
/// `SkyboxPath` stays a plain `String`, so scene RON, the scripting API and
/// the MCP tools are unaffected by how a load is tracked.
#[derive(bevy_ecs::prelude::Resource, Default, Debug)]
struct ShownSkybox {
    shown: Option<(String, u64)>,
    /// The path a give-up was already reported for, so it is said once.
    warned: Option<String>,
}

impl ShownSkybox {
    /// The generation of the registry object the shown sky was copied from,
    /// for tests that watch it change.
    #[cfg(test)]
    fn generation(&self) -> Option<u64> {
        self.shown.as_ref().map(|(_, generation)| *generation)
    }
}

/// Keeps the surface's skybox in sync with `SkyboxPath`, through the texture
/// cache.
///
/// The image is requested, polled and uploaded by
/// [`crate::texture_cache::TextureCache::upload`], exactly as a material's or a
/// UI image's is, and the skybox copies the registry's GPU object
/// (`WgpuSurface::set_skybox_from_texture`). It used to request its own handle
/// and upload the pixels out of `Assets` for itself, which is the reason
/// `TextureImportSettings::release_pixels` had to stay off by default: an image
/// a material had uploaded first had no pixels left for the skybox to read.
/// One owner of the GPU copy, and this asks it by path.
///
/// Copied again whenever the registry rebuilds the object behind the id --
/// its generation changes on a hot reload (the cache re-uploads a modified
/// asset under the same id) and on every streamed level that arrives -- and
/// whenever the surface stops showing the path (a rebuilt surface). That is
/// the whole of the bookkeeping: the separate reload system that listened for
/// asset events is gone with the handle it matched them against.
///
/// Item 23 split the surface's old blocking `set_skybox` (path in,
/// `image::open`, upload) into decode and upload halves, and the blocking half
/// has since been deleted outright -- nothing in the engine may stall a frame
/// on a file read. This gets its own system rather than staying in
/// `render_frame` because that function is already at Bevy 0.14's
/// 16-top-level-param ceiling (see the comment on its `render_queries` param),
/// and because waiting across frames is not a render pass's job.
///
/// Without a `GpuTextureRegistry` nothing can be uploaded and the request is
/// made on the first frame one exists, as for materials. A load the cache
/// gave up on is reported once.
fn sync_skybox(
    mut surface: Option<ResMut<WgpuSurfaceResource>>,
    skybox_path: Option<Res<SkyboxPath>>,
    mut cache: ResMut<crate::texture_cache::TextureCache>,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
    mut textures: ResMut<bevy_asset::Assets<bsengine_asset::TextureAsset>>,
    registry: Option<ResMut<bsengine_rhi_wgpu::GpuTextureRegistry>>,
    mut shown: ResMut<ShownSkybox>,
) {
    let wanted = skybox_path.and_then(|p| p.0.clone());
    let Some(wanted) = wanted else {
        // Off. Cleared once: the IBL maps go with the skybox, and clearing
        // an already-clear surface every frame would keep dropping nothing.
        if let Some(surface) = surface.as_mut() {
            if surface.0.has_skybox() {
                surface.0.clear_skybox();
            }
        }
        shown.shown = None;
        return;
    };
    let Some(mut registry) = registry else {
        return;
    };
    let Some(id) = cache.upload(&wanted, &asset_server, &mut textures, &mut registry) else {
        if cache.gave_up(&wanted) && shown.warned.as_deref() != Some(wanted.as_str()) {
            tracing::warn!("skybox: cannot read '{wanted}'");
            shown.warned = Some(wanted);
        }
        return;
    };
    let generation = registry
        .generation(id)
        .expect("an id the cache returned is loaded");
    let Some(surface) = surface.as_mut() else {
        return;
    };
    // Already showing this path, copied from this very object: nothing to do.
    // Both halves matter. The surface's own record is what lets a rebuilt
    // surface get its skybox back; the generation is what makes a hot reload
    // or a streamed level reach the screen.
    let current = surface.0.loaded_skybox_path() == Some(wanted.as_str())
        && shown
            .shown
            .as_ref()
            .is_some_and(|(path, from)| *path == wanted && *from == generation);
    if current {
        return;
    }
    let texture = registry
        .get_texture(id)
        .expect("an id the cache returned is loaded");
    surface.0.set_skybox_from_texture(texture);
    // `set_skybox_from_texture` doesn't record the path (that bookkeeping
    // lived in the now-deleted blocking `set_skybox`), so do it here.
    surface.0.set_loaded_skybox_path(&wanted);
    shown.shown = Some((wanted, generation));
}

/// The colour LUT the surface is showing: the path and the registry
/// generation it was copied from, as [`ShownSkybox`] tracks the sky's.
#[derive(bevy_ecs::prelude::Resource, Default, Debug)]
struct ShownColorLut {
    shown: Option<(String, u64)>,
    /// The path a failure was already reported for, so it is said once.
    warned: Option<String>,
}

/// Keeps the post pass's colour LUT in sync with the camera's
/// `ColorGrading::lut`, through the texture cache -- [`sync_skybox`]'s
/// arrangement, for the same reasons: one owner of the GPU copy, asked by
/// path, and re-copied when the registry rebuilds the object (a hot reload).
///
/// A LUT the post pass refuses -- not a strip, or block-compressed -- is
/// reported once and *unbound*, rather than leaving the previous LUT on
/// screen under the new one's name. The first camera with a `ColorGrading`
/// is the one read; `render_frame` takes the grade's other settings from
/// the first camera too.
#[allow(clippy::too_many_arguments)]
fn sync_color_lut(
    mut surface: Option<ResMut<WgpuSurfaceResource>>,
    cameras: Query<&bsengine_core::ColorGrading, bevy_ecs::prelude::With<Camera>>,
    mut cache: ResMut<crate::texture_cache::TextureCache>,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
    mut textures: ResMut<bevy_asset::Assets<bsengine_asset::TextureAsset>>,
    registry: Option<ResMut<bsengine_rhi_wgpu::GpuTextureRegistry>>,
    mut shown: ResMut<ShownColorLut>,
) {
    let Some(surface) = surface.as_mut() else {
        return;
    };
    let wanted = cameras
        .iter()
        .next()
        .map(|g| g.lut.clone())
        .filter(|path| !path.is_empty());
    let Some(wanted) = wanted else {
        if shown.shown.is_some() || surface.0.color_lut_size() > 0 {
            let _ = surface.0.set_color_lut_from_texture(None);
        }
        shown.shown = None;
        return;
    };
    let Some(mut registry) = registry else {
        return;
    };
    let Some(id) = cache.upload(&wanted, &asset_server, &mut textures, &mut registry) else {
        if cache.gave_up(&wanted) && shown.warned.as_deref() != Some(wanted.as_str()) {
            tracing::warn!("colour grading LUT: cannot read '{wanted}'");
            shown.warned = Some(wanted);
        }
        return;
    };
    let generation = registry
        .generation(id)
        .expect("an id the cache returned is loaded");
    if shown
        .shown
        .as_ref()
        .is_some_and(|(path, from)| *path == wanted && *from == generation)
    {
        return;
    }
    let texture = registry
        .get_texture(id)
        .expect("an id the cache returned is loaded");
    if let Err(e) = surface.0.set_color_lut_from_texture(Some(texture)) {
        if shown.warned.as_deref() != Some(wanted.as_str()) {
            tracing::warn!("colour grading LUT '{wanted}' not applied: {e}");
            shown.warned = Some(wanted.clone());
        }
        let _ = surface.0.set_color_lut_from_texture(None);
    }
    // Recorded on a refusal too, so a bad LUT is tried once per change
    // rather than copied and refused every frame.
    shown.shown = Some((wanted, generation));
}

/// Pixels scrolled per unit of wheel delta.
///
/// A wheel notch reports 1.0, and 40 pixels is roughly a line and a half --
/// close to what a desktop toolkit moves per notch, which is the only reference
/// a player's hand has.
const WHEEL_PIXELS: f32 = 40.0;

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
    // Folding these Querys into one param keeps the total within that limit.
    //
    // This ParamSet is now FULL: Bevy 0.14's `impl_param_set!` macro is
    // generated with `max_params = 8`, so there is no `p8()`. A ninth
    // sub-query needs the same treatment applied one level down (a tuple,
    // or a `#[derive(SystemParam)]` struct), not another entry here.
    mut render_queries: ParamSet<(
        // `VolumetricFog` rides in this tuple rather than as a seventeenth
        // top-level parameter for the reason spelled out above: this function
        // is at Bevy 0.14's 16/16 `SystemParamFunction` ceiling, and going
        // over it does not say so -- it surfaces as a `.chain()` trait-bound
        // error in `RenderPlugin::build`.
        Query<(
            &Camera,
            &Transform,
            Option<&Bloom>,
            Option<&ToneMap>,
            Option<&AmbientOcclusion>,
            Option<&Taa>,
            Option<&bsengine_core::VolumetricFog>,
            Option<&bsengine_core::ScreenSpaceReflections>,
            Option<&bsengine_core::ColorGrading>,
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
        Query<(&Occluder, &Transform, Option<&GlobalTransform>)>,
    )>,
    editor_panels: Option<Res<EditorPanelRegistry>>,
    type_registry: Option<Res<bevy_ecs::reflect::AppTypeRegistry>>,
    // Emitters name a texture by path, like materials do; this is where that
    // path becomes the id the GPU knows. Absent until item 38's cache exists,
    // in which case particles draw against the default white texture.
    texture_cache: Option<Res<crate::texture_cache::TextureCache>>,
    // ONE tuple parameter, deliberately, rather than two plain ones: this
    // function already stood at 15 top-level params and Bevy 0.14 stops
    // implementing `SystemParamFunction` at 16, while the `ParamSet` above
    // is now at its own hard maximum of 8 sub-params. Two more plain
    // parameters would overflow the first limit and a ninth ParamSet entry
    // the second. A tuple of `SystemParam`s is itself a `SystemParam`, so
    // this pays one slot for both. (Overflowing either limit does not say
    // so: it surfaces as a `.chain()` trait-bound error in
    // `RenderPlugin::build`, exactly as the ParamSet comment above records.)
    //
    // The buffer is a `Local`, not a `Resource`, because nothing outside
    // this system reads it -- so it does not belong in the ECS world -- but
    // its 64 KB backing allocation should persist across frames rather than
    // be rebuilt every one.
    //
    // `taa_frame_index` rides along in the same tuple for exactly the reason
    // above: with the tuple counted as one, this function already stands at
    // 16 top-level params, and `bevy_ecs` 0.14 stops implementing
    // `SystemParamFunction` at 16 (`all_tuples!(impl_system_function, 0, 16,
    // F)`). It is a `Local` for the same reason the buffer is -- it is this
    // system's own frame counter, driving the Halton jitter cycle, and no
    // one else reads it.
    //
    // The light-probe volume query joins this tuple for exactly that reason.
    // Both limits above are already at their maximum, so a 17th top-level
    // parameter and a 9th `ParamSet` entry are equally impossible -- the
    // `ParamSet` comment names this same remedy, "the same treatment applied
    // one level down (a tuple ...)". The query is read-only and touches no
    // component the `ParamSet` writes, so it conflicts with nothing.
    (
        occlusion_enabled,
        mut occlusion_buf,
        mut taa_frame_index,
        probe_volumes,
        shadow_settings,
        decal_query,
        reflection_probe_query,
    ): (
        Option<Res<bsengine_core::OcclusionCullingEnabled>>,
        Local<crate::occlusion::OcclusionBuffer>,
        Local<u32>,
        Query<(
            &bsengine_core::LightProbeVolume,
            &Transform,
            Option<&GlobalTransform>,
        )>,
        Option<Res<bsengine_core::ShadowSettings>>,
        // In this tuple rather than the `ParamSet` above for the reason that
        // tuple's own comment gives: the ParamSet is at its hard maximum of 8
        // sub-params and this function is at Bevy's 16 top-level ones, while a
        // tuple of `SystemParam`s counts as one. `probe_volumes` above is a
        // `Query` here for exactly the same reason.
        Query<(&bsengine_core::Decal, &Transform, Option<&GlobalTransform>)>,
        // Same tuple, same reason as the two queries above.
        Query<(
            Entity,
            &bsengine_core::ReflectionProbe,
            &Transform,
            Option<&GlobalTransform>,
        )>,
    ),
) {
    let (Some(mut surface), Some(registry)) = (surface, registry) else {
        return;
    };
    let empty = std::collections::HashMap::new();
    let hud_map = hud_texts.as_deref().map(|h| &h.0).unwrap_or(&empty);
    let (cursor_x, cursor_y) = mouse_state
        .as_deref()
        .map(|ms| (ms.position.0 as f32, ms.position.1 as f32))
        .unwrap_or((0.0, 0.0));
    // The wheel scrolls whatever scroll container sits under the cursor,
    // before the frame is laid out for drawing, so a wheel turned this frame is
    // reflected in this frame rather than the next. `scroll_by` recomputes the
    // layout because the offset it clamps against is a layout result -- the
    // arithmetic is pure and cheap, and sharing a cached layout with the
    // renderer would couple two things that are otherwise independent.
    let wheel = mouse_state
        .as_deref()
        .map(|ms| ms.scroll_delta as f32)
        .unwrap_or(0.0);
    if wheel != 0.0 {
        if let Some(state) = ui_state.as_deref_mut() {
            let (w, h) = (surface.0.width() as f32, surface.0.height() as f32);
            let layout = state.layout(w, h);
            // Negated: a wheel pushed forward scrolls the content up, which is
            // what every one of the reference engines and every browser does.
            state.scroll_by(&layout, cursor_x, cursor_y, 0.0, -wheel * WHEEL_PIXELS);
        }
    }
    let empty_ui = UiState::default();
    let ui = ui_state.as_deref().unwrap_or(&empty_ui);
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

    let (
        mut view_proj,
        mut cam_pos,
        mut cam_proj,
        bloom,
        tone_map,
        ambient_occlusion,
        taa,
        fog,
        ssr,
        color_grading,
    ) = render_queries
        .p0()
        .iter()
        .next()
        .map(|(cam, t, b, tm, ao, taa, fog, ssr, grade)| {
            let proj = cam.projection_matrix();
            (
                proj * t.view_matrix(),
                t.position.0,
                proj,
                b.copied(),
                tm.copied(),
                ao.copied(),
                taa.copied(),
                fog.copied(),
                ssr.copied(),
                grade.cloned(),
            )
        })
        .unwrap_or((
            Mat4::IDENTITY,
            Vec3::ZERO,
            Mat4::IDENTITY,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
        ));

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

    // TAA reprojection inputs, computed *after* the editor override above and
    // never before it: whichever camera won that block is the one this frame
    // rasterizes with, so it must also be the one the next frame reprojects
    // against. Capturing the game camera's matrix here instead would make the
    // editor viewport smear every time the orbit camera moved.
    //
    // `view_proj` itself stays unjittered -- everything downstream of it
    // (occlusion rasterization, frustum culling, and the reprojection matrix
    // below) wants the camera's true matrix. The jitter is a separate offset
    // that `WgpuSurface::render_frame` applies to the rasterization
    // projection alone.
    let unjittered_view_proj = view_proj;
    // Captured before the advance below, and handed to `render_frame` alongside
    // the offset: the froxel fog's depth dither runs on every foggy frame, TAA
    // enabled or not, so it needs the counter itself and not just the
    // TAA-conditional offset derived from it.
    let frame_index = *taa_frame_index;
    let jitter_clip = if taa.map(|t| t.enabled).unwrap_or(false) {
        bsengine_rhi_wgpu::taa_jitter::jitter_clip_offset(
            frame_index,
            surface.0.width(),
            surface.0.height(),
        )
    } else {
        (0.0, 0.0)
    };
    // Advanced every frame, not only the TAA ones, so the cycle never stalls
    // on a repeated offset. `wrapping_add` because this counts frames
    // forever; `jitter_clip_offset` takes it modulo the cycle length anyway.
    *taa_frame_index = taa_frame_index.wrapping_add(1);

    // Rotation-only VP inverse for skybox (no translation → direction-only)
    let sky_vp_inv: Option<Mat4> = if surface.0.has_skybox() {
        render_queries.p0().iter().next().map(|(cam, t, ..)| {
            let proj = cam.projection_matrix();
            let view = t.view_matrix();
            let view_rot = Mat4::from_cols(view.x_axis, view.y_axis, view.z_axis, Vec4::W);
            (proj * view_rot).inverse()
        })
    } else {
        None
    };

    // The baked-probe volume, read in one self-contained block for the same
    // reason the occlusion pre-pass below is one: the whole query is consumed
    // here and nothing downstream holds a borrow of it.
    //
    // Only the first volume counts. The GPU side has one `MAX_PROBES` grid,
    // and picking silently among several would be worse than ignoring the
    // extras -- there is nothing sensible to blend between two boxes.
    //
    // The box is axis-aligned in world space: the entity's translation moves
    // it, but its rotation and scale do not, because `inside_probe_volume` and
    // the trilinear lookup in the scene shader are both written against an
    // AABB. `half_extents` is therefore taken as world units directly, not
    // scaled by the transform.
    // Reflection probes, in entity order so the same set of probes is the
    // same list from frame to frame: the surface re-captures whenever the
    // list differs, and an order shuffled by an unrelated archetype move
    // would re-capture for nothing. Axis-aligned boxes in world units, for
    // the reason the light-probe volume's comment below gives.
    let mut reflection_probes: Vec<(Entity, bsengine_rhi_wgpu::ReflectionProbeParams)> =
        reflection_probe_query
            .iter()
            .map(|(entity, probe, t, gt)| {
                let center = gt
                    .map(|g| g.to_matrix().w_axis.truncate())
                    .unwrap_or(t.position.0);
                (
                    entity,
                    bsengine_rhi_wgpu::ReflectionProbeParams {
                        center,
                        half_extents: *probe.half_extents,
                        box_projection: probe.box_projection,
                        intensity: probe.intensity,
                    },
                )
            })
            .collect();
    reflection_probes.sort_by_key(|(entity, _)| *entity);
    let reflection_probes: Vec<bsengine_rhi_wgpu::ReflectionProbeParams> =
        reflection_probes.into_iter().map(|(_, p)| p).collect();

    let light_probes = probe_volumes.iter().next().map(|(volume, t, gt)| {
        let centre = gt
            .map(|g| g.to_matrix().w_axis.truncate())
            .unwrap_or(t.position.0);
        let half = *volume.half_extents;
        bsengine_rhi_wgpu::ProbeVolumeParams {
            origin: centre - half,
            extent: half * 2.0,
            resolution: volume.clamped_resolution(),
        }
    });

    // Occlusion pre-pass. It has to finish before the draw-call loop below
    // borrows `p1()`: a ParamSet lends exactly one sub-query at a time, so
    // this block's `p7()` borrow must end first -- hence a self-contained
    // block here rather than anything interleaved with the loop.
    //
    // An absent `OcclusionCullingEnabled` means enabled, matching frustum
    // culling, which has always run unconditionally.
    let occlusion_on = occlusion_enabled.as_deref().map(|o| o.0).unwrap_or(true);
    occlusion_buf.clear();
    if occlusion_on {
        for (occ, t, gt) in render_queries.p7().iter() {
            let model = gt.map(|g| g.to_matrix()).unwrap_or_else(|| t.to_matrix());
            crate::occlusion::rasterize_occluder_box(
                &mut occlusion_buf,
                view_proj,
                model,
                *occ.center,
                *occ.half_extents,
            );
        }
    }
    // A plain shared reference for the culling closure below, which runs on
    // several threads: `occlusion_buf` is a `Local` (a smart pointer held by
    // value) and the closure must capture a `&OcclusionBuffer`, which is
    // `Sync`, not the `Local` itself.
    let occlusion_buf = &*occlusion_buf;

    // Per-entity culling in parallel. Each entity's work -- its model
    // matrix, the frustum test, the occlusion-buffer scan, the LOD pick --
    // reads shared, unchanging data and writes only that entity's own
    // `LodLevels`, which each item carries as its own `Mut`, so nothing is
    // shared mutably across threads. Measured (release, scale-level) at
    // about 1.06 ms of the renderer's ECS side for 1,238 entities, most of
    // it the occlusion scan; the reference engines cull in jobs for the same
    // reason (Unity's culling jobs, Unreal's parallel view-visibility pass).
    //
    // Bevy's own `par_iter` would run serially here -- the workspace builds
    // `bevy_ecs` without `multi_threaded`, deliberately: gameplay keeps one
    // thread and one order (measured in #1910) -- so the query is gathered
    // into a Vec and rayon splits that. rayon's `collect` keeps the input
    // order, so the draw list comes out exactly as the serial loop made it.
    enum Culled {
        Drawn(DrawCall),
        Hidden,
        Occluded,
    }
    type DrawCall = (u64, Mat4, Option<u64>, MaterialParams, Option<String>);
    let mut mesh_query = render_queries.p1();
    let candidates: Vec<_> = mesh_query.iter_mut().collect();
    let culled: Vec<Culled> = candidates
        .into_par_iter()
        .map(|(mr, t, gt, mat, vis, cs, mut lod)| {
            if !vis.map(|v| v.is_visible).unwrap_or(true) {
                return Culled::Hidden;
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
                    return Culled::Hidden;
                }
                // Only entities that survived the frustum test get here, so
                // the two culling stages compose instead of duplicating
                // work. The candidate is tested as a cube of side
                // `2 * world_radius` around its bounding sphere -- an
                // over-estimate of the object, deliberately: testing
                // something larger than the object makes it harder to
                // declare occluded, never easier, and a false cull is a
                // visible rendering bug while a missed one only costs a
                // draw call.
                if occlusion_on
                    && crate::occlusion::box_occluded(
                        occlusion_buf,
                        view_proj,
                        center,
                        Vec3::splat(world_radius),
                    )
                {
                    return Culled::Occluded;
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
            Culled::Drawn((
                effective_mesh_id,
                model,
                tex_id,
                mat_params,
                cs.map(|c| c.path.clone()),
            ))
        })
        .collect();
    let occluded_count = culled
        .iter()
        .filter(|c| matches!(c, Culled::Occluded))
        .count() as u32;
    let draw_calls: Vec<DrawCall> = culled
        .into_iter()
        .filter_map(|c| match c {
            Culled::Drawn(d) => Some(d),
            Culled::Hidden | Culled::Occluded => None,
        })
        .collect();

    // The count is the only externally visible evidence that occlusion
    // culling did anything; it is handed to `render_frame` below, which
    // records it in `FrameStats::occluded_count` for the profiler panel and
    // the `get_frame_stats` query tool.
    if occluded_count > 0 {
        tracing::trace!(occluded_count, "occlusion culling skipped entities");
    }

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

    // Fitted to the camera, not to the world origin. `unjittered_view_proj`
    // rather than the jittered matrix: a sub-pixel TAA offset must not shift
    // the shadow map's texel grid, which is snapped precisely to stop it
    // moving.
    //
    // An absent `ShadowSettings` means the defaults, matching how
    // `OcclusionCullingEnabled` is read above: the editor and every test that
    // builds an app directly insert neither.
    let shadow = shadow_settings.as_deref().copied().unwrap_or_default();
    let cascades = bsengine_rhi_wgpu::shadow::DirectionalCascades::new(
        light.direction,
        unjittered_view_proj,
        shadow.distance,
        shadow.cascades,
        shadow.blend,
    );
    let tex_reg_ref = tex_registry.as_deref();

    // Each UI image's path resolved to a GPU id, for the paths this frame's
    // widgets actually name. Resolved here rather than in the renderer because
    // the path-to-id map is `TextureCache`, which lives in this crate -- the
    // same reason mesh draw calls carry an id rather than a path.
    let ui_image_ids: std::collections::HashMap<String, u64> = ui
        .widgets
        .iter()
        .filter_map(|w| match w {
            bsengine_core::UiWidget::Image { texture_path, .. } => {
                let id = texture_cache.as_deref()?.id_for(texture_path)?;
                Some((texture_path.clone(), id))
            }
            _ => None,
        })
        .collect();

    // Decals: the box's world matrix, and its texture path resolved to the id
    // the GPU knows. Resolved here rather than in the renderer because the
    // path-to-id map is `TextureCache`, which lives in this crate -- the same
    // reason mesh draw calls carry an id rather than a path.
    let decals: Vec<bsengine_rhi_wgpu::decals::DecalDraw> = decal_query
        .iter()
        .map(|(decal, transform, global)| {
            // The global transform when the entity has one, so a decal
            // parented to a moving object goes with it.
            let placed = global
                .map(|g| g.to_matrix())
                .unwrap_or_else(|| transform.to_matrix());
            bsengine_rhi_wgpu::decals::DecalDraw {
                // The authored size is the box's full extent, and the cube the
                // renderer draws is a unit box, so the size goes in as scale.
                model: placed * Mat4::from_scale(decal.clamped_size()),
                opacity: decal.clamped_opacity(),
                normal_fade: decal.normal_fade.clamp(0.0, 1.0),
                texture: texture_cache
                    .as_deref()
                    .and_then(|c| c.id_for(&decal.texture_path)),
                // Empty path resolves to `None`, and the renderer binds a flat
                // normal for that -- which leaves the surface's own normal
                // exactly as it was.
                normal_texture: texture_cache
                    .as_deref()
                    .and_then(|c| c.id_for(&decal.normal_map_path)),
            }
        })
        .collect();

    match surface.0.render_frame(
        view_proj,
        cam_pos,
        &cascades,
        sky_vp_inv,
        &draw_calls,
        &terrain_draw_calls,
        &decals,
        occluded_count,
        &registry,
        light,
        tex_reg_ref,
        hud_map,
        ui,
        &ui_image_ids,
        cursor_x,
        cursor_y,
        left_just_pressed,
        left_just_released,
        cam_proj,
        bloom,
        tone_map,
        ambient_occlusion,
        ssr,
        inspector.as_deref_mut(),
        &key_events_this_frame,
        ctrl_held,
        shift_held,
        alt_held,
        editor_panels.as_deref(),
        type_registry.as_deref(),
        time.as_deref().map(|t| t.elapsed_seconds).unwrap_or(0.0),
        &particle_batches,
        taa,
        jitter_clip,
        frame_index,
        unjittered_view_proj,
        light_probes,
        &reflection_probes,
        fog,
        color_grading,
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
        app.register_type::<bsengine_core::Decal>();
        app.register_type::<bsengine_core::ScreenSpaceReflections>();
        app.init_asset::<crate::shader_asset::ShaderSource>()
            .register_asset_loader(crate::shader_asset::ShaderSourceLoader)
            .init_resource::<UiState>()
            .init_resource::<PendingShaders>()
            .init_resource::<ShownSkybox>()
            .init_resource::<ShownColorLut>()
            .init_resource::<crate::texture_cache::TextureCache>()
            .add_event::<WindowResized>()
            .add_event::<KeyInput>()
            .add_systems(Startup, bsengine_rhi_wgpu::panels::register_mixer_panel)
            .add_systems(
                Update,
                (
                    update_camera_aspect,
                    crate::texture_cache::resolve_texture_paths,
                    crate::texture_cache::reupload_modified_textures,
                    crate::texture_cache::stream_textures,
                ),
            )
            .add_systems(
                PostUpdate,
                (
                    bsengine_core::propagate_global_transforms,
                    compile_pending_shaders,
                    rebuild_modified_shaders,
                    sync_skybox,
                    sync_color_lut,
                    render_frame,
                )
                    .chain(),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CompileStatus, PendingShader, PendingShaders, RenderPlugin, ShownColorLut, ShownSkybox,
    };
    use crate::components::{LodLevels, MeshRenderer, Occluder};
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
            .position(|n| n.contains("sync_skybox"))
            .unwrap_or_else(|| panic!("sync_skybox not found in PostUpdate: {names:?}"));
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
            "sync_skybox (index {skybox_idx}) must run before render_frame \
             (index {render_idx}) so a skybox copied this frame is available to its \
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

    /// A real offscreen surface on the device every test in the process
    /// shares (`WgpuSurface::offscreen_for_testing`), and the registries on
    /// it, so the skybox system runs for real: the cache uploads, the surface
    /// copies, and `has_skybox`/`loaded_skybox_path` say what happened. The
    /// tests that used to run without one could only watch a request's
    /// bookkeeping, and that bookkeeping is gone now that the cache does the
    /// requesting.
    fn with_surface(app: &mut bevy_app::App) {
        let surface = bsengine_rhi_wgpu::surface::WgpuSurface::offscreen_for_testing(16, 16)
            .expect("these tests need an adapter; a skip here would look like a pass");
        app.insert_resource(GpuMeshRegistry::new(surface.device_arc()));
        app.insert_resource(bsengine_rhi_wgpu::GpuTextureRegistry::new(
            surface.device_arc(),
            surface.queue_arc(),
        ));
        app.insert_resource(bsengine_rhi_wgpu::WgpuSurfaceResource(surface));
    }

    /// The asset and render plugins on a real surface, plus a 1x1 PNG at a
    /// fresh path under the temp directory named for `test`.
    fn skybox_app(test: &str) -> (bevy_app::App, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "bsengine_test_skybox_{test}_{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("sky.png");
        std::fs::write(&png, MINIMAL_PNG_1X1).unwrap();
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        with_surface(&mut app);
        (app, png)
    }

    fn surface_of(app: &bevy_app::App) -> &bsengine_rhi_wgpu::surface::WgpuSurface {
        &app.world()
            .resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
            .0
    }

    fn shown_generation(app: &bevy_app::App) -> Option<u64> {
        app.world().resource::<ShownSkybox>().generation()
    }

    fn uploaded(app: &bevy_app::App) -> usize {
        app.world()
            .resource::<crate::texture_cache::TextureCache>()
            .uploaded_count()
    }

    /// Runs frames until `done`, at most 200; whether it got there.
    fn run_until(app: &mut bevy_app::App, done: impl Fn(&bevy_app::App) -> bool) -> bool {
        for _ in 0..200 {
            app.update();
            if done(app) {
                return true;
            }
        }
        false
    }

    /// A camera's `ColorGrading::lut` reaches the post pass through the
    /// texture cache, as a skybox does: a strip is bound (its slice count
    /// read back from the post pass itself), an image that is not a strip is
    /// refused and *unbinds* -- the old LUT must not stay on screen under the
    /// new one's name -- and clearing the path unbinds too.
    #[test]
    fn a_cameras_lut_is_loaded_through_the_texture_cache() {
        let dir =
            std::env::temp_dir().join(format!("bsengine_test_colour_lut_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // A 2-slice strip (4 x 2) and an image that is not one (3 x 2).
        let strip = dir.join("strip.png");
        let not_a_strip = dir.join("not_a_strip.png");
        image::RgbaImage::from_pixel(4, 2, image::Rgba([10, 20, 30, 255]))
            .save(&strip)
            .unwrap();
        image::RgbaImage::from_pixel(3, 2, image::Rgba([10, 20, 30, 255]))
            .save(&not_a_strip)
            .unwrap();

        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::windowed());
        app.add_plugins(RenderPlugin);
        with_surface(&mut app);
        let grading = |path: &std::path::Path| bsengine_core::ColorGrading {
            lut: path.to_string_lossy().to_string(),
            ..Default::default()
        };
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_position(Vec3::new(0.0, 0.0, 10.0)),
                grading(&strip),
            ))
            .id();
        let lut_size = |a: &bevy_app::App| surface_of(a).color_lut_size();

        assert!(
            run_until(&mut app, |a| lut_size(a) == 2),
            "the 2-slice strip must be bound"
        );

        *app.world_mut()
            .get_mut::<bsengine_core::ColorGrading>(camera)
            .unwrap() = grading(&not_a_strip);
        assert!(
            run_until(&mut app, |a| lut_size(a) == 0),
            "an image that is not a strip is refused and the old LUT unbound"
        );
        assert_eq!(
            app.world().resource::<ShownColorLut>().warned.as_deref(),
            Some(not_a_strip.to_string_lossy().as_ref()),
            "and the refusal is reported"
        );

        *app.world_mut()
            .get_mut::<bsengine_core::ColorGrading>(camera)
            .unwrap() = grading(&strip);
        assert!(run_until(&mut app, |a| lut_size(a) == 2), "bound again");
        app.world_mut()
            .get_mut::<bsengine_core::ColorGrading>(camera)
            .unwrap()
            .lut
            .clear();
        app.update();
        assert_eq!(lut_size(&app), 0, "an empty path unbinds the LUT");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The whole path, on a real surface: the image goes through the texture
    /// cache (one upload, the cache's), the surface copies the registry's
    /// object and records the path, the IBL maps are convolved from it, and
    /// what is shown is the generation the registry holds now.
    #[test]
    fn a_skybox_is_shown_out_of_the_texture_cache() {
        let (mut app, png) = skybox_app("shown");
        let path = png.to_string_lossy().to_string();
        app.insert_resource(bsengine_core::SkyboxPath(Some(path.clone())));

        assert!(
            run_until(&mut app, |a| surface_of(a).has_skybox()),
            "the skybox must appear"
        );
        let surface = surface_of(&app);
        assert_eq!(surface.loaded_skybox_path(), Some(path.as_str()));
        assert!(
            surface.has_ibl(),
            "the environment maps are convolved from it"
        );
        assert_eq!(
            uploaded(&app),
            1,
            "one upload, the cache's -- the skybox no longer uploads for itself"
        );
        let id = app
            .world()
            .resource::<crate::texture_cache::TextureCache>()
            .id_for(&path)
            .expect("the cache holds the skybox image");
        let registry = app
            .world()
            .resource::<bsengine_rhi_wgpu::GpuTextureRegistry>();
        assert_eq!(
            shown_generation(&app),
            registry.generation(id),
            "copied from the object the registry holds now"
        );
        let _ = std::fs::remove_file(&png);
    }

    // The skybox half of the same precondition
    // `a_compiled_shader_keeps_its_handle_so_a_reload_can_reach_it` pins for
    // shaders: `AssetEvent::Modified` only fires while a strong handle to the
    // asset still exists, so the moment the last one drops,
    // `Assets::track_assets` frees the image in PreUpdate and
    // `AssetServer::reload` on that path is a silent no-op. The cache retains
    // the handle (`TextureCache::asset_handle`) -- and the two assertions
    // after the load, not the handle's existence, are what prove the
    // retention is real: a `clone_weak` handle exists while still letting
    // the image be freed.
    //
    // With a real surface this goes one step further than the shader test:
    // the reload must reach the *screen*. The cache re-uploads the modified
    // asset under the same id, the registry's generation changes, and the
    // surface copies again -- without a second upload.
    #[test]
    fn an_uploaded_skybox_keeps_its_handle_so_a_reload_reaches_the_screen() {
        use bevy_asset::{AssetEvent, AssetServer, Assets};
        use bevy_ecs::event::{Events, ManualEventReader};
        use bsengine_asset::TextureAsset;

        let (mut app, png) = skybox_app("reload");
        let path = png.to_string_lossy().to_string();
        app.insert_resource(bsengine_core::SkyboxPath(Some(path.clone())));
        assert!(
            run_until(&mut app, |a| surface_of(a).has_skybox()),
            "the skybox must appear before it can be reloaded"
        );
        let first_generation = shown_generation(&app).expect("shown");

        let asset_id = app
            .world()
            .resource::<crate::texture_cache::TextureCache>()
            .asset_handle(&path)
            .expect("the cache retains the handle it loaded the skybox through")
            .id();

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

        app.world().resource::<AssetServer>().reload(path.clone());
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

        assert!(
            run_until(&mut app, |a| shown_generation(a) != Some(first_generation)),
            "the reload must reach the screen: the registry rebuilt its object and the \
             surface must have copied the new one"
        );
        assert_eq!(
            surface_of(&app).loaded_skybox_path(),
            Some(path.as_str()),
            "and it is still this path that is shown"
        );
        assert_eq!(
            uploaded(&app),
            1,
            "a reload replaces the object under its id; it is not a second upload"
        );

        let _ = std::fs::remove_file(&png);
    }

    // The skybox used to be the one consumer that never went through
    // `bsengine_asset::load` -- that dispatcher wants a `sync_loader` closure
    // for its `Sync` arm and this codebase has no synchronous texture loader --
    // so it was the one that could silently stay invisible to `AssetStatuses`.
    // It now loads through the texture cache, whose `AssetSlot::requesting`
    // records the request; this is what says the routing survived the move.
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
        // The cache only requests once it has a registry to upload into.
        with_surface(&mut app);
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
    // reason: the cache requests the texture once and polls the handle it
    // kept. Re-requesting the path each frame would reset the failed load to
    // `Loading` and restart it, so the give-up state would never be reached
    // and this would spin forever.
    #[test]
    fn missing_skybox_is_given_up_on_instead_of_retried_forever() {
        let (mut app, png) = skybox_app("missing");
        let missing = "definitely/not/a/real/sky.png";
        app.insert_resource(bsengine_core::SkyboxPath(Some(missing.to_string())));

        let gave_up = |app: &bsengine_app::App| {
            app.world()
                .resource::<crate::texture_cache::TextureCache>()
                .gave_up(missing)
        };
        assert!(
            run_until(&mut app, gave_up),
            "an unloadable skybox path must end up given up on"
        );

        // Then *stays* given up on, on every frame rather than merely on the
        // one this happens to sample. A loop that re-requests the failed path
        // also passes through `GaveUp` repeatedly, so a single reading taken
        // after N frames cannot tell a give-up from an infinite retry -- which
        // is the entire property this test is named for.
        for frame in 0..60 {
            app.update();
            assert!(
                gave_up(&app),
                "a given-up skybox left GaveUp on frame {frame}, which means \
                 something re-requested the failed path"
            );
        }
        assert!(
            !surface_of(&app).has_skybox(),
            "and nothing was shown in its place"
        );
        let _ = std::fs::remove_file(&png);
    }

    // Changing `SkyboxPath` must put the new sky on screen and only that one,
    // and switching back must not upload again: the cache still holds the
    // first image, and the surface copies it a second time.
    //
    // The old version of this test switched *mid-load* and asserted on which
    // request was in flight. With the cache requesting, a switch mid-load
    // leaves both loads running -- harmless, the first is one cached image --
    // and what matters is only ever what the surface shows, which needs a
    // real surface to say.
    #[test]
    fn changing_the_skybox_path_shows_the_new_sky_and_keeps_the_old_upload() {
        let (mut app, first) = skybox_app("switch");
        let second = first.with_file_name("sky2.png");
        std::fs::write(&second, MINIMAL_PNG_1X1).unwrap();
        let (first, second) = (
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        );

        app.insert_resource(bsengine_core::SkyboxPath(Some(first.clone())));
        assert!(
            run_until(&mut app, |a| surface_of(a).loaded_skybox_path()
                == Some(first.as_str())),
            "precondition: the first sky is shown"
        );

        app.world_mut()
            .resource_mut::<bsengine_core::SkyboxPath>()
            .0 = Some(second.clone());
        assert!(
            run_until(&mut app, |a| surface_of(a).loaded_skybox_path()
                == Some(second.as_str())),
            "the second sky must replace the first"
        );
        assert_eq!(uploaded(&app), 2, "two images, two uploads");

        app.world_mut()
            .resource_mut::<bsengine_core::SkyboxPath>()
            .0 = Some(first.clone());
        assert!(
            run_until(&mut app, |a| surface_of(a).loaded_skybox_path()
                == Some(first.as_str())),
            "switching back must show the first sky again"
        );
        assert_eq!(
            uploaded(&app),
            2,
            "switching back copies the image the cache still holds; it does not upload it again"
        );
        let _ = std::fs::remove_file(&first);
        let _ = std::fs::remove_file(&second);
    }

    // `SkyboxPath.0 = None` must take the sky off the screen -- and its IBL
    // maps with it -- and putting the path back must show it again without
    // a second upload.
    #[test]
    fn turning_the_skybox_off_clears_it_and_on_again_needs_no_new_upload() {
        let (mut app, png) = skybox_app("off");
        let path = png.to_string_lossy().to_string();
        app.insert_resource(bsengine_core::SkyboxPath(Some(path.clone())));
        assert!(
            run_until(&mut app, |a| surface_of(a).has_skybox()),
            "precondition: the sky is shown"
        );

        app.world_mut()
            .resource_mut::<bsengine_core::SkyboxPath>()
            .0 = None;
        app.update();
        let surface = surface_of(&app);
        assert!(!surface.has_skybox(), "off must clear the sky");
        assert!(!surface.has_ibl(), "and the maps convolved from it");
        assert_eq!(shown_generation(&app), None);

        app.world_mut()
            .resource_mut::<bsengine_core::SkyboxPath>()
            .0 = Some(path.clone());
        assert!(
            run_until(&mut app, |a| surface_of(a).has_skybox()),
            "on again must show it again"
        );
        assert_eq!(
            uploaded(&app),
            1,
            "the cache still had the image; showing it again is a copy, not an upload"
        );
        let _ = std::fs::remove_file(&png);
    }

    // The registry rebuilds the object behind an id on a hot reload and on
    // every streamed level that arrives, and a bind group over the old object
    // would keep drawing it. The generation is what the skybox watches; here
    // the object is rebuilt directly, the way the cache's reload does it, and
    // the sky must be copied again from the new one.
    #[test]
    fn a_rebuilt_registry_object_is_copied_onto_the_skybox_again() {
        let (mut app, png) = skybox_app("rebuilt");
        let path = png.to_string_lossy().to_string();
        app.insert_resource(bsengine_core::SkyboxPath(Some(path.clone())));
        assert!(
            run_until(&mut app, |a| surface_of(a).has_skybox()),
            "precondition: the sky is shown"
        );
        let before = shown_generation(&app).expect("shown");
        let id = app
            .world()
            .resource::<crate::texture_cache::TextureCache>()
            .id_for(&path)
            .expect("cached");

        {
            let mut registry = app
                .world_mut()
                .resource_mut::<bsengine_rhi_wgpu::GpuTextureRegistry>();
            let settings = registry.get_settings(id).expect("loaded");
            assert!(
                registry.replace_with(id, 1, 1, &[10, 20, 30, 255], settings),
                "premise: the object behind the id was rebuilt"
            );
            assert_ne!(
                registry.generation(id),
                Some(before),
                "premise: a rebuild changes the generation"
            );
        }
        app.update();

        let after = shown_generation(&app).expect("still shown");
        assert_ne!(after, before, "the sky must have been copied again");
        assert_eq!(
            Some(after),
            app.world()
                .resource::<bsengine_rhi_wgpu::GpuTextureRegistry>()
                .generation(id),
            "from the object the registry holds now"
        );
        let _ = std::fs::remove_file(&png);
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

    /// `ReflectionProbe` entities reach the surface: each one's box, at its
    /// *world* position, in entity order, and a change to one re-captures.
    /// Read back from what the surface actually captured
    /// (`reflection_probes_captured`), not from a copy of the conversion.
    ///
    /// The second probe is a child of an entity at x = 10, so its world
    /// position disagrees with its local `Transform`: the capture must be
    /// taken where the probe *is*. (A root entity's `GlobalTransform` is
    /// recomputed from its `Transform` each frame, so a hand-set one on a
    /// root would not have made the two differ.) Not `fast_render`,
    /// which never captures -- that would make the empty list the answer to
    /// every question here.
    #[test]
    fn reflection_probe_entities_are_captured_at_their_world_position() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, false));
        app.add_plugins(RenderPlugin);
        app.update();
        app.world_mut().spawn((
            Camera::default(),
            Transform::from_position(Vec3::new(0.0, 0.0, 10.0)),
        ));
        let first = app
            .world_mut()
            .spawn((
                bsengine_core::ReflectionProbe {
                    half_extents: Vec3::new(1.0, 2.0, 3.0).into(),
                    box_projection: true,
                    intensity: 0.5,
                },
                Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
            ))
            .id();
        let parent = app
            .world_mut()
            .spawn(Transform::from_position(Vec3::new(10.0, 0.0, 0.0)))
            .id();
        app.world_mut().spawn((
            bsengine_core::ReflectionProbe::default(),
            Transform::from_position(Vec3::new(1.0, 1.0, 1.0)),
            GlobalTransform::default(),
            Parent(parent),
        ));
        app.update();

        let captured = |app: &bevy_app::App| {
            app.world()
                .resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
                .0
                .reflection_probes_captured()
                .to_vec()
        };
        let probes = captured(&app);
        assert_eq!(probes.len(), 2, "both probes captured: {probes:?}");
        assert_eq!(probes[0].center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(probes[0].half_extents, Vec3::new(1.0, 2.0, 3.0));
        assert!(probes[0].box_projection);
        assert_eq!(probes[0].intensity, 0.5);
        assert_eq!(
            probes[1].center,
            Vec3::new(11.0, 1.0, 1.0),
            "the child probe is captured at its world position, not its local one"
        );

        app.world_mut()
            .get_mut::<bsengine_core::ReflectionProbe>(first)
            .unwrap()
            .intensity = 2.0;
        app.update();
        assert_eq!(
            captured(&app)[0].intensity,
            2.0,
            "a changed probe is re-captured with its new settings"
        );

        app.world_mut().despawn(first);
        app.update();
        assert_eq!(
            captured(&app).len(),
            1,
            "a removed probe stops being captured, rather than lingering"
        );
    }

    /// A camera's `ColorGrading` reaches the renderer, and taking it off
    /// takes it away again -- absent has to mean "no grade", not "the last
    /// grade the renderer saw".
    #[test]
    fn a_cameras_colour_grading_reaches_the_renderer() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, false));
        app.add_plugins(RenderPlugin);
        app.update();
        let grade = bsengine_core::ColorGrading {
            enabled: true,
            contrast: 1.5,
            saturation: 0.25,
            color_filter: Vec3::new(1.0, 0.5, 0.25).into(),
            ..Default::default()
        };
        let camera = app
            .world_mut()
            .spawn((
                Camera::default(),
                Transform::from_position(Vec3::new(0.0, 0.0, 10.0)),
                grade.clone(),
            ))
            .id();
        app.update();
        let seen = |app: &bevy_app::App| {
            app.world()
                .resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
                .0
                .last_color_grading()
        };
        assert_eq!(seen(&app), Some(grade));

        app.world_mut()
            .entity_mut(camera)
            .remove::<bsengine_core::ColorGrading>();
        app.update();
        assert_eq!(seen(&app), None, "removing the component removes the grade");
    }

    /// The regression test that matters: an entity beside a large occluder
    /// must survive into the draw-call list while one directly behind it
    /// does not. A false cull is a visible rendering bug, so this asserts
    /// the safe direction explicitly rather than only testing that culling
    /// happens at all.
    ///
    /// **How it observes a cull.** `render_frame` builds `draw_calls` in a
    /// local `Vec` and hands it straight to the GPU, so draw-call
    /// membership is not readable from the world afterwards -- but LOD
    /// selection *is*. `LodLevels::current_index` is written inside the
    /// same `filter_map` closure, strictly *after* the frustum and
    /// occlusion `return None`s, and nothing else in the engine writes it.
    /// So a `LodLevels` whose `current_index` is still `None` after a frame
    /// in which its distance demands a level change is proof the closure
    /// bailed out before reaching the LOD block -- i.e. that this entity
    /// contributed no draw call. This is the same mechanism
    /// `lod_current_index_updates_based_on_camera_distance` above relies
    /// on, read in the other direction.
    ///
    /// The first frame runs with `OcclusionCullingEnabled(false)` as a
    /// control: it proves both entities are otherwise perfectly drawable
    /// (registered bounds, inside the frustum, visible), so the `None` seen
    /// in the second frame can only come from the occlusion test. Without
    /// that control a frustum-culled or bounds-less entity would produce
    /// the same `None` and the test would pass for the wrong reason.
    #[test]
    fn an_entity_beside_an_occluder_is_not_culled_while_one_behind_it_is() {
        let mut app = new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        // Offscreen, not windowed: a windowed plugin never acquires a
        // `WindowHandle` in a test, so `GpuMeshRegistry` never exists,
        // `render_frame` takes its early return, and this test would prove
        // nothing at all.
        app.add_plugins(WgpuRHIPlugin::offscreen(64, 64, true));
        app.add_plugins(RenderPlugin);
        app.update();

        // A real registered mesh, so `registry.get_bounds` returns a real
        // bounding sphere -- the occlusion test lives inside the `if let`
        // that unwraps it, so an unregistered mesh id would skip culling
        // entirely.
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

        // Camera at +Z looking down -Z (the identity-rotation convention of
        // `Transform::view_matrix`), default 60 degree vertical FOV.
        app.world_mut().spawn((
            Camera::default(),
            Transform::from_position(Vec3::new(0.0, 0.0, 20.0)),
        ));

        // A 16x16 wall standing at z = 0, half a unit thick: from the
        // camera it covers roughly the middle 40% of the screen width and
        // 71% of its height.
        app.world_mut().spawn((
            Transform::from_position(Vec3::ZERO),
            Occluder {
                center: Vec3::ZERO.into(),
                half_extents: Vec3::new(8.0, 8.0, 0.5).into(),
            },
        ));

        // Twenty units behind the wall, and offset off the wall's
        // screen-space diagonal: the box rasterizer splits each face into
        // two triangles and leaves the seam between them unwritten (it is
        // inner-conservative), so an object sitting exactly on the
        // projected diagonal would find an uncovered pixel and correctly
        // report itself un-occluded.
        let hidden = spawn_lod_candidate(&mut app, mesh_id, Vec3::new(-3.0, 3.0, -20.0));
        // Same depth, far enough sideways that the camera ray to it misses
        // the wall entirely -- still comfortably inside the frustum and
        // inside the occlusion buffer, so it is genuinely tested against
        // the buffer and genuinely found visible.
        let beside = spawn_lod_candidate(&mut app, mesh_id, Vec3::new(25.0, 0.0, -20.0));

        // --- Control frame: culling off, so both must be drawn. ---
        app.world_mut()
            .insert_resource(bsengine_core::OcclusionCullingEnabled(false));
        app.update();
        assert_eq!(
            lod_index(&app, hidden),
            Some(0),
            "control frame with occlusion culling disabled: the hidden \
             entity must still be drawn, so any later cull is attributable \
             to occlusion alone"
        );
        assert_eq!(
            lod_index(&app, beside),
            Some(0),
            "control frame with occlusion culling disabled: the beside \
             entity must be drawn"
        );

        // --- Real frame: culling on. ---
        for e in [hidden, beside] {
            app.world_mut()
                .get_mut::<LodLevels>(e)
                .expect("candidate still carries its LodLevels")
                .current_index = None;
        }
        // Two more that are not drawn for other reasons -- behind the camera
        // (outside the frustum) and hidden -- so the count below has to tell
        // "occluded" apart from "not drawn": a count of everything culled
        // would read 3.
        let behind_camera = spawn_lod_candidate(&mut app, mesh_id, Vec3::new(0.0, 0.0, 60.0));
        let invisible = spawn_lod_candidate(&mut app, mesh_id, Vec3::new(-25.0, 0.0, -20.0));
        app.world_mut()
            .entity_mut(invisible)
            .insert(bsengine_core::Visible { is_visible: false });
        app.world_mut()
            .insert_resource(bsengine_core::OcclusionCullingEnabled(true));
        app.update();

        assert_eq!(
            lod_index(&app, hidden),
            None,
            "an entity directly behind a 16x16 occluder wall must be culled \
             -- its LOD level was updated, so it reached the draw-call body"
        );
        assert_eq!(
            lod_index(&app, beside),
            Some(0),
            "an entity beside the occluder is visible and must NOT be \
             culled -- a false cull is a visible rendering bug"
        );
        assert_eq!(
            (lod_index(&app, behind_camera), lod_index(&app, invisible)),
            (None, None),
            "premise: the entity behind the camera and the hidden one are not drawn"
        );
        let stats = app
            .world()
            .resource::<bsengine_rhi_wgpu::WgpuSurfaceResource>()
            .0
            .latest_frame_stats()
            .expect("a frame rendered");
        assert_eq!(
            stats.occluded_count, 1,
            "exactly one entity was occluded; the frustum-culled and the hidden one are \
             not occlusion culls"
        );
    }

    /// Spawns a culling candidate whose `LodLevels` doubles as a
    /// draw-call-membership probe: at any distance past 11 units the LOD
    /// selector must move it off LOD0, so `current_index == Some(0)` means
    /// "this entity reached the draw-call body" and `None` means "it was
    /// culled before that".
    fn spawn_lod_candidate(
        app: &mut bevy_app::App,
        mesh_id: u64,
        position: Vec3,
    ) -> bevy_ecs::entity::Entity {
        app.world_mut()
            .spawn((
                MeshRenderer { mesh_id },
                Transform::from_position(position),
                LodLevels {
                    // Never drawn -- this test only reads `current_index`.
                    mesh_ids: vec![mesh_id + 100],
                    switch_distances: vec![10.0],
                    hysteresis_band: 2.0,
                    current_index: None,
                },
            ))
            .id()
    }

    fn lod_index(app: &bevy_app::App, entity: bevy_ecs::entity::Entity) -> Option<usize> {
        app.world()
            .get::<LodLevels>(entity)
            .expect("candidate still carries its LodLevels")
            .current_index
    }
}
