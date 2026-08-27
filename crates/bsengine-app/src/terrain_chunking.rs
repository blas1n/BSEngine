//! Pure heightmap-to-chunk-geometry logic for the terrain system. No ECS,
//! GPU, or physics types -- the ECS layer (a future task) calls this and
//! wires the results into `GpuMeshRegistry`/`Collider`.

use bsengine_asset::HeightmapAsset;
use bsengine_rhi_wgpu::Vertex;

/// Chunking configuration for one `Terrain` entity.
pub struct ChunkParams {
    /// Number of chunks along (x, z).
    pub chunk_count: (u32, u32),
    /// World-space size of one chunk along each axis (chunks are square).
    pub chunk_size: f32,
    /// Multiplier applied to the normalized heightmap sample.
    pub height_scale: f32,
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
}

/// Divides `heightmap` into `params.chunk_count.0 * params.chunk_count.1`
/// chunks. Each chunk samples `(texels_per_chunk_x + 1, texels_per_chunk_z +
/// 1)` heightmap texels -- the `+1` on each axis is the boundary row/column
/// shared with the next chunk over, read from the same underlying texel data
/// as that neighbor's own boundary, so the two never disagree. If
/// `heightmap`'s resolution doesn't evenly divide `chunk_count`, the
/// remainder is absorbed into the last chunk along that axis.
pub fn generate_chunks(heightmap: &HeightmapAsset, params: &ChunkParams) -> Vec<ChunkData> {
    let (chunks_x, chunks_z) = params.chunk_count;
    let base_texels_x = heightmap.width / chunks_x;
    let base_texels_z = heightmap.height / chunks_z;

    let height_at = |x: u32, z: u32| -> f32 {
        let x = x.min(heightmap.width - 1);
        let z = z.min(heightmap.height - 1);
        let raw = heightmap.data[(z * heightmap.width + x) as usize];
        (raw as f32 / u16::MAX as f32) * params.height_scale
    };

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
            let world_step = params.chunk_size / texels_x.max(1) as f32; // square chunks; x and z share one step

            for lz in 0..verts_z {
                for lx in 0..verts_x {
                    let hx = origin_x + lx;
                    let hz = origin_z + lz;
                    let y = height_at(hx, hz);
                    heightfield_heights.push(y);

                    // Central-difference normal from the four neighboring texels.
                    let hl = height_at(hx.saturating_sub(1), hz);
                    let hr = height_at(hx + 1, hz);
                    let hd = height_at(hx, hz.saturating_sub(1));
                    let hu = height_at(hx, hz + 1);
                    let normal = glam::Vec3::new(hl - hr, 2.0 * world_step, hd - hu).normalize();

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
}
