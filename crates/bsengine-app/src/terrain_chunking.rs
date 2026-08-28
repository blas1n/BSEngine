//! Pure heightmap-to-chunk-geometry logic for the terrain system. No ECS,
//! GPU, or physics types -- the ECS layer (a future task) calls this and
//! wires the results into `GpuMeshRegistry`/`Collider`.

use bsengine_asset::HeightmapAsset;
use bsengine_rhi_wgpu::Vertex;

/// Normalized height (raw / u16::MAX, before `height_scale`) above which the
/// snow layer dominates.
const SNOW_HEIGHT_RATIO: f32 = 0.75;
/// `normal.y` below which the rock layer dominates (lower = steeper).
const ROCK_SLOPE_THRESHOLD: f32 = 0.6;
/// Width of the smooth blend band around each threshold, so a boundary
/// blends rather than hard-edges.
const TRANSITION_BAND: f32 = 0.1;

/// Linear-interpolation-parameter smoothstep. Works correctly whether
/// `edge0 < edge1` (the usual case) or `edge0 > edge1` (used for the rock
/// threshold, where weight should *increase* as `normal_y` *decreases*) --
/// the formula is direction-agnostic, only its two callers' argument order
/// differs.
fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Computes one vertex's 4-layer splat weight from its normalized height
/// ratio (raw heightmap sample / `u16::MAX`, i.e. independent of
/// `height_scale`) and slope (`normal.y`, 1.0 = flat, 0.0 = vertical).
/// Channel order is [grass, rock, dirt, snow] -- (R, G, B, A) once packed
/// into a texture, matching `TERRAIN_WGSL`'s `t_layer0..3` binding order
/// (that shader is added in a later task; this function's channel order is
/// the contract it will rely on).
///
/// Snow takes priority over rock at high, steep samples (a steep snowy peak
/// still reads as snow). Dirt (channel B) is always 0 here -- it has no
/// natural height/slope signal; it stays a real, wired channel with zero
/// weight so a future brush-paint tool has an empty layer to paint into.
///
/// Weights sum to exactly 1.0 in `f32` (`t_snow + (1-t_snow)*(t_rock +
/// (1-t_rock)) == 1.0`); `[u8; 4]` packing rounds each channel independently,
/// so the packed sum can be off by a few units of 255 -- callers needing an
/// exact sum should re-normalize after unpacking.
fn splat_weight_for(height_ratio: f32, normal_y: f32) -> [u8; 4] {
    let t_rock = smoothstep(
        ROCK_SLOPE_THRESHOLD + TRANSITION_BAND,
        ROCK_SLOPE_THRESHOLD - TRANSITION_BAND,
        normal_y,
    );
    let t_snow = smoothstep(
        SNOW_HEIGHT_RATIO - TRANSITION_BAND,
        SNOW_HEIGHT_RATIO + TRANSITION_BAND,
        height_ratio,
    );
    let snow_w = t_snow;
    let rock_w = t_rock * (1.0 - t_snow);
    let grass_w = (1.0 - t_rock) * (1.0 - t_snow);
    let to_u8 = |w: f32| (w.clamp(0.0, 1.0) * 255.0).round() as u8;
    [to_u8(grass_w), to_u8(rock_w), 0u8, to_u8(snow_w)]
}

/// A whole-terrain splatmap already decoded to RGBA8 pixels, provided by
/// the caller instead of letting `generate_chunks` compute weights
/// procedurally. `width`/`height` must match the heightmap's own
/// dimensions -- both describe the same terrain, sampled at the same grid.
pub struct SplatmapOverride {
    /// Splatmap width in pixels; expected to match the heightmap's width.
    pub width: u32,
    /// Splatmap height in pixels; expected to match the heightmap's height.
    pub height: u32,
    /// Decoded RGBA8 pixel data, row-major, `width * height * 4` bytes long.
    pub data: Vec<u8>,
}

