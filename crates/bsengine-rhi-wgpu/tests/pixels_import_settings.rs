//! Each texture import setting, seen in pixels.
//!
//! `GpuTextureRegistry`'s own tests can read a texture's format and mip count
//! back off the GPU object, but a sampler's filter and wrap mode leave no such
//! trace, and even the format only matters for what it does to a sampled
//! value. So every setting here is asserted the only way it can be: render
//! the same texture on the same cube with the setting on and off, and compare.
//!
//! **Orientation-agnostic and tint-agnostic on purpose.** Which way U runs
//! across the cube's front face is the mesh's business, and so is the
//! per-face vertex colour it carries -- which scales channels unequally (a
//! `(20, 20, 220)` texel came back as `(14, ., 32)`), so a red-versus-blue
//! comparison measures the tint as much as the texture. The wrap and filter
//! tests therefore use black and white texels, compare *luma*, and read the
//! *pattern* across the face -- how many times it flips, whether the middle
//! is a mix -- which comes out the same mirrored or tinted.

mod common;

use bsengine_core::{TextureFilter, TextureImportSettings, TextureWrap};
use common::{Draw, Harness, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

/// The cube scaled so its front face fills a comfortable part of the frame:
/// with the harness camera 5 units back and a 60 degree field of view, the
/// face spans about 110 pixels, centred.
const FACE_SCALE: f32 = 3.0;

fn face_scene(cube: u64, texture: u64) -> Scene {
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)
            .textured(texture)
            .scaled(Vec3::splat(FACE_SCALE), Vec3::ZERO)],
        ..Default::default()
    }
}

/// Pixel x of the point a fraction `t` of the way across the front face.
/// Deliberately a little inside the measured extent (columns 44..156), so
/// the samples stay clear of the face's edges.
fn face_x(t: f32) -> u32 {
    let half = 50.0;
    (WIDTH as f32 / 2.0 + (t * 2.0 - 1.0) * half).round() as u32
}

fn centre_y() -> u32 {
    HEIGHT / 2
}

fn black_and_white() -> Vec<u8> {
    let mut two = Vec::new();
    two.extend_from_slice(&WHITE);
    two.extend_from_slice(&BLACK);
    two
}

/// The lit cube face must be visible for any of this to mean anything.
///
/// Only for a render whose centre pixel is not a texel boundary: on a
/// black-and-white two-texel face the centre *is* the boundary, and under
/// Nearest it reads as whichever texel the sampler rounds to -- black, on
/// lavapipe. Those tests assert visibility on the band centres instead.
fn assert_face_visible(pixels: &Pixels, what: &str) {
    assert!(
        pixels.centre_luma() > 0.05,
        "premise: the textured face should be lit and in frame for {what}, centre luma {}",
        pixels.centre_luma()
    );
}

/// The luma range the two texels render at on this face, taken from the
/// two sample points and checked to be wide enough to tell apart.
fn texel_range(pixels: &Pixels, a: u32, b: u32) -> (f32, f32) {
    let (la, lb) = (pixels.luma(a, centre_y()), pixels.luma(b, centre_y()));
    let (lo, hi) = (la.min(lb), la.max(lb));
    assert!(
        hi - lo > 0.15,
        "premise: the white and black texels must render clearly apart; got {lo} and {hi}"
    );
    (lo, hi)
}

/// An sRGB-encoded 50% grey is 21.4% light; the same bytes read as linear
/// are 50% light. On screen -- whose output is sRGB-encoded again -- the
/// sRGB texture comes back as the mid grey it was painted, and the linear
/// misread comes back visibly brighter. The setting is the only thing that
/// differs between the two renders.
#[test]
fn srgb_decodes_a_colour_texture_darker_than_reading_it_as_linear() {
    let mut h = Harness::new();
    let cube = h.cube();
    let grey = [128u8, 128, 128, 255];
    let srgb = h.texture_with(
        TextureImportSettings {
            srgb: true,
            mipmaps: false,
            ..Default::default()
        },
        1,
        1,
        &grey,
    );
    let linear = h.texture_with(
        TextureImportSettings {
            srgb: false,
            mipmaps: false,
            ..Default::default()
        },
        1,
        1,
        &grey,
    );

    let as_srgb = h.render(&face_scene(cube, srgb));
    let as_linear = h.render(&face_scene(cube, linear));
    assert_face_visible(&as_linear, "the linear render");

    let (srgb_luma, linear_luma) = (as_srgb.centre_luma(), as_linear.centre_luma());
    assert!(
        srgb_luma < linear_luma * 0.75,
        "an sRGB texture must render darker than the same bytes read as linear: \
         sRGB {srgb_luma}, linear {linear_luma}"
    );
}

