//! Where each renderer's lightmap goes: a rectangle of one atlas layer.
//!
//! A renderer's own `uv2` fills `[0,1]²` (`lightmap_uv::unwrap`, or the
//! model's `TEXCOORD_1`), so it only needs a square of the right size, and the
//! scene's lightmap is those squares packed. Size follows the renderer's world
//! surface area at a texel density, times its `resolution_scale` -- Unity's
//! "Lightmap Resolution" times "Scale In Lightmap", Godot's texel scale -- so
//! a texel covers about the same world area everywhere.

/// Texels left empty between two rectangles: what bilinear filtering and the
/// dilation read past a rectangle's edge, so neighbours never bleed into each
/// other.
pub const PADDING: u32 = 2;
/// The smallest square a renderer gets, however small it is: below this the
/// unwrap's charts and gutters no longer resolve at all.
pub const MIN_SIDE: u32 = 4;

/// One renderer's place in the atlas, in texels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub layer: u32,
    pub x: u32,
    pub y: u32,
    pub side: u32,
}

impl Placement {
    /// The `[scale_x, scale_y, offset_x, offset_y]` that moves the renderer's
    /// `uv2` into this rectangle, for its `Lightmap` component.
    pub fn scale_offset(&self, layer_size: u32) -> [f32; 4] {
        let s = layer_size as f32;
        [
            self.side as f32 / s,
            self.side as f32 / s,
            self.x as f32 / s,
            self.y as f32 / s,
        ]
    }
}

/// The packing, and the texel density it was achieved at.
#[derive(Debug, Clone, PartialEq)]
pub struct Packing {
    pub placements: Vec<Placement>,
    pub layers: u32,
    /// Texels per world unit actually used: the requested density, or less if
    /// the scene did not fit in `max_layers` at it.
    pub texels_per_unit: f32,
}

/// Packs one square per renderer. `areas[i]` is renderer `i`'s world surface
/// area and `scales[i]` its `resolution_scale`.
pub fn pack(
    areas: &[f32],
    scales: &[f32],
    texels_per_unit: f32,
    layer_size: u32,
    max_layers: u32,
) -> Packing {
    let mut density = texels_per_unit;
    // Each attempt that overflows shrinks the density by a fifth; the
    // minimum side guarantees termination (at worst everything is MIN_SIDE).
    for _ in 0..64 {
        if let Some((placements, layers)) = try_pack(areas, scales, density, layer_size, max_layers)
        {
            return Packing {
                placements,
                layers,
                texels_per_unit: density,
            };
        }
        density *= 0.8;
    }
    let (placements, layers) = try_pack(areas, scales, 0.0, layer_size, u32::MAX)
        .expect("minimum-size squares always pack given unlimited layers");
    Packing {
        placements,
        layers,
        texels_per_unit: 0.0,
    }
}

fn side_for(area: f32, scale: f32, density: f32, layer_size: u32) -> u32 {
    let side = (area.max(0.0).sqrt() * density * scale.max(0.0)).ceil() as u32;
    side.clamp(MIN_SIDE, layer_size - 2 * PADDING)
}

/// Shelf packing, largest first; `None` when more than `max_layers` would be
/// needed.
fn try_pack(
    areas: &[f32],
    scales: &[f32],
    density: f32,
    layer_size: u32,
    max_layers: u32,
) -> Option<(Vec<Placement>, u32)> {
    let sides: Vec<u32> = areas
        .iter()
        .zip(scales)
        .map(|(&a, &s)| side_for(a, s, density, layer_size))
        .collect();
    let mut order: Vec<usize> = (0..sides.len()).collect();
    // Largest first, index as the tie-break: deterministic.
    order.sort_by(|&a, &b| sides[b].cmp(&sides[a]).then(a.cmp(&b)));

    let mut placements = vec![
        Placement {
            layer: 0,
            x: 0,
            y: 0,
            side: 0
        };
        sides.len()
    ];
    let (mut layer, mut x, mut y, mut row) = (0u32, PADDING, PADDING, 0u32);
    for &i in &order {
        let side = sides[i];
        if x + side + PADDING > layer_size {
            x = PADDING;
            y += row + PADDING;
            row = 0;
        }
        if y + side + PADDING > layer_size {
            layer += 1;
            x = PADDING;
            y = PADDING;
            row = 0;
        }
        if layer >= max_layers {
            return None;
        }
        placements[i] = Placement { layer, x, y, side };
        x += side + PADDING;
        row = row.max(side);
    }
    let layers = if sides.is_empty() { 0 } else { layer + 1 };
    Some((placements, layers))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn overlaps(a: &Placement, b: &Placement) -> bool {
        a.layer == b.layer
            && a.x < b.x + b.side + PADDING
            && b.x < a.x + a.side + PADDING
            && a.y < b.y + b.side + PADDING
            && b.y < a.y + a.side + PADDING
    }

    /// Thirty renderers of assorted sizes: no two rectangles (with their
    /// padding) overlap, all stay inside a layer, and a renderer with four
    /// times the area gets twice the side.
    #[test]
    fn rectangles_never_overlap_and_follow_area() {
        let areas: Vec<f32> = (0..30).map(|i| 1.0 + (i % 7) as f32 * 3.0).collect();
        let scales = vec![1.0; 30];
        let p = pack(&areas, &scales, 16.0, 256, 8);
        for (i, a) in p.placements.iter().enumerate() {
            assert!(a.x >= PADDING && a.y >= PADDING, "{i}: {a:?}");
            assert!(
                a.x + a.side + PADDING <= 256 && a.y + a.side + PADDING <= 256,
                "{i}: {a:?}"
            );
            for (j, b) in p.placements.iter().enumerate().skip(i + 1) {
                assert!(!overlaps(a, b), "{i} {a:?} and {j} {b:?} overlap");
            }
        }
        let q = pack(&[1.0, 4.0], &[1.0, 1.0], 16.0, 1024, 4);
        assert_eq!(q.placements[1].side, 2 * q.placements[0].side);
    }

    /// `resolution_scale` doubles a renderer's side; zero gives the minimum.
    #[test]
    fn resolution_scale_scales_the_side() {
        let p = pack(&[4.0, 4.0, 4.0], &[1.0, 2.0, 0.0], 8.0, 1024, 4);
        assert_eq!(p.placements[1].side, 2 * p.placements[0].side);
        assert_eq!(p.placements[2].side, MIN_SIDE);
    }

    /// A scene too big for the layers at the requested density is packed at
    /// a lower one rather than refused, and says which.
    #[test]
    fn an_oversized_scene_lowers_the_density_to_fit() {
        let areas = vec![100.0; 40];
        let scales = vec![1.0; 40];
        let p = pack(&areas, &scales, 64.0, 256, 2);
        assert!(p.layers <= 2);
        assert!(
            p.texels_per_unit < 64.0,
            "density was lowered: {}",
            p.texels_per_unit
        );
        for (i, a) in p.placements.iter().enumerate() {
            for b in p.placements.iter().skip(i + 1) {
                assert!(!overlaps(a, b));
            }
        }
    }

    #[test]
    fn scale_offset_maps_uv2_onto_the_rectangle() {
        let p = Placement {
            layer: 1,
            x: 256,
            y: 512,
            side: 128,
        };
        assert_eq!(p.scale_offset(1024), [0.125, 0.125, 0.25, 0.5]);
    }
}
