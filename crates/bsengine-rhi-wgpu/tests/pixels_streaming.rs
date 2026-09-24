//! Texture streaming, seen in pixels.
//!
//! `GpuTextureRegistry`'s own tests can read a streamed texture's residency
//! and footprint back off the GPU object, but the property that matters to a
//! player -- that what is *drawn* changes when a level comes in -- is only
//! visible by rendering. A registry that rebuilt the texture on a raise but
//! kept binding the old view would pass every footprint test and draw the
//! blurry stand-in forever; this file is what catches that.
//!
//! The texture is a checker of 2x2-texel blocks. Its mip chain is what makes
//! residency visible: level 1 (the 2x2 box filter of level 0) is a checker of
//! single texels, and level 2 averages that to flat grey. A streamed 256x256
//! texture starts at level 2 -- the first at or under 64 px -- so it draws
//! flat, and its first raise brings in the checker.

mod common;

use bsengine_core::TextureImportSettings;
use common::{Draw, Harness, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const SIZE: u32 = 256;

/// A checker whose cells are 2x2 texels, so level 1 of the chain is a
/// one-texel checker and level 2 is grey.
fn block_checker() -> Vec<u8> {
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let v = if (x / 2 + y / 2) % 2 == 0 { 255 } else { 0 };
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
    }
    rgba
}

/// The cube scaled until its front face overflows the frame: the camera is
/// 5 units back, so a 7-unit cube puts the face 1.5 units away and about
/// 60 texels of a 256-texel texture across the frame's height. Magnified
/// like that, the sampler reads the largest *resident* level, whichever it
/// is, at a few pixels per texel -- a checker of single texels shows as
/// blocks about five pixels wide, and grey shows as grey.
fn magnified_scene(cube: u64, texture: u64) -> Scene {
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)
            .textured(texture)
            .scaled(Vec3::splat(7.0), Vec3::ZERO)],
        ..Default::default()
    }
}

/// The 24x24 window at the centre of the frame -- wide enough to cover
/// several checker blocks whichever way the mesh runs U.
fn window() -> impl Iterator<Item = (u32, u32)> {
    let (cx, cy) = (WIDTH / 2, HEIGHT / 2);
    (cy - 12..cy + 12).flat_map(move |y| (cx - 12..cx + 12).map(move |x| (x, y)))
}

/// Mean and variance of luma (0..255) over [`window`].
fn variance(pixels: &Pixels) -> (f32, f32) {
    let samples: Vec<f32> = window().map(|(x, y)| pixels.luma(x, y)).collect();
    let mean = samples.iter().sum::<f32>() / samples.len() as f32;
    let var = samples.iter().map(|s| (s - mean).powi(2)).sum::<f32>() / samples.len() as f32;
    (mean, var)
}

/// The largest luma difference between two renders over [`window`].
fn max_difference(a: &Pixels, b: &Pixels) -> f32 {
    window()
        .map(|(x, y)| (a.luma(x, y) - b.luma(x, y)).abs())
        .fold(0.0, f32::max)
}

fn settings(streaming: bool) -> TextureImportSettings {
    TextureImportSettings {
        srgb: false,
        mipmaps: true,
        streaming,
        ..Default::default()
    }
}

/// A streamed texture draws its small levels first -- the checker, averaged
/// to grey -- shows the checker as soon as the next level is raised, and
/// once whole draws exactly what the unstreamed upload of the same texels
/// draws. That upload is the reference for what "the checker" looks like,
/// and the premise that it is visible at all.
///
/// Measured on lavapipe and WARP: the lit face spans luma 18..41, the whole
/// texture's checker has a variance around 330, the initial residency is
/// flat at 31 (variance 0), and the first raise shows a variance around 90
/// -- lower than the whole texture's because bilinear filtering across a
/// checker of *single* texels (level 1) is a triangle wave where the
/// 2x2-block checker of level 0 keeps flat plateaus. The thresholds sit
/// well inside those gaps.
#[test]
fn a_streamed_texture_draws_grey_first_and_the_checker_once_raised() {
    let mut h = Harness::new();
    let cube = h.cube();
    let checker = block_checker();
    let whole = h.texture_with(settings(false), SIZE, SIZE, &checker);
    let streamed = h.texture_with(settings(true), SIZE, SIZE, &checker);
    assert_eq!(
        h.residency(whole),
        None,
        "premise: the reference is not streamed"
    );
    assert_eq!(
        h.residency(streamed),
        Some((2, 9)),
        "premise: the streamed texture starts at the 64x64 level"
    );

    let reference = h.render(&magnified_scene(cube, whole));
    let (whole_mean, detail) = variance(&reference);
    assert!(
        whole_mean > 10.0,
        "premise: the face is lit and in frame (mean luma {whole_mean})"
    );
    assert!(
        detail > 50.0,
        "premise: the whole texture shows the checker (variance {detail})"
    );

    let (flat_mean, flat) = variance(&h.render(&magnified_scene(cube, streamed)));
    assert!(
        flat_mean > 10.0,
        "premise: the streamed face is lit and in frame (mean luma {flat_mean})"
    );
    assert!(
        flat < detail * 0.05,
        "at initial residency the checker has averaged to flat grey: \
         variance {flat} against {detail} for the whole texture"
    );

    assert!(h.raise_residency(streamed), "the 128x128 level comes in");
    assert_eq!(h.residency(streamed), Some((1, 9)));
    let (_, raised) = variance(&h.render(&magnified_scene(cube, streamed)));
    assert!(
        raised > detail * 0.15 && raised > flat * 10.0 + 10.0,
        "the raised level must be what is sampled now: variance {raised}, against {flat} \
         before the raise and {detail} for the whole texture. A raise that rebuilt the \
         texture but kept binding the old view would still draw grey"
    );

    assert!(h.raise_residency(streamed), "and the 256x256 level");
    assert_eq!(h.residency(streamed), Some((0, 9)), "fully resident");
    let full = h.render(&magnified_scene(cube, streamed));
    let difference = max_difference(&full, &reference);
    assert!(
        difference <= 1.0,
        "whole, a streamed texture draws what the unstreamed upload draws: \
         the largest luma difference is {difference}"
    );
}