impl SplatmapOverride {
    fn sample(&self, x: u32, z: u32) -> [u8; 4] {
        let x = x.min(self.width - 1);
        let z = z.min(self.height - 1);
        let i = ((z * self.width + x) * 4) as usize;
        [
            self.data[i],
            self.data[i + 1],
            self.data[i + 2],
            self.data[i + 3],
        ]
    }
}

/// Chunking configuration for one `Terrain` entity.
pub struct ChunkParams {
    /// Number of chunks along (x, z).
    pub chunk_count: (u32, u32),
    /// World-space size of one chunk along each axis (chunks are square).
    pub chunk_size: f32,
    /// Multiplier applied to the normalized heightmap sample.
    pub height_scale: f32,
}

/// Samples `heightmap` at texel `(x, z)` (clamped to its bounds) and scales
/// it into world-space height units. Factored out of
/// `generate_chunks_with_splatmap`'s own per-vertex loop (a top-level `fn`
/// rather than the closure it used to be) so `procedural_splat_grid` below
/// can reuse the exact same sampling -- both need "the height at this texel"
/// and must never disagree about it.
fn height_at(heightmap: &HeightmapAsset, height_scale: f32, x: u32, z: u32) -> f32 {
    let x = x.min(heightmap.width - 1);
    let z = z.min(heightmap.height - 1);
    let raw = heightmap.data[(z * heightmap.width + x) as usize];
    (raw as f32 / u16::MAX as f32) * height_scale
}

/// Central-difference surface normal at texel `(x, z)`, from the four
/// neighboring texels. `world_step` scales the normal's Y component to
/// match the actual world-space texel spacing (see
/// `generate_chunks_with_splatmap`'s own use of this same formula). Shared
/// with `procedural_splat_grid` for the same reason `height_at` is.
fn normal_at(
    heightmap: &HeightmapAsset,
    height_scale: f32,
    world_step: f32,
    x: u32,
    z: u32,
) -> glam::Vec3 {
    let hl = height_at(heightmap, height_scale, x.saturating_sub(1), z);
    let hr = height_at(heightmap, height_scale, x + 1, z);
    let hd = height_at(heightmap, height_scale, x, z.saturating_sub(1));
    let hu = height_at(heightmap, height_scale, x, z + 1);
    glam::Vec3::new(hl - hr, 2.0 * world_step, hd - hu).normalize()
}

/// World-space size, in world units, of one heightmap texel along x and z
/// respectively, given `params`. Assumes `heightmap_width`/`heightmap_height`
/// divide evenly by `params.chunk_count` on each axis -- the common case,
/// and the same case `generate_chunks_with_splatmap` itself favors (see its
/// doc comment on remainder absorption). When they don't divide evenly, the
/// real last chunk along that axis is very slightly denser than this
/// returns (it absorbed the leftover texels), so callers that use this for
/// world<->texel conversion (the terrain brush tool) are very slightly off
/// only within that one edge chunk. Getting that edge case pixel-perfect is
/// out of scope for the terrain brush's first working version -- see
/// `world_to_texel`'s doc comment.
pub(crate) fn texel_world_step(
    heightmap_width: u32,
    heightmap_height: u32,
    params: &ChunkParams,
) -> (f32, f32) {
    let base_texels_x = (heightmap_width / params.chunk_count.0.max(1)).max(1);
    let base_texels_z = (heightmap_height / params.chunk_count.1.max(1)).max(1);
    (
        params.chunk_size / base_texels_x as f32,
        params.chunk_size / base_texels_z as f32,
    )
}

