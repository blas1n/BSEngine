//! Terrain brush tool: picking (screen -> world ray -> terrain surface
//! point) and applying height/paint edits. All the "real work" lives here
//! rather than in the editor UI crate (`bsengine-rhi-wgpu`) because this is
//! the only crate with `PhysicsWorld`, `GpuMeshRegistry`, `GpuTextureRegistry`,
//! and `InspectorState` all in reach at once -- the same reason
//! `generate_terrain_chunks` lives in this crate rather than in
//! `bsengine-editor` or `bsengine-rhi-wgpu`.

use crate::terrain::{Terrain, TerrainChunkOf, TerrainChunksGenerated};
use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::{Local, Resource};
use bsengine_core::{InspectorState, TerrainBrushKind, TerrainBrushStroke, Transform};
use bsengine_ecs::{Entity, IntoSystemConfigs, Query, Res, ResMut, With};
use bsengine_input::MouseState;
use bsengine_physics::{Collider, ColliderShape, PhysicsWorld};
use bsengine_render::components::TerrainSplat;
use bsengine_render::MeshRenderer;
use bsengine_rhi_wgpu::{GpuMeshRegistry, GpuQueueResource, GpuTextureRegistry};
use glam::{Mat4, Vec3};
use tracing::warn;

/// Unprojects a screen-space point into a world-space ray, given the
/// camera's combined view-projection matrix and its world position.
/// Standard technique: unproject the near and far NDC points through the
/// inverse view-projection matrix, then the ray direction is far - near.
pub fn screen_to_world_ray(
    view_proj: Mat4,
    cam_pos: Vec3,
    screen_pos: (f32, f32),
    viewport_pos: (f32, f32),
    viewport_size: (f32, f32),
) -> (Vec3, Vec3) {
    let ndc_x = ((screen_pos.0 - viewport_pos.0) / viewport_size.0) * 2.0 - 1.0;
    let ndc_y = 1.0 - ((screen_pos.1 - viewport_pos.1) / viewport_size.1) * 2.0;
    let inv_vp = view_proj.inverse();
    let near = inv_vp * glam::Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
    let far = inv_vp * glam::Vec4::new(ndc_x, ndc_y, 1.0, 1.0);
    let near_world = near.truncate() / near.w;
    let far_world = far.truncate() / far.w;
    let dir = (far_world - near_world).normalize();
    (cam_pos, dir)
}

/// Every frame, if the editor is in terrain-brush mode and the cursor is
/// over the viewport, raycasts from the camera through the cursor and
/// writes the hit (if any, and if it landed on a `TerrainChunkOf` entity)
/// into `InspectorState::terrain_pick`.
///
/// The raw cursor position comes from `bsengine_input::MouseState` (the
/// same resource `bsengine-render`'s `render_frame` reads for its own
/// cursor position, via `ms.position`) rather than from any field on
/// `InspectorState`, which does not track raw screen coordinates.
fn pick_terrain_under_cursor(
    physics: Res<PhysicsWorld>,
    chunk_query: Query<&crate::terrain::TerrainChunkOf>,
    mut inspector: Option<ResMut<InspectorState>>,
    mouse_state: Option<Res<MouseState>>,
) {
    let Some(insp) = inspector.as_mut() else {
        return;
    };
    if !insp.terrain_brush_active || !insp.viewport_contains_cursor {
        insp.terrain_pick = None;
        return;
    }
    let Some(view_proj) = insp.editor_view_proj else {
        insp.terrain_pick = None;
        return;
    };
    let Some(mouse_state) = mouse_state.as_deref() else {
        insp.terrain_pick = None;
        return;
    };
    let cam_pos = Vec3::from(insp.editor_cam_pos);
    let cursor = (mouse_state.position.0 as f32, mouse_state.position.1 as f32);
    let (origin, dir) = screen_to_world_ray(
        Mat4::from_cols_array_2d(&view_proj),
        cam_pos,
        cursor,
        (insp.viewport_pos[0], insp.viewport_pos[1]),
        (insp.viewport_size[0], insp.viewport_size[1]),
    );
    let hit = physics.cast_ray(origin, dir, 10_000.0);
    insp.terrain_pick = hit.and_then(|h| {
        let chunk_entity = h.entity?;
        let owner = chunk_query.get(chunk_entity).ok()?;
        Some((owner.0.index() as u64, h.point.to_array()))
    });
}

/// In-memory, full-resolution copy of the heightmap/splatmap grid an
/// in-progress brush stroke is editing, if any.
///
/// **Design decision (where does edited grid data live between "chunk
/// spawned" and "brush stroke commits" -- see this task's own design
/// question):** a `Resource`, populated fresh from disk the first preview
/// frame that actually touches a `Terrain`, mutated in place by every later
/// preview frame, and cleared back to `TerrainBrushEditState::default()`
/// once `commit_terrain_brush_stroke` finishes committing (or gives up).
///
/// This was chosen over the alternative the task sketch also names -- a
/// permanent full-resolution copy `generate_terrain_chunks` (terrain.rs)
/// populates once at chunk-spawn time and keeps alive on the `Terrain`
/// entity forever. That alternative would cost *every* `Terrain` entity,
/// including every shipped game that never opens the editor, a second full
/// copy of its heightmap/splatmap in memory, permanently, for a feature
/// only the editor ever uses. A stroke-scoped resource only exists while
/// someone is actively dragging the brush, and `commit_terrain_brush_stroke`
/// already has to hit disk at the *end* of every stroke anyway -- re-reading
/// from disk at the *start* of one costs nothing new in kind, only in
/// frequency (once per stroke, not once ever).
///
/// A `Resource` rather than a per-`Terrain` `Component`, because
/// `InspectorState::terrain_brush_stroke` is a single `Option`, not one per
/// entity -- at most one stroke is ever in flight at a time, so there is
/// nothing to key by entity. A resource also sidesteps a real problem a
/// component would have: populating it via `Commands::insert` on first
/// touch would not be readable until the *next* frame (`Commands` are
/// deferred to the next sync point), so the very first preview frame of
/// every drag would silently do nothing. A `ResMut<TerrainBrushEditState>`
/// can be populated and read within the same system call, the same frame a
/// drag starts.
///
/// Edits are kept at full heightmap/splatmap resolution, not per-chunk,
/// because `terrain_chunking::generate_chunks_with_splatmap`'s boundary
/// row/column is shared between neighboring chunks (see its own doc
/// comment) -- a brush stroke spanning a chunk boundary must edit that one
/// shared source of truth once, not two independent per-chunk copies that
/// could disagree at the seam.
#[derive(Resource, Default)]
struct TerrainBrushEditState {
    /// Which `Terrain` entity (by index, matching
    /// `TerrainBrushStroke::terrain_entity_id`) this edit belongs to. Lets a
    /// stroke that jumps to a different terrain (or a stale leftover from a
    /// previous stroke that somehow wasn't cleared) be detected and reset
    /// rather than silently misapplied to the wrong terrain.
    terrain_entity_id: Option<u64>,
    /// Full-resolution raw height samples (row-major, same shape as
    /// `HeightmapAsset::data`/the PNG at `Terrain::heightmap_path`). `Some`
    /// once a `Height` stroke has actually touched this terrain this drag.
    heights: Option<Vec<u16>>,
    /// Full-resolution RGBA8 splat weights (row-major, same shape as a
    /// decoded splatmap PNG). `Some` once a `Paint` stroke has actually
    /// touched this terrain this drag.
    weights: Option<Vec<u8>>,
    /// Heightmap/splatmap pixel width; both grids share this (see
    /// `terrain_chunking::SplatmapOverride`'s doc comment: a splatmap's
    /// dimensions are assumed to match the heightmap's).
    width: u32,
    /// Heightmap/splatmap pixel height; see `width`.
    height: u32,
}

