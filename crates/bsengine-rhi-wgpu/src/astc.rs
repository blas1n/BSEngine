//! ASTC 4x4 encoding, for the GPUs that have no BC: every phone.
//!
//! ARM's astcenc (through `ctt-astcenc`, built from C++ by `cc`) -- the
//! encoder Unity, Unreal and Godot ship for ASTC. Only a desktop host
//! compiles it: the machine that packages a game encodes its textures
//! (`texture::precook_mip_cache`), and a phone or a browser reads what was
//! made there or uploads uncompressed. Elsewhere [`encode`] is `None`, which
//! the registry takes as "no encoder here".
//!
//! Why 4x4 and not the 6x6 Unity defaults to: wgpu, following WebGPU,
//! refuses a compressed texture whose level 0 is not whole blocks, and a
//! power-of-two texture -- most textures -- is never a multiple of 6. 4x4 is
//! Godot's ASTC choice, and costs a byte a texel, as BC3 and BC7 do.

/// The quality setting: astcenc's "medium", the middle of its presets and
/// what its own command line documents for production colour textures. The
/// result is cached (mip cache, precooked package files), so the encode is
/// paid once per image as BC1/BC3's cluster fit is.
#[cfg(astc_encoder)]
const PRESET: ctt_astcenc::Preset = ctt_astcenc::Preset::Medium;

/// Block rows per parallel strip: 64 texel rows. Each strip is compressed by
/// its own single-threaded context. Blocks are independent, so a strip of
/// whole block rows compresses to exactly the bytes the same rows would in
/// one call (measured by `strips_encode_as_one_image`), and the strips run on
/// rayon like the BC encoders' block rows. astcenc's own parallel path
/// (`rayon::broadcast`) is not used: it needs the pool's size to be what it
/// was at context creation, and fails when called from inside a pool job.
#[cfg(astc_encoder)]
const STRIP_BLOCK_ROWS: u32 = 16;

/// One RGBA8 level in ASTC 4x4 blocks, row-major as the GPU reads them;
/// `srgb` picks the LDR sRGB profile, which a texture sampled through an
/// `…Srgb` view must be encoded with. A level smaller than a block (2x2,
/// 1x1) or not whole blocks is padded by astcenc itself. `None` where no
/// encoder is built, or if astcenc refuses the configuration.
#[cfg(astc_encoder)]
pub(crate) fn encode(width: u32, height: u32, rgba: &[u8], srgb: bool) -> Option<Vec<u8>> {
    use rayon::prelude::*;
    let profile = if srgb {
        ctt_astcenc::Profile::LdrSrgb
    } else {
        ctt_astcenc::Profile::Ldr
    };
    // No USE_DECODE_UNORM8: wgpu enables no ASTC decode-mode extension, so
    // a linear texture is decoded at fp16 precision, which is what the
    // encoder assumes without the flag. An sRGB one decodes to unorm8 by
    // the format's definition, whatever the flag says.
    let config = ctt_astcenc::config_init(
        profile,
        4,
        4,
        1,
        PRESET,
        ctt_astcenc::Flags::empty(),
    )
    .ok()?;
    let row_bytes = width.div_ceil(4) as usize * 16;
    let mut out = vec![0u8; row_bytes * height.div_ceil(4) as usize];
    let strip_bytes = row_bytes * STRIP_BLOCK_ROWS as usize;
    let ok = out
        .par_chunks_mut(strip_bytes)
        .enumerate()
        .all(|(index, blocks)| {
            let top = index as u32 * STRIP_BLOCK_ROWS * 4;
            let rows = (height - top).min(STRIP_BLOCK_ROWS * 4);
            let start = (top * width * 4) as usize;
            let end = start + (rows * width * 4) as usize;
            encode_strip(&config, width, rows, &rgba[start..end], blocks).is_some()
        });
    ok.then_some(out)
}

/// One strip of whole block rows (the last may be shorter) through its own
/// context.
#[cfg(astc_encoder)]
fn encode_strip(
    config: &ctt_astcenc::bindings::astcenc_config,
    width: u32,
    rows: u32,
    rgba: &[u8],
    blocks: &mut [u8],
) -> Option<()> {
    let mut context = ctt_astcenc::Context::new(config).ok()?;
    // astcenc takes a mutable pointer but only reads the image; a copy keeps
    // the caller's slice out of a `*mut` it never asked to lend.
    let mut texels = rgba.to_vec();
    let mut plane = texels.as_mut_ptr().cast::<std::ffi::c_void>();
    let mut image = ctt_astcenc::bindings::astcenc_image {
        dim_x: width,
        dim_y: rows,
        dim_z: 1,
        data_type: ctt_astcenc::bindings::astcenc_type_ASTCENC_TYPE_U8,
        data: &mut plane,
    };
    context
        .compress(&mut image, ctt_astcenc::Swizzle::IDENTITY, blocks)
        .ok()
}

/// No encoder in this build (a phone, a browser): see the module docs.
#[cfg(not(astc_encoder))]
pub(crate) fn encode(_width: u32, _height: u32, _rgba: &[u8], _srgb: bool) -> Option<Vec<u8>> {
    None
}

