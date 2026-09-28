//! Block-compressed textures, measured at the pixel.
//!
//! The claim is two-sided and both sides are asserted: a BC1 upload takes a
//! fraction of the memory an RGBA8 one does *and* draws the same picture.
//! Either alone is easy to pass by accident -- a registry that quietly
//! ignored the setting draws the same picture at the same size, and one
//! that uploaded garbage in a compressed format is small and wrong.
//!
//! The fixture is chosen so that "the same picture" can be *exact*: a
//! checker whose cells are whole 4x4 blocks of pure black and pure white.
//! BC1 stores two RGB565 endpoints per block and picks each texel from
//! them, and a block that is one colour whose channels are 0 or 255 is
//! reproduced without loss. A photograph would compress with error and a
//! tolerance would be needed; here a tolerance would only hide a bug.

mod common;

use bsengine_core::{TextureCompression, TextureImportSettings};
use common::{Draw, Harness, Pixels, Scene, HEIGHT, WIDTH};
use glam::Vec3;

const SIZE: u32 = 64;

/// 4x4 blocks of pure black and pure white, alternating.
fn block_checker() -> Vec<u8> {
    let mut rgba = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let on = ((x / 4) + (y / 4)) % 2 == 0;
            let v = if on { 255 } else { 0 };
            rgba.extend_from_slice(&[v, v, v, 255]);
        }
    }
    rgba
}

fn settings(compression: TextureCompression) -> TextureImportSettings {
    TextureImportSettings {
        mipmaps: false,
        srgb: false,
        compression,
        ..TextureImportSettings::raw()
    }
}

/// The cube's front face square to the camera and spanning about 110 of
/// the frame's 200 pixels -- the same framing `pixels_import_settings`
/// measures its settings on -- so each 4-texel checker cell is drawn about
/// seven pixels wide and the middle third of the frame is all face.
fn scene(cube: u64, texture: u64) -> Scene {
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)
            .scaled(Vec3::splat(3.0), Vec3::ZERO)
            .textured(texture)],
        ..Scene::default()
    }
}

fn max_difference(a: &Pixels, b: &Pixels) -> u8 {
    let mut worst = 0u8;
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let (pa, pb) = (a.at(x, y), b.at(x, y));
            for ch in 0..3 {
                worst = worst.max(pa[ch].abs_diff(pb[ch]));
            }
        }
    }
    worst
}

#[test]
fn a_bc1_texture_draws_the_same_picture_at_an_eighth_of_the_memory() {
    let mut h = Harness::new();
    assert!(
        h.bc_supported(),
        "premise: this device must have block compression, or the test measures the fallback"
    );
    let cube = h.cube();
    let checker = block_checker();
    let plain = h.texture_with(settings(TextureCompression::None), SIZE, SIZE, &checker);
    let bc1 = h.texture_with(settings(TextureCompression::Bc1), SIZE, SIZE, &checker);

    let (fmt_plain, _) = h.gpu_shape(plain).unwrap();
    let (fmt_bc1, _) = h.gpu_shape(bc1).unwrap();
    assert_eq!(fmt_plain, wgpu::TextureFormat::Rgba8Unorm);
    assert_eq!(
        fmt_bc1,
        wgpu::TextureFormat::Bc1RgbaUnorm,
        "the setting must reach the GPU format"
    );
    let bytes_plain = h.gpu_footprint(plain).unwrap().2;
    let bytes_bc1 = h.gpu_footprint(bc1).unwrap().2;
    assert_eq!(
        bytes_plain,
        (SIZE * SIZE * 4) as u64,
        "premise: four bytes a texel"
    );
    assert_eq!(
        bytes_bc1 * 8,
        bytes_plain,
        "BC1 is half a byte a texel: {bytes_bc1} against {bytes_plain}"
    );

    let reference = h.render(&scene(cube, plain));
    let compressed = h.render(&scene(cube, bc1));

    // Premise: the picture has both colours in it, so "the same" means
    // something. Counted over the middle third of the frame, which is all
    // face, rather than at one pixel. Read in the red channel: the harness
    // cube's front face is lit with a red tint (an untextured one renders
    // about (122, 36, 36) at the centre), so a white texel is bright in red
    // and never in green -- a green threshold counted the whole face as
    // dark and nothing as bright, on the first run of this test.
    let (mut dark, mut bright) = (0u32, 0u32);
    for y in (HEIGHT / 3)..(2 * HEIGHT / 3) {
        for x in (WIDTH / 3)..(2 * WIDTH / 3) {
            let red = reference.at(x, y)[0];
            if red < 30 {
                dark += 1;
            } else if red > 80 {
                bright += 1;
            }
        }
    }
    assert!(
        dark > 100 && bright > 100,
        "premise: the checker is on screen in both colours ({dark} dark, {bright} bright)"
    );

    let difference = max_difference(&reference, &compressed);
    assert!(
        difference <= 1,
        "a block-aligned black and white checker compresses without loss, so the BC1 \
         upload must draw exactly what the RGBA8 one draws; the largest channel \
         difference was {difference}"
    );
}

/// BC3 carries an alpha channel; the same checker with alpha 255 everywhere
/// draws the same and takes a quarter of the memory.
#[test]
fn a_bc3_texture_draws_the_same_picture_at_a_quarter_of_the_memory() {
    let mut h = Harness::new();
    assert!(
        h.bc_supported(),
        "premise: block compression on this device"
    );
    let cube = h.cube();
    let checker = block_checker();
    let plain = h.texture_with(settings(TextureCompression::None), SIZE, SIZE, &checker);
    let bc3 = h.texture_with(settings(TextureCompression::Bc3), SIZE, SIZE, &checker);
    assert_eq!(
        h.gpu_shape(bc3).map(|s| s.0),
        Some(wgpu::TextureFormat::Bc3RgbaUnorm)
    );
    assert_eq!(
        h.gpu_footprint(bc3).unwrap().2 * 4,
        h.gpu_footprint(plain).unwrap().2,
        "BC3 is a byte a texel"
    );
    let difference = max_difference(&h.render(&scene(cube, plain)), &h.render(&scene(cube, bc3)));
    assert!(difference <= 1, "largest channel difference {difference}");
}