/// Raw heightmap units (out of `u16::MAX`) a height stroke shifts the texel
/// exactly at the brush center by, per frame, at `strength == 1.0` --
/// scaled down by the smoothstep falloff for texels away from center, and
/// linearly by `strength` itself (see `TerrainBrushSettings::strength`).
/// Not tied to any real-world unit; chosen so a held brush visibly
/// raises/lowers terrain over a couple dozen frames rather than jumping to
/// a plateau on frame one.
const HEIGHT_BRUSH_RATE: f32 = 3000.0;

/// Raises (`raise == true`) or lowers raw heightmap samples within `radius`
/// world units of `(local_x, local_z)` -- both already `Terrain`-local
/// (the world-space brush position minus the `Terrain`'s own `Transform`).
/// Falloff is smoothstep, from full effect at the center to zero at
/// `radius`, so a brush edge blends rather than hard-stepping.
fn apply_height_brush(
    heights: &mut [u16],
    width: u32,
    height: u32,
    params: &crate::terrain_chunking::ChunkParams,
    local_x: f32,
    local_z: f32,
    radius: f32,
    strength: f32,
    raise: bool,
) {
    if width == 0 || height == 0 || radius <= 0.0 {
        return;
    }
    let (step_x, step_z) = crate::terrain_chunking::texel_world_step(width, height, params);
    let (center_x, center_z) =
        crate::terrain_chunking::world_to_texel(local_x, local_z, width, height, params);
    let radius_texels_x = (radius / step_x).ceil() as i32;
    let radius_texels_z = (radius / step_z).ceil() as i32;
    let strength = strength.clamp(0.0, 1.0);
    let sign = if raise { 1.0 } else { -1.0 };

    for dz in -radius_texels_z..=radius_texels_z {
        for dx in -radius_texels_x..=radius_texels_x {
            let hx = center_x as i32 + dx;
            let hz = center_z as i32 + dz;
            if hx < 0 || hz < 0 || hx >= width as i32 || hz >= height as i32 {
                continue;
            }
            let wx = hx as f32 * step_x;
            let wz = hz as f32 * step_z;
            let dist = ((wx - local_x).powi(2) + (wz - local_z).powi(2)).sqrt();
            if dist > radius {
                continue;
            }
            let t = 1.0 - (dist / radius);
            let falloff = t * t * (3.0 - 2.0 * t); // smoothstep
            let delta = HEIGHT_BRUSH_RATE * strength * falloff * sign;
            let idx = (hz as u32 * width + hx as u32) as usize;
            let new_val = (heights[idx] as f32 + delta).clamp(0.0, u16::MAX as f32);
            heights[idx] = new_val as u16;
        }
    }
}

/// Blends RGBA8 splat weights within `radius` world units of `(local_x,
/// local_z)` towards a one-hot target vector for `layer` (0-3, matching
/// `TerrainBrushKind::Paint::layer` and `splat_weight_for`'s [grass, rock,
/// dirt, snow] channel order). Falloff and `strength` scale the per-frame
/// lerp factor towards the target -- the same smoothstep shape
/// `apply_height_brush` uses, but blended rather than additive: a paint
/// brush converges on a target color, it doesn't accumulate without bound.
fn apply_paint_brush(
    weights: &mut [u8],
    width: u32,
    height: u32,
    params: &crate::terrain_chunking::ChunkParams,
    local_x: f32,
    local_z: f32,
    radius: f32,
    strength: f32,
    layer: u8,
) {
    if width == 0 || height == 0 || radius <= 0.0 {
        return;
    }
    let layer = layer.min(3) as usize;
    let (step_x, step_z) = crate::terrain_chunking::texel_world_step(width, height, params);
    let (center_x, center_z) =
        crate::terrain_chunking::world_to_texel(local_x, local_z, width, height, params);
    let radius_texels_x = (radius / step_x).ceil() as i32;
    let radius_texels_z = (radius / step_z).ceil() as i32;
    let strength = strength.clamp(0.0, 1.0);

    for dz in -radius_texels_z..=radius_texels_z {
        for dx in -radius_texels_x..=radius_texels_x {
            let hx = center_x as i32 + dx;
            let hz = center_z as i32 + dz;
            if hx < 0 || hz < 0 || hx >= width as i32 || hz >= height as i32 {
                continue;
            }
            let wx = hx as f32 * step_x;
            let wz = hz as f32 * step_z;
            let dist = ((wx - local_x).powi(2) + (wz - local_z).powi(2)).sqrt();
            if dist > radius {
                continue;
            }
            let t = 1.0 - (dist / radius);
            let t = t * t * (3.0 - 2.0 * t) * strength; // smoothstep * strength
            let idx = ((hz as u32 * width + hx as u32) * 4) as usize;
            for c in 0..4 {
                let target = if c == layer { 255.0 } else { 0.0 };
                let old = weights[idx + c] as f32;
                weights[idx + c] = (old + (target - old) * t).clamp(0.0, 255.0) as u8;
            }
        }
    }
}