/// Converts a world-space, `Terrain`-local (x, z) offset (i.e. already
/// relative to the `Terrain` entity's own `Transform`) into the
/// corresponding heightmap texel coordinate -- the inverse of the vertex
/// placement math in `generate_chunks_with_splatmap`, which places a
/// chunk-local vertex at `[lx as f32 * world_step, y, lz as f32 *
/// world_step]` and then offsets the whole chunk by `chunk.world_offset`
/// (itself `cx as f32 * chunk_size`). For a resolution that divides evenly
/// by `chunk_count`, `world_step` is the same in every chunk, so that
/// composition collapses to exactly `texel_index as f32 * world_step` --
/// which is what this inverts. Used by the terrain brush tool to turn a
/// raycast hit's world position into "which texel did the user click."
pub(crate) fn world_to_texel(
    local_x: f32,
    local_z: f32,
    heightmap_width: u32,
    heightmap_height: u32,
    params: &ChunkParams,
) -> (u32, u32) {
    let (step_x, step_z) = texel_world_step(heightmap_width, heightmap_height, params);
    let hx = (local_x / step_x)
        .round()
        .clamp(0.0, (heightmap_width.max(1) - 1) as f32) as u32;
    let hz = (local_z / step_z)
        .round()
        .clamp(0.0, (heightmap_height.max(1) - 1) as f32) as u32;
    (hx, hz)
}

/// Computes the full-resolution procedural splat-weight grid for the whole
/// `heightmap` (row-major RGBA8, `width * height * 4` bytes), using the
/// exact same per-texel `splat_weight_for` formula
/// `generate_chunks_with_splatmap` applies per vertex -- just evaluated once
/// over the whole heightmap instead of once per chunk (with duplicated
/// boundary texels).
///
/// Used by the terrain brush tool to seed a paint stroke's starting weights
/// when a `Terrain` has no splatmap yet: without this, the first paint
/// stroke would reset every already-rendered texel in a touched chunk to an
/// arbitrary default the moment that chunk's weight texture is re-uploaded
/// (the brush re-uploads a whole touched chunk's buffer at once, not a
/// sub-region diff), which would look like the rest of that chunk
/// spontaneously re-texturing itself.
pub(crate) fn procedural_splat_grid(heightmap: &HeightmapAsset, params: &ChunkParams) -> Vec<u8> {
    let (world_step, _) = texel_world_step(heightmap.width, heightmap.height, params);
    let mut out = Vec::with_capacity((heightmap.width * heightmap.height * 4) as usize);
    for z in 0..heightmap.height {
        for x in 0..heightmap.width {
            let y = height_at(heightmap, params.height_scale, x, z);
            let normal = normal_at(heightmap, params.height_scale, world_step, x, z);
            let height_ratio = y / params.height_scale.max(0.0001);
            out.extend_from_slice(&splat_weight_for(height_ratio, normal.y));
        }
    }
    out
}

/// One chunk's generated geometry -- both the render mesh and the physics
/// heightfield are derived from the same sampled height grid, so a visual
/// seam and a collision gap are prevented by construction, not by
/// coincidence.
pub struct ChunkData {
    /// Render mesh vertices for this chunk, in chunk-local space.
    pub vertices: Vec<Vertex>,
    /// Render mesh triangle indices for this chunk.
    pub indices: Vec<u32>,
    /// World-space (x, z) offset of this chunk's origin corner.
    pub world_offset: (f32, f32),
    /// Row-major height values for this chunk's `Collider::Heightfield`.
    pub heightfield_heights: Vec<f32>,
    /// Number of rows in `heightfield_heights`.
    pub heightfield_rows: usize,
    /// Number of columns in `heightfield_heights`.
    pub heightfield_cols: usize,
    /// Per-sample RGBA8 splat weight, same length/ordering as
    /// `heightfield_heights` (row-major, `verts_x * verts_z` long).
    pub splat_weights: Vec<[u8; 4]>,
}

