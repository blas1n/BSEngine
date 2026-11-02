# block_compression

[![Crate](https://img.shields.io/crates/v/block_compression.svg)](https://crates.io/crates/block_compression)
[![API](https://docs.rs/block_compression/badge.svg)](https://docs.rs/block_compression)

Texture block compression using WGPU compute shader.
The shaders are a port of Intel's ISPC Texture Compressor's kernel to WGSL compute shader.

Tested with the following backends:

* DX12
* Metal
* Vulkan

## Supported block compressions

Currently supported block compressions are:

* BC1
* BC2
* BC3
* BC4
* BC5
* BC6H
* BC7

## DX12 pipeline creation

The pipeline creation for BC7 and especially BC6H takes a long time under DX12. The DXC compiler seems to take a very
long time to compile the shader. For this reason we moved them behind features, which are included in the default
features.

## License

This project is licensed under the [MIT](LICENSE) license.