/// Recovers a chunk entity's (cx, cz) grid coordinate from its own
/// `Transform` and its `Terrain`'s, inverting the placement
/// `generate_terrain_chunks` (terrain.rs) uses when it spawns each chunk at
/// `transform.position + (chunk.world_offset.0, 0, chunk.world_offset.1)`
/// where `world_offset == (cx * chunk_size, cz * chunk_size)`. Used instead
/// of relying on `Query` iteration order, which nothing guarantees to
/// preserve.
fn chunk_grid_coords(
    chunk_transform: &Transform,
    terrain_transform: &Transform,
    chunk_size: f32,
) -> (i32, i32) {
    let offset = chunk_transform.position.0 - terrain_transform.position.0;
    (
        (offset.x / chunk_size).round() as i32,
        (offset.z / chunk_size).round() as i32,
    )
}

/// Whether chunk `(cx, cz)`'s world-space AABB (size `chunk_size` per side)
/// falls within `radius` of the `Terrain`-local brush center `(local_x,
/// local_z)` -- a circle-vs-AABB overlap test (closest point on the box to
/// the circle's center). Used to skip re-uploading chunks the brush isn't
/// anywhere near.
fn chunk_touches_brush(
    cx: i32,
    cz: i32,
    chunk_size: f32,
    local_x: f32,
    local_z: f32,
    radius: f32,
) -> bool {
    if cx < 0 || cz < 0 {
        return false;
    }
    let min_x = cx as f32 * chunk_size;
    let min_z = cz as f32 * chunk_size;
    let closest_x = local_x.clamp(min_x, min_x + chunk_size);
    let closest_z = local_z.clamp(min_z, min_z + chunk_size);
    let dx = local_x - closest_x;
    let dz = local_z - closest_z;
    (dx * dx + dz * dz).sqrt() <= radius
}

/// Row-major index into `generate_chunks`/`generate_chunks_with_splatmap`'s
/// output for grid coordinate `(cx, cz)`, matching that function's own `for
/// cz { for cx { chunks.push(..) } }` iteration/push order. `chunks_x` is
/// `params.chunk_count.0`.
fn chunk_index(cx: i32, cz: i32, chunks_x: u32, total: usize) -> Option<usize> {
    if cx < 0 || cz < 0 {
        return None;
    }
    let idx = (cz as u32 * chunks_x + cx as u32) as usize;
    (idx < total).then_some(idx)
}

/// Derives a fresh splatmap path from `heightmap_path` the first time a
/// paint stroke touches a `Terrain` with no `splatmap_path` yet --
/// `"terrain/hills.png"` -> `"terrain/hills_splatmap.png"`, alongside the
/// heightmap. Always forward-slash, matching this codebase's convention for
/// authored asset paths.
fn default_splatmap_path(heightmap_path: &str) -> String {
    let path = std::path::Path::new(heightmap_path);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("terrain");
    let ext = path.extension().and_then(|s| s.to_str()).unwrap_or("png");
    let file_name = format!("{stem}_splatmap.{ext}");
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => {
            parent.join(&file_name).to_string_lossy().replace('\\', "/")
        }
        _ => file_name,
    }
}