/// A 64x64 checkerboard of single texels on a face only a few pixels wide
/// is minified far below its resolution. Without mips every pixel samples
/// some texel and the face is a noisy mix of black and white; with them the
/// sampler reads a level that has already averaged to grey, and the face is
/// flat. The variance across the face is the difference.
#[test]
fn mipmaps_average_a_minified_checkerboard_into_a_flat_grey() {
    let mut h = Harness::new();
    let cube = h.cube();
    let mut checker = Vec::with_capacity(64 * 64 * 4);
    for y in 0..64 {
        for x in 0..64 {
            let v = if (x + y) % 2 == 0 { 255 } else { 0 };
            checker.extend_from_slice(&[v, v, v, 255]);
        }
    }
    let mipped = h.texture_with(
        TextureImportSettings {
            mipmaps: true,
            srgb: false,
            ..Default::default()
        },
        64,
        64,
        &checker,
    );
    let unmipped = h.texture_with(
        TextureImportSettings {
            mipmaps: false,
            srgb: false,
            ..Default::default()
        },
        64,
        64,
        &checker,
    );
    // A small cube: its face is about 9 pixels across, so the 64 texels
    // are minified roughly seven to one.
    let small = |texture| Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)
            .textured(texture)
            .scaled(Vec3::splat(0.25), Vec3::ZERO)],
        ..Default::default()
    };

    let variance = |pixels: &Pixels| -> f32 {
        let (cx, cy) = (WIDTH / 2, HEIGHT / 2);
        let samples: Vec<f32> = (cy - 2..=cy + 2)
            .flat_map(|y| (cx - 2..=cx + 2).map(move |x| (x, y)))
            .map(|(x, y)| pixels.luma(x, y))
            .collect();
        let mean = samples.iter().sum::<f32>() / samples.len() as f32;
        samples.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / samples.len() as f32
    };

    let noisy = variance(&h.render(&small(unmipped)));
    let flat = variance(&h.render(&small(mipped)));
    assert!(
        noisy > 0.005,
        "premise: without mips the minified checker must alias (variance {noisy})"
    );
    assert!(
        flat < noisy * 0.25,
        "with mips the same face must be nearly flat: variance {flat} vs {noisy} without"
    );
}

/// A two-texel texture across a face whose UVs run 0..2. Repeating, the
/// face shows the two texels twice -- four bands, three flips between dark
/// and light. Clamped, everything past u = 1 is the edge texel -- two
/// bands, one flip. Counted from the band centres, so it reads the same
/// however the mesh orients U.
#[test]
fn repeat_tiles_past_uv_one_where_clamp_stretches_the_edge() {
    let mut h = Harness::new();
    let cube = h.cube_uv_scaled(2.0);
    let two = black_and_white();
    let repeat = h.texture_with(
        TextureImportSettings {
            wrap: TextureWrap::Repeat,
            filter: TextureFilter::Nearest,
            srgb: false,
            mipmaps: false,
            streaming: false,
        },
        2,
        1,
        &two,
    );
    let clamp = h.texture_with(
        TextureImportSettings {
            wrap: TextureWrap::Clamp,
            filter: TextureFilter::Nearest,
            srgb: false,
            mipmaps: false,
            streaming: false,
        },
        2,
        1,
        &two,
    );

    let tiled = h.render(&face_scene(cube, repeat));
    let stretched = h.render(&face_scene(cube, clamp));

    // No centre-pixel visibility check: the centre of this face is u = 1,
    // a texel boundary, and under Nearest which texel that pixel shows is
    // the sampler's rounding -- see `nearest_keeps_a_hard_texel_edge...`
    // for the run where lavapipe read it as black. The premise inside
    // `flips` -- both texels visible at the band centres, well apart -- is
    // the one this test actually needs.
    //
    // Eight band centres across the face; light or dark relative to the
    // midpoint of what the two texels render at on this face.
    let flips = |pixels: &Pixels| -> usize {
        let lumas: Vec<f32> = (0..8)
            .map(|b| pixels.luma(face_x((b as f32 + 0.5) / 8.0), centre_y()))
            .collect();
        let (lo, hi) = lumas
            .iter()
            .fold((f32::INFINITY, f32::NEG_INFINITY), |(lo, hi), &l| {
                (lo.min(l), hi.max(l))
            });
        assert!(
            hi - lo > 0.15,
            "premise: both texels must show across the face; lumas {lumas:?}"
        );
        let mid = (lo + hi) / 2.0;
        let light: Vec<bool> = lumas.iter().map(|&l| l > mid).collect();
        light.windows(2).filter(|w| w[0] != w[1]).count()
    };

    assert_eq!(
        flips(&tiled),
        3,
        "u in 0..2 over a two-texel texture repeats into four bands"
    );
    assert_eq!(
        flips(&stretched),
        1,
        "clamped, everything past u = 1 is the edge texel: two bands"
    );
}