/// ASTC 4x4 blocks back to RGBA8, as a GPU would sample them -- for tests,
/// which can then measure the encode where no GPU in reach has ASTC (no
/// desktop GPU has it; Apple's do).
#[cfg(all(test, astc_encoder))]
pub(crate) fn decode(width: u32, height: u32, blocks: &[u8], srgb: bool) -> Vec<u8> {
    let profile = if srgb {
        ctt_astcenc::Profile::LdrSrgb
    } else {
        ctt_astcenc::Profile::Ldr
    };
    let config = ctt_astcenc::config_init(
        profile,
        4,
        4,
        1,
        PRESET,
        ctt_astcenc::Flags::DECOMPRESS_ONLY,
    )
    .expect("astcenc decode config");
    let mut context = ctt_astcenc::Context::new(&config).expect("astcenc decode context");
    let mut texels = vec![0u8; (width * height * 4) as usize];
    let mut plane = texels.as_mut_ptr().cast::<std::ffi::c_void>();
    let mut image = ctt_astcenc::bindings::astcenc_image {
        dim_x: width,
        dim_y: height,
        dim_z: 1,
        data_type: ctt_astcenc::bindings::astcenc_type_ASTCENC_TYPE_U8,
        data: &mut plane,
    };
    context
        .decompress(blocks, &mut image, ctt_astcenc::Swizzle::IDENTITY)
        .expect("astcenc decode");
    texels
}

#[cfg(all(test, astc_encoder))]
mod tests {
    use super::*;

    /// A 4-channel test image with structure in every channel: a diagonal
    /// colour ramp, a checker in blue and an alpha gradient, so an encode
    /// that drops or swaps a channel, or ignores alpha, is far off.
    fn pattern(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                out.push((x * 255 / width.max(1)) as u8);
                out.push((y * 255 / height.max(1)) as u8);
                out.push(if (x / 8 + y / 8) % 2 == 0 { 230 } else { 30 });
                out.push(((x + y) * 255 / (width + height).max(1)) as u8);
            }
        }
        out
    }

    /// Peak signal-to-noise ratio over every channel, in dB.
    fn psnr(a: &[u8], b: &[u8]) -> f64 {
        let mse = a
            .iter()
            .zip(b)
            .map(|(x, y)| (*x as f64 - *y as f64).powi(2))
            .sum::<f64>()
            / a.len() as f64;
        10.0 * (255.0f64 * 255.0 / mse.max(1e-9)).log10()
    }

    /// The encode decodes back to the image, per channel: the ramp, the
    /// checker and the alpha all survive. 40 dB is ASTC 4x4's territory on
    /// smooth content (an encode that zeroed alpha or swapped two channels
    /// lands near 10 dB) -- and the size is 16 bytes a block.
    #[test]
    fn an_encoded_level_decodes_back_to_the_image() {
        for srgb in [false, true] {
            let (w, h) = (128, 96);
            let image = pattern(w, h);
            let blocks = encode(w, h, &image, srgb).expect("encoded");
            assert_eq!(blocks.len(), (w / 4 * h / 4 * 16) as usize);
            let decoded = decode(w, h, &blocks, srgb);
            for channel in 0..4 {
                let pick = |v: &[u8]| v.iter().skip(channel).step_by(4).copied().collect::<Vec<_>>();
                let db = psnr(&pick(&image), &pick(&decoded));
                assert!(db > 40.0, "srgb {srgb}, channel {channel}: {db:.1} dB");
            }
        }
    }

    /// Strips are an implementation detail: an image taller than one strip
    /// encodes to the bytes one context makes of the whole -- so the strip
    /// offsets, and the rows each strip reads, line up. Several strips plus
    /// a shorter last one (200 rows = 64 + 64 + 64 + 8).
    #[test]
    fn strips_encode_as_one_image() {
        let (w, h) = (40, 200);
        let image = pattern(w, h);
        let striped = encode(w, h, &image, true).expect("encoded");
        assert!(h > STRIP_BLOCK_ROWS * 4 * 2, "the premise: several strips");
        let config = ctt_astcenc::config_init(
            ctt_astcenc::Profile::LdrSrgb,
            4,
            4,
            1,
            PRESET,
            ctt_astcenc::Flags::empty(),
        )
        .unwrap();
        let mut whole = vec![0u8; striped.len()];
        encode_strip(&config, w, h, &image, &mut whole).expect("whole image");
        assert!(striped == whole, "striped and whole-image encodes differ");
    }

    /// A chain's tail -- 2x2 and 1x1 levels -- is one block, padded by the
    /// encoder, and decodes to its texels.
    #[test]
    fn a_level_smaller_than_a_block_is_one_block() {
        let tiny = [200u8, 40, 90, 255, 10, 220, 60, 255, 90, 90, 200, 255, 250, 250, 20, 255];
        let blocks = encode(2, 2, &tiny, false).expect("encoded");
        assert_eq!(blocks.len(), 16);
        let decoded = decode(2, 2, &blocks, false);
        for (a, b) in tiny.iter().zip(&decoded) {
            assert!((*a as i32 - *b as i32).abs() <= 24, "{tiny:?} -> {decoded:?}");
        }
    }
}