/// Every frame `InspectorState::terrain_brush_stroke` is `Some`, mutates
/// `TerrainBrushEditState`'s in-memory grid and re-uploads every touched
/// chunk's full vertex buffer (`Height`) or weight texture (`Paint`) --
/// live preview only, no physics or disk I/O (`commit_terrain_brush_stroke`
/// below handles those once the drag ends).
///
/// Recomputes the whole terrain's chunk geometry/weights in CPU memory each
/// call (the same work `generate_terrain_chunks` already does once at spawn
/// time, so it's cheap) but only re-uploads chunks the brush actually
/// overlaps (`chunk_touches_brush`) -- full per-chunk buffers, not a partial
/// sub-region diff, matching every other GPU update path in this codebase
/// (`GpuMeshRegistry::update_vertices`/`GpuTextureRegistry::replace` have no
/// partial-update variant to call instead).
fn preview_terrain_brush_stroke(
    inspector: Option<Res<InspectorState>>,
    terrain_query: Query<(Entity, &Terrain, &Transform), With<TerrainChunksGenerated>>,
    chunk_query: Query<(&Transform, &MeshRenderer, &TerrainSplat, &TerrainChunkOf)>,
    mut edit_state: ResMut<TerrainBrushEditState>,
    mut mesh_registry: Option<ResMut<GpuMeshRegistry>>,
    mut tex_registry: Option<ResMut<GpuTextureRegistry>>,
    gpu_queue: Option<Res<GpuQueueResource>>,
) {
    let Some(insp) = inspector else {
        return;
    };
    let Some(stroke) = insp.terrain_brush_stroke else {
        return;
    };
    let Some((terrain_entity, terrain, terrain_transform)) = terrain_query
        .iter()
        .find(|(e, _, _)| e.index() as u64 == stroke.terrain_entity_id)
    else {
        return;
    };

    if edit_state.terrain_entity_id != Some(stroke.terrain_entity_id) {
        *edit_state = TerrainBrushEditState {
            terrain_entity_id: Some(stroke.terrain_entity_id),
            ..Default::default()
        };
    }

    let params = crate::terrain_chunking::ChunkParams {
        chunk_count: terrain.chunk_count,
        chunk_size: terrain.chunk_size,
        height_scale: terrain.height_scale,
    };
    let local = Vec3::from(stroke.world_pos) - terrain_transform.position.0;
    let radius = insp.terrain_brush_settings.radius;
    let strength = insp.terrain_brush_settings.strength;

    match insp.terrain_brush_settings.kind {
        TerrainBrushKind::Height { raise } => {
            if edit_state.heights.is_none() {
                match std::fs::read(&terrain.heightmap_path)
                    .map_err(|e| e.to_string())
                    .and_then(|bytes| {
                        bsengine_asset::heightmap_loader::decode_heightmap_png(&bytes)
                    }) {
                    Ok(hm) => {
                        edit_state.width = hm.width;
                        edit_state.height = hm.height;
                        edit_state.heights = Some(hm.data);
                    }
                    Err(e) => {
                        warn!(
                            "[terrain-brush] could not load heightmap '{}' for preview: {e}",
                            terrain.heightmap_path
                        );
                        return;
                    }
                }
            }
            let width = edit_state.width;
            let height = edit_state.height;
            let Some(heights) = edit_state.heights.as_mut() else {
                return;
            };

            apply_height_brush(
                heights, width, height, &params, local.x, local.z, radius, strength, raise,
            );

            let Some(mesh_reg) = mesh_registry.as_mut() else {
                return;
            };
            let Some(queue) = gpu_queue.as_deref() else {
                return;
            };
            let hm = bsengine_asset::HeightmapAsset {
                width,
                height,
                data: heights.clone(),
            };
            let chunks = crate::terrain_chunking::generate_chunks(&hm, &params);
            let chunks_x = params.chunk_count.0;

            for (chunk_transform, mesh_renderer, _splat, chunk_of) in chunk_query.iter() {
                if chunk_of.0 != terrain_entity {
                    continue;
                }
                let (cx, cz) =
                    chunk_grid_coords(chunk_transform, terrain_transform, params.chunk_size);
                if !chunk_touches_brush(cx, cz, params.chunk_size, local.x, local.z, radius) {
                    continue;
                }
                let Some(idx) = chunk_index(cx, cz, chunks_x, chunks.len()) else {
                    continue;
                };
                if !mesh_reg.update_vertices(&queue.0, mesh_renderer.mesh_id, &chunks[idx].vertices)
                {
                    warn!("[terrain-brush] failed to upload preview vertices for a touched chunk");
                }
            }
        }
        TerrainBrushKind::Paint { layer } => {
            if edit_state.weights.is_none() {
                let seeded = terrain.splatmap_path.as_ref().and_then(|path| {
                    image::open(path).ok().map(|img| {
                        let rgba = img.to_rgba8();
                        (rgba.width(), rgba.height(), rgba.into_raw())
                    })
                });
                let (width, height, data) = match seeded {
                    Some(v) => v,
                    None => {
                        // No splatmap yet (or it failed to load): seed from
                        // the same procedural formula the chunks are
                        // already rendering, so painting doesn't
                        // discontinuously reset the rest of the terrain.
                        match std::fs::read(&terrain.heightmap_path)
                            .map_err(|e| e.to_string())
                            .and_then(|bytes| {
                                bsengine_asset::heightmap_loader::decode_heightmap_png(&bytes)
                            }) {
                            Ok(hm) => {
                                let grid =
                                    crate::terrain_chunking::procedural_splat_grid(&hm, &params);
                                (hm.width, hm.height, grid)
                            }
                            Err(e) => {
                                warn!(
                                    "[terrain-brush] could not load heightmap '{}' to seed \
                                     paint preview: {e}",
                                    terrain.heightmap_path
                                );
                                return;
                            }
                        }
                    }
                };
                edit_state.width = width;
                edit_state.height = height;
                edit_state.weights = Some(data);
            }
            let width = edit_state.width;
            let height = edit_state.height;
            let Some(weights) = edit_state.weights.as_mut() else {
                return;
            };

            apply_paint_brush(
                weights, width, height, &params, local.x, local.z, radius, strength, layer,
            );

            let Some(tex_reg) = tex_registry.as_mut() else {
                return;
            };
            // `generate_chunks_with_splatmap` always computes real vertex
            // heights/normals too, even though a splatmap override makes
            // its splat-weight output ignore them entirely -- so any
            // correctly-shaped heightmap works here; the actual values in
            // `dummy_hm.data` are never read for anything this branch uses.
            let dummy_hm = bsengine_asset::HeightmapAsset {
                width,
                height,
                data: vec![0u16; (width as usize) * (height as usize)],
            };
            let overlay = crate::terrain_chunking::SplatmapOverride {
                width,
                height,
                data: weights.clone(),
            };
            let chunks = crate::terrain_chunking::generate_chunks_with_splatmap(
                &dummy_hm,
                &params,
                Some(&overlay),
            );
            let chunks_x = params.chunk_count.0;

            for (chunk_transform, _mesh_renderer, splat, chunk_of) in chunk_query.iter() {
                if chunk_of.0 != terrain_entity {
                    continue;
                }
                let (cx, cz) =
                    chunk_grid_coords(chunk_transform, terrain_transform, params.chunk_size);
                if !chunk_touches_brush(cx, cz, params.chunk_size, local.x, local.z, radius) {
                    continue;
                }
                let Some(idx) = chunk_index(cx, cz, chunks_x, chunks.len()) else {
                    continue;
                };
                let chunk = &chunks[idx];
                let weight_bytes: Vec<u8> = chunk.splat_weights.iter().flatten().copied().collect();
                if !tex_reg.replace(
                    splat.weight_texture_id,
                    chunk.heightfield_cols as u32,
                    chunk.heightfield_rows as u32,
                    &weight_bytes,
                ) {
                    warn!("[terrain-brush] failed to upload preview weights for a touched chunk");
                }
            }
        }
    }
}