/// The boundary between the two texels lies across the middle of the face.
/// Bilinear filtering blends the two over a whole texel's width -- half the
/// face -- so columns near the boundary are a mix; nearest sampling keeps a
/// hard edge, so the same columns are one texel or the other.
#[test]
fn nearest_keeps_a_hard_texel_edge_that_linear_blends() {
    let mut h = Harness::new();
    let cube = h.cube();
    let two = black_and_white();
    let linear = h.texture_with(
        TextureImportSettings {
            filter: TextureFilter::Linear,
            wrap: TextureWrap::Clamp,
            srgb: false,
            mipmaps: false,
            streaming: false,
        },
        2,
        1,
        &two,
    );
    let nearest = h.texture_with(
        TextureImportSettings {
            filter: TextureFilter::Nearest,
            wrap: TextureWrap::Clamp,
            srgb: false,
            mipmaps: false,
            streaming: false,
        },
        2,
        1,
        &two,
    );

    let blended = h.render(&face_scene(cube, linear));
    let hard = h.render(&face_scene(cube, nearest));

    // No centre-pixel visibility check here, and not by oversight: on this
    // face the centre pixel *is* the texel boundary, and under Nearest which
    // texel it lands on is the sampler's rounding -- lavapipe on the Ubuntu
    // runner picked black (luma 0) where the Windows GPU picked white, and
    // the premise failed on a fully visible face. `texel_range` is the right
    // premise: both texels must show, well inside their halves, and apart.
    //
    // What the two texels render at on this face, from well inside each
    // half of the nearest render, and the midpoint that tells them apart.
    let (lo, hi) = texel_range(&hard, face_x(0.15), face_x(0.85));
    let mid = (lo + hi) / 2.0;
    let span = hi - lo;

    // Where the texel edge falls is the mesh's business -- which way U runs,
    // where the face sits -- so find it rather than assume it: the first
    // column across the face that is on the other side of `mid` from the
    // column before it, on the nearest render, where the edge is sharp.
    let start = face_x(0.05);
    let light: Vec<bool> = (start..=face_x(0.95))
        .map(|x| hard.luma(x, centre_y()) > mid)
        .collect();
    let edge = light
        .windows(2)
        .position(|w| w[0] != w[1])
        .map(|i| start + i as u32 + 1)
        .expect("premise: the two texels meet somewhere across the face");

    // Four pixels either side of the edge, not the edge itself: the pixel
    // the texel boundary runs through is resolved from samples on both sides
    // of it (the surface is multisampled) and reads as a blend under *any*
    // filter. Four pixels out, nearest is squarely on one texel, while
    // linear -- whose blend spans the middle half of the face, some 27
    // pixels each way -- is still nearly an even mix.
    for (side, x) in [("left", edge - 4), ("right", edge + 4)] {
        let l = hard.luma(x, centre_y());
        let off_extreme = (l - lo).abs().min((hi - l).abs());
        assert!(
            off_extreme < span * 0.2,
            "nearest: the {side} column (x = {x}) must be one texel, unblended; luma {l} \
             with texels at {lo} and {hi}"
        );
    }
    for (side, x) in [("left", edge - 4), ("right", edge + 4)] {
        let l = blended.luma(x, centre_y());
        assert!(
            (l - mid).abs() < span * 0.2,
            "linear: the {side} column (x = {x}) must still be inside the blend; luma {l} \
             against a midpoint of {mid}"
        );
    }
}