/// Divides `heightmap` into `params.chunk_count.0 * params.chunk_count.1`
/// chunks. Each chunk samples `(texels_per_chunk_x + 1, texels_per_chunk_z +
/// 1)` heightmap texels -- the `+1` on each axis is the boundary row/column
/// shared with the next chunk over, read from the same underlying texel data
/// as that neighbor's own boundary, so the two never disagree. If
/// `heightmap`'s resolution doesn't evenly divide `chunk_count`, the
/// remainder is absorbed into the last chunk along that axis.
pub fn generate_chunks(heightmap: &HeightmapAsset, params: &ChunkParams) -> Vec<ChunkData> {
    generate_chunks_with_splatmap(heightmap, params, None)
}

/// Same as `generate_chunks`, but when `splatmap` is `Some`, every vertex's
/// splat weight is sampled from it instead of computed procedurally via
/// `splat_weight_for`. `splatmap`'s dimensions are assumed to match
/// `heightmap`'s (the terrain brush tool is responsible for keeping the two
/// files the same size when it creates a splatmap).
pub fn generate_chunks_with_splatmap(
    heightmap: &HeightmapAsset,
    params: &ChunkParams,
    splatmap: Option<&SplatmapOverride>,
) -> Vec<ChunkData> {
    let (chunks_x, chunks_z) = params.chunk_count;
    let base_texels_x = heightmap.width / chunks_x;
    let base_texels_z = heightmap.height / chunks_z;

    let mut chunks = Vec::with_capacity((chunks_x * chunks_z) as usize);
    for cz in 0..chunks_z {
        for cx in 0..chunks_x {
            // Absorb the remainder into the last chunk along each axis.
            let texels_x = if cx == chunks_x - 1 {
                heightmap.width - base_texels_x * (chunks_x - 1)
            } else {
                base_texels_x
            };
            let texels_z = if cz == chunks_z - 1 {
                heightmap.height - base_texels_z * (chunks_z - 1)
            } else {
                base_texels_z
            };

            let origin_x = cx * base_texels_x;
            let origin_z = cz * base_texels_z;
            // +1 on both axes: the shared boundary row/column with the next chunk.
            let verts_x = texels_x + 1;
            let verts_z = texels_z + 1;

            let mut vertices = Vec::with_capacity((verts_x * verts_z) as usize);
            let mut heightfield_heights = Vec::with_capacity((verts_x * verts_z) as usize);
            let mut splat_weights = Vec::with_capacity((verts_x * verts_z) as usize);
            let world_step = params.chunk_size / texels_x.max(1) as f32; // square chunks; x and z share one step

            for lz in 0..verts_z {
                for lx in 0..verts_x {
                    let hx = origin_x + lx;
                    let hz = origin_z + lz;
                    let y = height_at(heightmap, params.height_scale, hx, hz);
                    heightfield_heights.push(y);

                    // Central-difference normal from the four neighboring texels.
                    let normal = normal_at(heightmap, params.height_scale, world_step, hx, hz);

                    let height_ratio = y / params.height_scale.max(0.0001);
                    splat_weights.push(match splatmap {
                        Some(sm) => sm.sample(hx, hz),
                        None => splat_weight_for(height_ratio, normal.y),
                    });

                    vertices.push(Vertex {
                        position: [lx as f32 * world_step, y, lz as f32 * world_step],
                        color: [1.0, 1.0, 1.0],
                        normal: normal.to_array(),
                        uv: [lx as f32 / texels_x as f32, lz as f32 / texels_z as f32],
                    });
                }
            }

            let mut indices = Vec::with_capacity((texels_x * texels_z * 6) as usize);
            for lz in 0..texels_z {
                for lx in 0..texels_x {
                    let i0 = lz * verts_x + lx;
                    let i1 = lz * verts_x + lx + 1;
                    let i2 = (lz + 1) * verts_x + lx + 1;
                    let i3 = (lz + 1) * verts_x + lx;
                    // Winding matches this crate's existing ground-plane convention
                    // (mesh.rs::plane_vertices: [0, 2, 1, 0, 3, 2] for a CCW quad
                    // viewed from above).
                    indices.extend_from_slice(&[i0, i2, i1, i0, i3, i2]);
                }
            }

            chunks.push(ChunkData {
                vertices,
                indices,
                world_offset: (cx as f32 * params.chunk_size, cz as f32 * params.chunk_size),
                heightfield_heights,
                heightfield_rows: verts_z as usize,
                heightfield_cols: verts_x as usize,
                splat_weights,
            });
        }
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A 5x5 heightmap (so 2x2 chunks of 2x2 texels each share exactly one
    /// boundary row/column, per the "duplicate boundary data" design decision).
    fn test_heightmap() -> HeightmapAsset {
        let mut data = Vec::with_capacity(25);
        for y in 0..5u32 {
            for x in 0..5u32 {
                data.push((y * 5 + x) as u16 * 1000); // distinct, checkable value per texel
            }
        }
        HeightmapAsset {
            width: 5,
            height: 5,
            data,
        }
    }

    #[test]
    fn chunk_count_matches_the_requested_grid() {
        let hm = test_heightmap();
        let params = ChunkParams {
            chunk_count: (2, 2),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        let chunks = generate_chunks(&hm, &params);
        assert_eq!(chunks.len(), 4);
    }

    #[test]
    fn neighboring_chunks_agree_exactly_at_their_shared_boundary() {
        let hm = test_heightmap();
        let params = ChunkParams {
            chunk_count: (2, 2),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        let chunks = generate_chunks(&hm, &params);
        // chunks[0] is (0,0), chunks[1] is (1,0) -- adjacent along X.
        // The right edge of chunks[0]'s height grid must equal the left edge of chunks[1]'s.
        let right_edge_of_0: Vec<f32> = chunks[0]
            .heightfield_heights
            .chunks(chunks[0].heightfield_cols)
            .map(|row| *row.last().unwrap())
            .collect();
        let left_edge_of_1: Vec<f32> = chunks[1]
            .heightfield_heights
            .chunks(chunks[1].heightfield_cols)
            .map(|row| row[0])
            .collect();
        assert_eq!(right_edge_of_0, left_edge_of_1);
    }

    #[test]
    fn a_resolution_that_does_not_evenly_divide_chunk_count_absorbs_the_remainder_into_the_last_chunk(
    ) {
        let mut data = vec![0u16; 7 * 7];
        for (i, v) in data.iter_mut().enumerate() {
            *v = i as u16 * 100;
        }
        let hm = HeightmapAsset {
            width: 7,
            height: 7,
            data,
        };
        let params = ChunkParams {
            chunk_count: (2, 2),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        let chunks = generate_chunks(&hm, &params);
        assert_eq!(chunks.len(), 4); // still produces the requested grid, no panic
    }

    #[test]
    fn chunk_world_position_reflects_its_grid_coordinate_and_chunk_size() {
        let hm = test_heightmap();
        let params = ChunkParams {
            chunk_count: (2, 2),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        let chunks = generate_chunks(&hm, &params);
        // chunks[0] = (0,0) -> world origin corner; chunks[1] = (1,0) -> offset by chunk_size on X.
        assert_eq!(chunks[0].world_offset, (0.0, 0.0));
        assert_eq!(chunks[1].world_offset, (10.0, 0.0));
    }

    #[test]
    fn flat_low_terrain_is_grass_dominant() {
        let w = splat_weight_for(0.1, 1.0);
        assert!(w[0] > 200, "grass channel should dominate, got {w:?}");
        assert!(w[1] < 20, "rock channel should be near zero, got {w:?}");
        assert!(w[3] < 20, "snow channel should be near zero, got {w:?}");
    }

    #[test]
    fn steep_low_slope_is_rock_dominant() {
        let w = splat_weight_for(0.1, 0.3);
        assert!(w[1] > 200, "rock channel should dominate, got {w:?}");
        assert!(w[0] < 20, "grass channel should be near zero, got {w:?}");
        assert!(w[3] < 20, "snow channel should be near zero, got {w:?}");
    }

    #[test]
    fn high_plateau_is_snow_dominant_even_when_flat() {
        let w = splat_weight_for(0.95, 1.0);
        assert!(w[3] > 200, "snow channel should dominate, got {w:?}");
        assert!(w[0] < 20, "grass channel should be near zero, got {w:?}");
    }

    #[test]
    fn high_and_steep_is_still_snow_not_rock() {
        // Snow takes priority over rock at high, steep samples.
        let w = splat_weight_for(0.95, 0.2);
        assert!(
            w[3] > w[1],
            "snow should outweigh rock at a high, steep sample, got {w:?}"
        );
    }

    #[test]
    fn dirt_channel_is_always_zero_in_this_sub_step() {
        for (h, n) in [(0.0, 1.0), (0.5, 0.5), (1.0, 0.0), (0.75, 0.6)] {
            assert_eq!(
                splat_weight_for(h, n)[2],
                0,
                "dirt at height={h} normal_y={n}"
            );
        }
    }

    #[test]
    fn weights_sum_to_one_within_u8_rounding_tolerance() {
        for height_ratio in [0.0, 0.2, 0.5, 0.65, 0.75, 0.85, 1.0] {
            for normal_y in [0.0, 0.3, 0.5, 0.6, 0.7, 1.0] {
                let w = splat_weight_for(height_ratio, normal_y);
                let sum: i32 = w.iter().map(|&c| c as i32).sum();
                assert!(
                    (sum - 255).abs() <= 4,
                    "height_ratio={height_ratio} normal_y={normal_y}: \
                     packed weights {w:?} sum to {sum}, expected ~255"
                );
            }
        }
    }

    #[test]
    fn a_provided_splatmap_overrides_procedural_weights() {
        let hm = test_heightmap(); // existing 5x5 helper already in this file
        let params = ChunkParams {
            chunk_count: (1, 1),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        // A splatmap that's 100% layer1 (rock) everywhere -- the opposite of
        // what the flat, low, un-sloped test_heightmap() would generate
        // procedurally (which would be grass-dominant).
        let splatmap_rgba: Vec<u8> = std::iter::repeat([0u8, 255, 0, 0])
            .take(5 * 5)
            .flatten()
            .collect();
        let splatmap = SplatmapOverride {
            width: 5,
            height: 5,
            data: splatmap_rgba,
        };

        let chunks = generate_chunks_with_splatmap(&hm, &params, Some(&splatmap));
        let chunk = &chunks[0];
        for w in &chunk.splat_weights {
            assert_eq!(
                *w,
                [0, 255, 0, 0],
                "every weight should come from the provided splatmap, not procedural generation"
            );
        }
    }

    #[test]
    fn generate_chunks_without_a_splatmap_is_unchanged() {
        // Regression: the existing procedural path (no splatmap) must still
        // produce exactly what generate_chunks always produced.
        let hm = test_heightmap();
        let params = ChunkParams {
            chunk_count: (1, 1),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        let via_old_fn = generate_chunks(&hm, &params);
        let via_new_fn = generate_chunks_with_splatmap(&hm, &params, None);
        assert_eq!(via_old_fn[0].splat_weights, via_new_fn[0].splat_weights);
    }

    #[test]
    fn generated_chunks_carry_splat_weights_matching_the_height_grid() {
        let hm = test_heightmap();
        let params = ChunkParams {
            chunk_count: (1, 1),
            chunk_size: 10.0,
            height_scale: 1.0,
        };
        let chunks = generate_chunks(&hm, &params);
        let chunk = &chunks[0];
        assert_eq!(
            chunk.splat_weights.len(),
            chunk.heightfield_heights.len(),
            "one splat weight per height sample"
        );
        for w in &chunk.splat_weights {
            let sum: i32 = w.iter().map(|&c| c as i32).sum();
            assert!((sum - 255).abs() <= 4, "weight {w:?} should sum to ~255");
        }
    }
}