/// Runs every frame; on the frame `InspectorState::terrain_brush_stroke`
/// transitions from `Some` (last frame) to `None` (this frame) -- i.e. the
/// drag just ended -- rebuilds the affected chunks' Rapier heightfield
/// colliders and persists the edited grid to disk.
///
/// Uses `Local<Option<TerrainBrushStroke>>` (a per-system, non-shared
/// value) rather than a new `InspectorState` field to detect the
/// transition: nothing else in the engine needs to observe "did a stroke
/// just end," so it doesn't belong on the shared editor blackboard
/// resource.
fn commit_terrain_brush_stroke(
    inspector: Option<Res<InspectorState>>,
    mut prev_stroke: Local<Option<TerrainBrushStroke>>,
    mut terrain_query: Query<(Entity, &mut Terrain, &Transform), With<TerrainChunksGenerated>>,
    mut chunk_query: Query<(Entity, &Transform, &TerrainChunkOf, &mut Collider)>,
    mut edit_state: ResMut<TerrainBrushEditState>,
    mut physics: Option<ResMut<PhysicsWorld>>,
) {
    let current = inspector.as_deref().and_then(|i| i.terrain_brush_stroke);
    let previous = std::mem::replace(&mut *prev_stroke, current);

    let Some(ended) = previous else {
        return;
    };
    if current.is_some() {
        return; // still dragging -- not a Some -> None transition
    }

    // Take (not just read) so a mid-commit early return below still leaves
    // a clean slate for the next stroke, rather than a half-applied one
    // that the next drag's first-touch check would mistake for its own.
    let state = std::mem::take(&mut *edit_state);
    if state.terrain_entity_id != Some(ended.terrain_entity_id) {
        // Nothing was ever previewed for this stroke (e.g. no
        // GpuMeshRegistry was ever available) -- nothing to commit.
        return;
    }

    let Some((terrain_entity, mut terrain, terrain_transform)) = terrain_query
        .iter_mut()
        .find(|(e, _, _)| e.index() as u64 == ended.terrain_entity_id)
    else {
        return;
    };

    let params = crate::terrain_chunking::ChunkParams {
        chunk_count: terrain.chunk_count,
        chunk_size: terrain.chunk_size,
        height_scale: terrain.height_scale,
    };

    if let Some(heights) = &state.heights {
        match bsengine_asset::heightmap_loader::encode_heightmap_png(
            state.width,
            state.height,
            heights,
        ) {
            Ok(bytes) => {
                if let Err(e) = std::fs::write(&terrain.heightmap_path, bytes) {
                    warn!(
                        "[terrain-brush] failed to write heightmap '{}': {e}",
                        terrain.heightmap_path
                    );
                }
            }
            Err(e) => warn!("[terrain-brush] failed to encode edited heightmap: {e}"),
        }

        // Rebuild every chunk's collider from the edited heightmap. Rebuilds
        // every chunk of this terrain rather than tracking exactly which
        // ones the stroke touched over the drag -- simpler, and correct
        // regardless of how many chunks a stroke spanned; it only runs once
        // per commit, not per frame.
        let hm = bsengine_asset::HeightmapAsset {
            width: state.width,
            height: state.height,
            data: heights.clone(),
        };
        let chunks = crate::terrain_chunking::generate_chunks(&hm, &params);
        let chunks_x = params.chunk_count.0;
        if let Some(physics) = physics.as_mut() {
            for (chunk_entity, chunk_transform, chunk_of, mut collider) in chunk_query.iter_mut() {
                if chunk_of.0 != terrain_entity {
                    continue;
                }
                let (cx, cz) =
                    chunk_grid_coords(chunk_transform, terrain_transform, params.chunk_size);
                let Some(idx) = chunk_index(cx, cz, chunks_x, chunks.len()) else {
                    continue;
                };
                let chunk = &chunks[idx];
                let shape = ColliderShape::Heightfield {
                    heights: chunk.heightfield_heights.clone(),
                    rows: chunk.heightfield_rows,
                    cols: chunk.heightfield_cols,
                    scale: Vec3::new(params.chunk_size, 1.0, params.chunk_size).into(),
                };
                if !physics.set_collider_shape(chunk_entity, &shape) {
                    warn!("[terrain-brush] failed to rebuild collider for a terrain chunk");
                }
                // Keep the ECS-visible `Collider` component in step with the
                // Rapier-side shape `set_collider_shape` just rebuilt --
                // chunks aren't scene-serialized (they're regenerated from
                // the heightmap on next load), so this is purely for
                // anything else that inspects `Collider.shape` at runtime.
                collider.shape = shape;
            }
        }
    }

    if let Some(weights) = &state.weights {
        let path = terrain
            .splatmap_path
            .clone()
            .unwrap_or_else(|| default_splatmap_path(&terrain.heightmap_path));
        match image::RgbaImage::from_raw(state.width, state.height, weights.clone()) {
            Some(img) => match img.save(&path) {
                Ok(()) => {
                    if terrain.splatmap_path.is_none() {
                        terrain.splatmap_path = Some(path);
                    }
                }
                Err(e) => warn!("[terrain-brush] failed to write splatmap '{path}': {e}"),
            },
            None => warn!(
                "[terrain-brush] edited splat weights did not match {}x{}",
                state.width, state.height
            ),
        }
    }
}

/// Bevy plugin that runs the terrain brush's picking, live-preview, and
/// commit systems each frame, in that order.
pub struct TerrainBrushPlugin;

