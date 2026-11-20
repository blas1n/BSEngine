# ctt

[![docs.rs](https://img.shields.io/docsrs/ctt)](https://docs.rs/ctt)

A Rust library with [C bindings](#c-api) and CLI for GPU texture compression. ctt provides a unified interface over multiple encoder backends.

## Encoders

ctt binds to established open-source compression libraries and exposes them through a common `Encoder` trait. Each backend is compiled as an optional feature and can be enabled independently.

| Backend | Prefix | Feature | Description |
|---------|--------|---------|-------------|
| [**bc7enc-rdo**](https://github.com/richgel999/bc7enc_rdo) | `bc7e_` | `encoder-bc7enc` | Perceptual BC7 encoder with RDO support. |
| [**BC7F**](https://github.com/BinomialLLC/basis_universal) | `bc7f_` | `encoder-bc7f` | Portable analytical BC7 encoder from Basis Universal. |
| [**Intel ISPC Texture Compressor**](https://github.com/GameTechDev/ISPCTextureCompressor) | `intel_` | `encoder-intel` | SIMD-optimized BCn and ETC encoder. |
| [**etcpak**](https://github.com/wolfpld/etcpak) | `etcpak_` | `encoder-etcpak` | Fast ETC/EAC and BCn encoder. |
| [**AMD Compressonator**](https://github.com/GPUOpen-Tools/compressonator) | `amd_` | `encoder-amd` | AMD's BCn encoder suite. |
| [**astcenc**](https://github.com/ARM-software/astc-encoder) | `astcenc_` | `encoder-astcenc` | ARM ASTC encoder. |

Encoders are listed in priority order — when multiple encoders support the same format (e.g. BC7), the first one in the table is used. To override this, prefix the format with an encoder name (e.g. `intel_bc7`).

Use `ctt --list-encoders` to see what's available in your build:

```text
$ ctt --list-encoders
Encoder    Priority     Formats
-------    --------     -------
bc7e       1            bc7
bc7f       2            bc7
intel      3            bc1, bc3, bc4, bc5, bc6h, bc7, etc1
etcpak     4            etc1, etc2_rgba, eac_r, eac_rg, bc1, bc3, bc4, bc5
amd        5            bc1, bc2, bc3, bc4, bc4s, bc5, bc5s, bc6h, bc6hsf, bc7
astcenc    6            astc
```

## Formats

| Format | CLI name | Description |
|--------|----------|-------------|
| BC1 | `bc1` | RGB with 1-bit alpha. Opaque textures and simple cutouts. |
| BC2 | `bc2` | RGBA with explicit 4-bit alpha. Sharp alpha transitions. |
| BC3 | `bc3` | RGBA with interpolated alpha. General-purpose transparency. |
| BC4 | `bc4` | Single channel. Grayscale, heightmaps, roughness. |
| BC4S | `bc4s` | Single channel, signed. |
| BC5 | `bc5` | Two channels. Normal maps. |
| BC5S | `bc5s` | Two channels, signed. |
| BC6H | `bc6h` | HDR half-float RGB. Environment maps, HDR textures. |
| BC6H SF | `bc6hsf` | HDR signed float RGB. |
| BC7 | `bc7` | High-quality RGBA. Best LDR quality, supports alpha. |
| ETC1 | `etc1` | Mobile-friendly RGB. |
| ETC2 RGBA | `etc2_rgba` | Mobile-friendly RGBA. |
| EAC R | `eac_r` | Single channel 11-bit. |
| EAC RG | `eac_rg` | Two channel 11-bit. |
| ASTC | `astc_WxH` | Adaptive scalable texture compression. Variable block sizes (4x4 to 12x12). |

Compression accepts a quality preset from `ultra-fast` to `very-slow` (default `basic`); each encoder maps presets onto its native effort levels.

Uncompressed formats are also supported using WebGPU names (e.g. `rgba8unorm`, `rgb10a2unorm`) or Vulkan names (e.g. `r8g8b8a8_unorm`, `e5b9g9r9_ufloat_pack32`).

## Output containers

The container format is inferred from the output file extension (`.ktx2` or `.dds`), or can be set explicitly with `--container`.

- **KTX2** — Khronos cross-platform container. Supports all formats. Optional zstd or zlib supercompression.
- **DDS** — DirectX standard. Does not support ETC/EAC or ASTC formats.

## Installation

```sh
# Install the CLI (includes all encoders)
cargo install ctt-cli

# Or add the library to your project
cargo add ctt
```

By default the library enables all encoders. To select specific encoders:

```sh
cargo add ctt --no-default-features --features encoder-bc7enc,encoder-intel,ispc-prebuilt
```

## Library usage

The library API mirrors the CLI. Borrow your pixel bytes in a `SurfaceRef`, wrap it in an `ImageRef` with a `FormatDesc` that tells how to read the bytes, and call `convert`. An owned `Image` (for example, from a decoder) gives an `ImageRef` with `Image::to_ref`.

```rust,no_run
use ctt::{convert, ConvertSettings, Container, TargetFormat, Format};
use ctt::{FormatDesc, ImageRef, SurfaceRef, ColorSpace, AlphaMode, TextureKind};
use ctt::encoders::Encoder;

fn main() -> Result<(), ctt::Error> {
    let pixel_bytes = vec![0u8; 512 * 512 * 4];
    let surface = SurfaceRef {
        data: &pixel_bytes,
        width: 512,
        height: 512,
        depth: 1,
        stride: 512 * 4,
        slice_stride: 0,
    };

    let image = ImageRef {
        surfaces: vec![vec![surface]],
        kind: TextureKind::Texture2D,
        desc: FormatDesc {
            format: Format::R8G8B8A8_SRGB,
            color_space: ColorSpace::Srgb,
            alpha: AlphaMode::Straight,
        },
    };

    let _ktx2_bytes = convert(image, ConvertSettings {
        format: Some(TargetFormat::Compressed {
            format: Format::BC7_SRGB_BLOCK,
            encoder: Encoder::Auto,
        }),
        container: Container::ktx2(),
        ..Default::default()
    })?;

    Ok(())
}
```

See the [API documentation](https://docs.rs/ctt) for the full `ConvertSettings` options and the lower-level pipeline API.

### Parallel compression

The Rust library is serial unless its default-off `rayon` feature is enabled. With that feature, each compressed surface is split into block-row jobs on the active Rayon pool. `ctt` does not configure Rayon's global pool or add a thread count to `ConvertSettings`; callers that need a fixed worker budget can install conversion into their own pool:

```rust,ignore
let pool = rayon::ThreadPoolBuilder::new().num_threads(4).build()?;
let output = pool.install(|| ctt::convert(image, settings))?;
```

Without an explicit `install`, Rayon uses its lazily initialized global pool.

## C API

C bindings are available as a separate crate. See [`crates/ctt-c-api/README.md`](crates/ctt-c-api/README.md) for build, link, and usage instructions; the full API reference lives in the generated header at `crates/ctt-c-api/include/ctt.h`.

## CLI usage

```text
ctt <INPUT>... --output <PATH> [--format <FORMAT>] [OPTIONS]
```

When `--format` is omitted the input format is preserved without compression.

### Examples

Compress to BC7 (auto-selects bc7enc-rdo when available):

```sh
ctt diffuse.png -o diffuse.ktx2 -f bc7
```

Force the Intel ISPC encoder for BC7:

```sh
ctt diffuse.png -o diffuse.ktx2 -f intel_bc7
```

Normal map to BC5 as DDS:

```sh
ctt normal.png -o normal.dds -f bc5 --input-color-space linear
```

High quality:

```sh
ctt diffuse.png -o diffuse.ktx2 -f bc7 --quality slow
```

Choose the compression worker count with `--threads` / `-t`. Zero (the default) uses Rayon's platform default, one is effectively serial, and positive values request that exact number of workers:

```sh
ctt diffuse.png -o diffuse.ktx2 -f bc7 --threads 4
```

Cubemap from a cross layout. `--cubemap-layout cross` (the default) accepts
both the horizontal 4×3 and vertical 3×4 cross arrangements; `--cubemap-layout
strip` accepts a horizontal 6×1 strip:

```sh
ctt skybox_cross.png -o skybox.ktx2 -f bc6h --cubemap --cubemap-layout cross
```

Cubemap from six separate faces (order: +X, -X, +Y, -Y, +Z, -Z):

```sh
ctt px.png nx.png py.png ny.png pz.png nz.png -o skybox.ktx2 -f bc7 --cubemap
```

Cubemap array from N×6 face images. Pass a multiple of six inputs with
`--cubemap` and every group of six becomes one cube, stacked in argv order
(you can also pass several already-assembled cubemaps):

```sh
ctt px0.png nx0.png py0.png ny0.png pz0.png nz0.png \
    px1.png nx1.png py1.png ny1.png pz1.png nz1.png \
    -o cube_array.ktx2 -f bc7 --cubemap
```

2D array from multiple plain inputs. Passing more than one image without
`--cubemap` or `--volume` assembles a 2D array texture, one layer per input
(argv order). All inputs must share dimensions, format, and mip count:

```sh
ctt layer0.png layer1.png layer2.png -o array.ktx2 -f bc7
```

3D (volume) texture. Each input is stacked as a Z slice in argv order.
3D textures are written as-is (passthrough only): compression, swizzle, and
mipmap generation are not supported for them, so omit `--format`. Both KTX2
and DDS output are supported:

```sh
ctt slice0.png slice1.png slice2.png slice3.png -o volume.ktx2 --volume
```

Generate mipmaps:

```sh
ctt diffuse.png -o diffuse.ktx2 -f bc7 --mipmap
```

With supercompression (KTX2 only). Use `--zstd` or `--zlib`; each optionally
takes a compression level (`--zstd` accepts negative through 22, default 0;
`--zlib` accepts 1 through 10, default 6):

```sh
ctt diffuse.png -o diffuse.ktx2 -f bc7 --zstd
ctt diffuse.png -o diffuse.ktx2 -f bc7 --zlib=9
```

Swizzle channels:

```sh
ctt input.png -o output.ktx2 -f bc7 --swizzle bgra
```

Converting a texture with straight alpha to a format without alpha preserves
the RGB values and emits a warning. Use `--output-alpha premultiplied` to bake
alpha into RGB, `--output-alpha opaque` to explicitly discard it, or
`--allow-discarding-alpha` to acknowledge the default discard and silence the
warning without changing pixels.

### Per-encoder tuning

Each backend exposes its own low-level knobs through a
`--<encoder>-opts "key=val;key=val"` flag: `--bc7e-opts`, `--intel-opts`,
`--bc7f-opts`, `--etcpak-opts`, `--amd-opts`, and `--astcenc-opts`. The opts apply only when
the selected format resolves to that encoder; opts for a different encoder are
ignored with a warning. Run `ctt --help-encoder <name>` (e.g. `bc7e`, `intel`,
`bc7f`, `etcpak`, `amd`, `astcenc`) to list the available keys, their types, and
descriptions for the encoders compiled into your build:

```sh
ctt --help-encoder bc7e
ctt diffuse.png -o diffuse.ktx2 -f bc7e_bc7 --bc7e-opts "uber-level=4;perceptual=true"
```

Run `ctt --help` for a full list of options.

## Minimum Supported Rust Version (MSRV)

The MSRV is **1.90** (edition 2024). MSRV bumps are considered breaking changes.

## Prebuilt binaries

By default, ctt ships prebuilt ISPC static libraries for all supported platforms. A default build requires only a Rust toolchain and a C++ compiler.

Every prebuilt binary has a [GitHub Artifact Attestation](https://docs.github.com/en/actions/security-for-github-actions/using-artifact-attestations/using-artifact-attestations-to-establish-provenance-for-builds) that cryptographically proves it was produced by this repository's CI. See [`docs/prebuilt-binaries.md`](docs/prebuilt-binaries.md) for full details on the build process, attestation guarantees, and how to verify them.

To build from source instead (requires [`ispc.exe`](https://github.com/ispc/ispc/releases) on `PATH`):

```sh
cargo install ctt-cli --no-default-features --features ispc-build-from-source
```

## License

Licensed under any of:

- MIT License
- Apache License, Version 2.0
- Zlib License

at your option.