impl Plugin for TerrainBrushPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainBrushEditState>().add_systems(
            Update,
            (
                pick_terrain_under_cursor,
                preview_terrain_brush_stroke,
                commit_terrain_brush_stroke,
            )
                .chain(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn screen_center_unprojects_to_the_camera_forward_direction() {
        let cam_pos = Vec3::new(0.0, 5.0, 10.0);
        let proj = Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(cam_pos, Vec3::ZERO, Vec3::Y);
        let view_proj = proj * view;

        let (origin, dir) = screen_to_world_ray(
            view_proj,
            cam_pos,
            (640.0, 360.0),
            (0.0, 0.0),
            (1280.0, 720.0),
        );

        assert!((origin - cam_pos).length() < 1e-4);
        let expected_dir = (Vec3::ZERO - cam_pos).normalize();
        assert!(
            dir.dot(expected_dir) > 0.999,
            "screen center should unproject close to the camera-forward direction, \
             got dir={dir:?}, expected~={expected_dir:?}"
        );
    }

    #[test]
    fn a_ray_through_a_known_point_passes_near_it() {
        // Camera looking straight down at the origin from above; the world
        // point (2, 0, 0) should unproject from wherever its own screen
        // projection is.
        let cam_pos = Vec3::new(0.0, 10.0, 0.0);
        let proj = Mat4::perspective_rh(60f32.to_radians(), 1.0, 0.1, 1000.0);
        let view = Mat4::look_at_rh(cam_pos, Vec3::ZERO, Vec3::NEG_Z);
        let view_proj = proj * view;

        let target = Vec3::new(2.0, 0.0, 0.0);
        let clip = view_proj * target.extend(1.0);
        let ndc = clip.truncate() / clip.w;
        let screen_x = (ndc.x * 0.5 + 0.5) * 800.0;
        let screen_y = (1.0 - (ndc.y * 0.5 + 0.5)) * 800.0;

        let (origin, dir) = screen_to_world_ray(
            view_proj,
            cam_pos,
            (screen_x, screen_y),
            (0.0, 0.0),
            (800.0, 800.0),
        );

        // Distance from `target` to the infinite ray (origin, dir).
        let to_target = target - origin;
        let t = to_target.dot(dir);
        let closest = origin + dir * t;
        let dist = (closest - target).length();
        assert!(
            dist < 0.05,
            "ray through target's own screen projection should pass within 5cm of it, got {dist}"
        );
    }

    #[test]
    fn terrain_brush_plugin_can_be_added_to_app() {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        app.insert_resource(InspectorState::default());
        app.add_plugins(TerrainBrushPlugin);
        app.update();
    }

    #[test]
    fn inactive_brush_leaves_terrain_pick_none() {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        // `terrain_brush_active` defaults to false; the picking system must
        // leave `terrain_pick` untouched (still `None`) rather than raycast.
        app.insert_resource(InspectorState::default());
        app.add_plugins(TerrainBrushPlugin);
        app.update();

        let insp = app.world().resource::<InspectorState>();
        assert_eq!(insp.terrain_pick, None);
    }

    #[test]
    fn active_brush_without_a_view_proj_leaves_terrain_pick_none() {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        // Active and hovered, but no camera matrix yet (e.g. before the
        // editor viewport has rendered a first frame) -- must not panic and
        // must leave the pick cleared rather than raycasting with stale data.
        // Built via mutation, not struct-literal update syntax: InspectorState
        // has private fields (e.g. `prev_selected_id`), so only this crate's
        // own `Default` impl can construct one at all.
        let mut inspector = InspectorState::default();
        inspector.terrain_brush_active = true;
        inspector.viewport_contains_cursor = true;
        inspector.editor_view_proj = None;
        app.insert_resource(inspector);
        app.add_plugins(TerrainBrushPlugin);
        app.update();

        let insp = app.world().resource::<InspectorState>();
        assert_eq!(insp.terrain_pick, None);
    }

    // ---- apply_height_brush / apply_paint_brush: pure-function coverage ----
    //
    // These are the tests that actually prove a brush stroke computes
    // different values than before -- see this module's `preview_terrain_
    // brush_stroke` test below for why an end-to-end GPU-level assertion
    // can't do this (no read-back API exists; `mesh.rs`'s own
    // `update_vertices_overwrites_existing_buffer_contents` test hits the
    // exact same wall and settles for "didn't panic, still registered").
    // What actually reaches `update_vertices`/`GpuTextureRegistry::replace`
    // is exactly the array these pure functions computed, so testing them
    // directly is testing the real content of the upload.

    fn flat_params() -> crate::terrain_chunking::ChunkParams {
        crate::terrain_chunking::ChunkParams {
            chunk_count: (1, 1),
            chunk_size: 10.0,
            height_scale: 20.0,
        }
    }

    #[test]
    fn apply_height_brush_raises_more_at_the_center_than_the_edge_and_leaves_outside_radius_untouched(
    ) {
        let width = 5;
        let height = 5;
        let mut heights = vec![10_000u16; (width * height) as usize];
        let original = heights.clone();
        let params = flat_params(); // chunk_size=10, chunk_count=(1,1) -> texel step = 10/5 = 2.0

        // world (4.0, 4.0) lands exactly on texel (2, 2) -- an exact
        // multiple of the 2.0 step, not a half-step, so `world_to_texel`'s
        // rounding is unambiguous.
        apply_height_brush(
            &mut heights,
            width,
            height,
            &params,
            4.0,
            4.0,
            3.0,
            1.0,
            true,
        );

        let idx_at = |x: u32, z: u32| (z * width + x) as usize;
        let center = idx_at(2, 2); // world (4.0, 4.0) at step 2.0 -> texel (2,2)
        let near_edge = idx_at(1, 2); // world (2.0, 4.0): distance 2.0 < radius 3.0
        let far_corner = idx_at(0, 0); // world (0,0): distance ~5.66 > radius 3.0

        assert!(
            heights[center] > original[center],
            "the brush center should have been raised"
        );
        assert!(
            heights[near_edge] > original[near_edge],
            "a nearby texel should also have been raised"
        );
        assert!(
            heights[near_edge] - original[near_edge] < heights[center] - original[center],
            "falloff should make a texel farther from the center rise less than the center: \
             center delta={}, near-edge delta={}",
            heights[center] - original[center],
            heights[near_edge] - original[near_edge]
        );
        assert_eq!(
            heights[far_corner], original[far_corner],
            "a texel outside the brush radius must be left untouched"
        );
    }

    #[test]
    fn apply_height_brush_lowers_when_raise_is_false() {
        let width = 5;
        let height = 5;
        let mut heights = vec![10_000u16; (width * height) as usize];
        let original = heights.clone();
        let params = flat_params();

        apply_height_brush(
            &mut heights,
            width,
            height,
            &params,
            4.0,
            4.0,
            3.0,
            1.0,
            false,
        );

        let center = (2 * width + 2) as usize; // world (4.0, 4.0) at step 2.0 -> texel (2,2)
        assert!(
            heights[center] < original[center],
            "raise=false should lower the brushed texel, got {} (was {})",
            heights[center],
            original[center]
        );
    }

    #[test]
    fn apply_height_brush_clamps_at_u16_max_instead_of_wrapping() {
        let width = 3;
        let height = 3;
        let mut heights = vec![u16::MAX; (width * height) as usize];
        let params = flat_params();

        // Already saturated; raising further must clamp, not wrap around to
        // a small number (which would silently invert the terrain).
        apply_height_brush(
            &mut heights,
            width,
            height,
            &params,
            5.0,
            5.0,
            10.0,
            1.0,
            true,
        );

        assert!(
            heights.iter().all(|&h| h == u16::MAX),
            "raising an already-saturated grid must clamp at u16::MAX, got {heights:?}"
        );
    }

    #[test]
    fn apply_paint_brush_blends_the_center_toward_the_target_layer() {
        let width = 5;
        let height = 5;
        // Start fully grass (channel 0) everywhere, matching what
        // `splat_weight_for` would produce for a flat, low terrain.
        let mut weights = vec![0u8; (width * height * 4) as usize];
        for px in weights.chunks_mut(4) {
            px[0] = 255;
        }
        let params = flat_params();

        // Paint layer 1 (rock) at the center with full strength.
        apply_paint_brush(&mut weights, width, height, &params, 4.0, 4.0, 3.0, 1.0, 1);

        let center = ((2 * width + 2) * 4) as usize; // world (4.0, 4.0) at step 2.0 -> texel (2,2)
        assert!(
            weights[center + 1] > weights[center],
            "rock (channel 1) should now outweigh grass (channel 0) at the brush center, \
             got {:?}",
            &weights[center..center + 4]
        );

        let far_corner = 0usize; // world (0,0), outside radius 3.0
        assert_eq!(
            &weights[far_corner..far_corner + 4],
            &[255, 0, 0, 0],
            "a texel outside the brush radius must be left untouched"
        );
    }

    // ---- end-to-end preview/commit tests ----

    fn write_test_heightmap(name: &str, width: u32, height: u32, values: &[u16]) -> String {
        let img: image::ImageBuffer<image::Luma<u16>, Vec<u16>> =
            image::ImageBuffer::from_raw(width, height, values.to_vec())
                .expect("test fixture dimensions must match values.len()");
        let mut bytes = Vec::new();
        image::DynamicImage::ImageLuma16(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .expect("encoding the test fixture PNG failed");
        let path = std::env::temp_dir().join(format!(
            "bsengine_terrain_brush_test_{name}_{}.png",
            std::process::id()
        ));
        std::fs::write(&path, bytes).expect("write test heightmap fixture");
        path.to_str().unwrap().to_owned()
    }

    fn write_test_texture(name: &str, rgba: [u8; 4]) -> String {
        let img = image::RgbaImage::from_pixel(2, 2, image::Rgba(rgba));
        let path = std::env::temp_dir().join(format!(
            "bsengine_terrain_brush_test_tex_{name}_{}.png",
            std::process::id()
        ));
        img.save(&path).expect("write test texture fixture");
        path.to_str().unwrap().to_owned()
    }

    /// Inserts real, headless `GpuMeshRegistry`/`GpuTextureRegistry`/
    /// `GpuQueueResource` -- not stand-ins, the same types the renderer
    /// uses. Mirrors `bsengine-gltf`'s own
    /// `insert_headless_gpu_registries` test helper exactly (including
    /// inserting `GpuQueueResource`, which `WgpuRHIPlugin::windowed()`
    /// alone never does without a real window/surface): this module's
    /// `preview_terrain_brush_stroke` needs all three, the same way
    /// `bsengine-gltf`'s `update_skinned_meshes` does, so a helper that
    /// only supplied the two registries would silently exclude this
    /// system from every test built on it.
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
                label: Some("bsengine-app terrain-brush test device"),
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
        app.insert_resource(GpuQueueResource(queue));
    }

    /// An app with everything the terrain brush needs end-to-end: a real
    /// `AssetServer`, real headless GPU registries, `PhysicsPlugin`,
    /// `TerrainPlugin` (to actually spawn chunks), and `TerrainBrushPlugin`
    /// itself.
    fn brush_test_app() -> bevy_app::App {
        let mut app = crate::new_app();
        app.add_plugins(bsengine_asset::AssetPlugin);
        app.add_plugins(bsengine_rhi_wgpu::WgpuRHIPlugin::windowed());
        app.add_plugins(bsengine_physics::PhysicsPlugin);
        app.add_plugins(bsengine_input::InputPlugin);
        app.add_plugins(crate::terrain::TerrainPlugin);
        insert_headless_gpu_registries(&mut app);
        app.insert_resource(InspectorState::default());
        app.add_plugins(TerrainBrushPlugin);
        app
    }

    fn spawn_flat_terrain(
        app: &mut bevy_app::App,
        name: &str,
        width: u32,
        height: u32,
        flat_raw: u16,
        chunk_count: (u32, u32),
        chunk_size: f32,
        height_scale: f32,
    ) -> Entity {
        let path = write_test_heightmap(
            name,
            width,
            height,
            &vec![flat_raw; (width * height) as usize],
        );
        app.world_mut()
            .spawn((
                Terrain {
                    heightmap_path: path,
                    chunk_count,
                    chunk_size,
                    height_scale,
                    layer0_texture_path: write_test_texture(
                        &format!("{name}-l0"),
                        [50, 200, 50, 255],
                    ),
                    layer1_texture_path: write_test_texture(
                        &format!("{name}-l1"),
                        [120, 120, 120, 255],
                    ),
                    layer2_texture_path: write_test_texture(
                        &format!("{name}-l2"),
                        [110, 80, 40, 255],
                    ),
                    layer3_texture_path: write_test_texture(
                        &format!("{name}-l3"),
                        [240, 240, 250, 255],
                    ),
                    splatmap_path: None,
                },
                Transform::from_position(Vec3::ZERO),
            ))
            .id()
    }

    fn run_until_generated(app: &mut bevy_app::App, entity: Entity) {
        for _ in 0..200 {
            app.update();
            if app
                .world()
                .get::<crate::terrain::TerrainChunksGenerated>(entity)
                .is_some()
            {
                return;
            }
        }
        panic!("terrain chunks were not generated within 200 frames");
    }

    /// End-to-end smoke test mirroring `mesh.rs`'s own
    /// `update_vertices_overwrites_existing_buffer_contents` idiom: no GPU
    /// read-back API exists, so this proves the real system runs against a
    /// real `GpuMeshRegistry`/`GpuQueueResource` without panicking and
    /// leaves the touched chunk's mesh registered -- the numeric proof that
    /// different Y values were actually computed lives in the
    /// `apply_height_brush` pure-function tests above, which is exactly
    /// what gets forwarded into `update_vertices` here.
    #[test]
    fn preview_terrain_brush_stroke_runs_end_to_end_without_panicking_and_leaves_the_chunk_mesh_registered(
    ) {
        let mut app = brush_test_app();
        let terrain_entity =
            spawn_flat_terrain(&mut app, "preview", 4, 4, 20_000, (1, 1), 10.0, 20.0);
        run_until_generated(&mut app, terrain_entity);

        let mesh_id = {
            let mut q = app.world_mut().query::<&MeshRenderer>();
            q.iter(app.world()).next().expect("one chunk").mesh_id
        };

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_settings.kind = TerrainBrushKind::Height { raise: true };
            insp.terrain_brush_settings.radius = 20.0;
            insp.terrain_brush_settings.strength = 1.0;
            insp.terrain_brush_stroke = Some(TerrainBrushStroke {
                terrain_entity_id: terrain_entity.index() as u64,
                world_pos: [5.0, 0.0, 5.0],
            });
        }
        for _ in 0..5 {
            app.update();
        }

        let mesh_registry = app.world().resource::<GpuMeshRegistry>();
        assert!(
            mesh_registry.get(mesh_id).is_some(),
            "the touched chunk's mesh must still be registered after preview updates"
        );
    }

    /// The physics regression test: proves `set_collider_shape` is actually
    /// wired end-to-end by this task's systems (Task 2 already unit-tested
    /// the method itself in isolation). Mirrors terrain.rs's own
    /// `a_dropped_body_lands_on_the_chunk_it_visually_sits_above` in shape,
    /// but drives the new height through a real held brush stroke and
    /// commit first, then confirms a dropped body lands at whatever height
    /// the brush actually produced (read back from the committed heightmap
    /// PNG) rather than the original flat height -- so this test isn't
    /// coupled to `HEIGHT_BRUSH_RATE`'s exact tuning value.
    #[test]
    fn held_height_stroke_then_commit_raises_where_a_dropped_body_lands() {
        let mut app = brush_test_app();

        let flat_raw: u16 = 20_000;
        let height_scale = 20.0f32;
        let chunk_size = 10.0f32;
        let original_height = (flat_raw as f32 / u16::MAX as f32) * height_scale;

        let terrain_entity = spawn_flat_terrain(
            &mut app,
            "physics-regression",
            4,
            4,
            flat_raw,
            (1, 1),
            chunk_size,
            height_scale,
        );
        run_until_generated(&mut app, terrain_entity);
        let heightmap_path = app
            .world()
            .get::<Terrain>(terrain_entity)
            .unwrap()
            .heightmap_path
            .clone();

        let drop_xz = chunk_size / 2.0;
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_settings.kind = TerrainBrushKind::Height { raise: true };
            insp.terrain_brush_settings.radius = 20.0;
            insp.terrain_brush_settings.strength = 1.0;
            insp.terrain_brush_stroke = Some(TerrainBrushStroke {
                terrain_entity_id: terrain_entity.index() as u64,
                world_pos: [drop_xz, 0.0, drop_xz],
            });
        }
        // Hold the drag for a few frames -- each frame nudges the center
        // texel up by HEIGHT_BRUSH_RATE at full strength/falloff.
        for _ in 0..5 {
            app.update();
        }
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_stroke = None;
        }
        app.update(); // the Some -> None transition frame: commits

        // Read back the actual height the brush produced, rather than
        // hardcoding an expected value derived from HEIGHT_BRUSH_RATE.
        let bytes = std::fs::read(&heightmap_path).expect("heightmap PNG should still exist");
        let decoded = bsengine_asset::heightmap_loader::decode_heightmap_png(&bytes)
            .expect("decode the committed heightmap");
        let params = crate::terrain_chunking::ChunkParams {
            chunk_count: (1, 1),
            chunk_size,
            height_scale,
        };
        let (tx, tz) = crate::terrain_chunking::world_to_texel(
            drop_xz,
            drop_xz,
            decoded.width,
            decoded.height,
            &params,
        );
        let raw = decoded.data[(tz * decoded.width + tx) as usize];
        let new_height = (raw as f32 / u16::MAX as f32) * height_scale;
        assert!(
            new_height > original_height + 0.5,
            "the committed heightmap should be measurably higher than the original: \
             new={new_height}, original={original_height}"
        );

        let radius = 0.5;
        let start = Vec3::new(drop_xz, new_height + 10.0, drop_xz);
        let ball = app
            .world_mut()
            .spawn((
                Transform::from_position(start),
                bsengine_physics::RigidBody::dynamic(),
                bsengine_physics::Collider::ball(radius),
                bsengine_physics::PhysicsInput {
                    position: start.into(),
                    rotation: glam::Quat::IDENTITY.into(),
                },
            ))
            .id();

        for _ in 0..200 {
            app.update();
        }

        let y = app.world().get::<Transform>(ball).unwrap().position.0.y;
        let expected = new_height + radius;
        assert!(
            (y - expected).abs() < 0.3,
            "expected the ball to rest at the BRUSHED height y~={expected} (new terrain \
             height {new_height}, original was {original_height}), but it settled at y={y} \
             -- the collider does not reflect the brushed heightmap"
        );
        assert!(
            (y - (original_height + radius)).abs() > 0.5,
            "the ball must land at the NEW height, not the original -- got y={y}, original \
             resting height would have been {}",
            original_height + radius
        );
    }

    /// Proves the heightmap PNG on disk actually changed after a commit.
    #[test]
    fn committing_a_height_stroke_writes_the_edited_heightmap_to_disk() {
        let mut app = brush_test_app();
        let flat_raw: u16 = 15_000;
        let chunk_size = 10.0;
        let height_scale = 20.0;
        let terrain_entity = spawn_flat_terrain(
            &mut app,
            "disk-persistence",
            4,
            4,
            flat_raw,
            (1, 1),
            chunk_size,
            height_scale,
        );
        run_until_generated(&mut app, terrain_entity);
        let heightmap_path = app
            .world()
            .get::<Terrain>(terrain_entity)
            .unwrap()
            .heightmap_path
            .clone();

        let original_bytes = std::fs::read(&heightmap_path).unwrap();
        let original =
            bsengine_asset::heightmap_loader::decode_heightmap_png(&original_bytes).unwrap();

        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_settings.kind = TerrainBrushKind::Height { raise: true };
            insp.terrain_brush_settings.radius = 20.0;
            insp.terrain_brush_settings.strength = 1.0;
            insp.terrain_brush_stroke = Some(TerrainBrushStroke {
                terrain_entity_id: terrain_entity.index() as u64,
                world_pos: [chunk_size / 2.0, 0.0, chunk_size / 2.0],
            });
        }
        for _ in 0..5 {
            app.update();
        }
        {
            let mut insp = app.world_mut().resource_mut::<InspectorState>();
            insp.terrain_brush_stroke = None;
        }
        app.update(); // commit frame

        let edited_bytes =
            std::fs::read(&heightmap_path).expect("heightmap PNG should still exist");
        let edited = bsengine_asset::heightmap_loader::decode_heightmap_png(&edited_bytes)
            .expect("decode the committed heightmap");

        assert_eq!(edited.width, original.width);
        assert_eq!(edited.height, original.height);
        assert_ne!(
            edited.data, original.data,
            "committing a height stroke must actually change the on-disk heightmap"
        );
    }
}
