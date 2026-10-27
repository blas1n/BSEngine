# mach-dxcompiler-rs

[![Crates.io](https://img.shields.io/crates/v/mach-dxcompiler-rs.svg)](https://crates.io/crates/mach-dxcompiler-rs)
[![Docs.rs](https://docs.rs/mach-dxcompiler-rs/badge.svg)](https://docs.rs/mach-dxcompiler-rs)

This library allows for statically linking _prebuilt binaries_ from [`mach-dxcompiler`](https://github.com/hexops/mach-dxcompiler) into a Rust project. The `mach-dxcompiler` repository is a fork of Microsoft's [`DirectXShaderCompiler`](https://github.com/microsoft/DirectXShaderCompiler/tree/main) that replaces the CMake build system with Zig. This allows for building the project statically and linking it into existing applications - but the core logic comes from the original DXC library.

### Usage

The `curl.exe` and `tar.exe` command line utilities are required for this crate's build script. They should be installed by default in Windows 10 and 11.

First, add this crate to the project: `cargo add mach-dxcompiler-rs`

Next, use the `DxcCreateInstance` function to create DXC COM objects. Once created, these objects are usable with the normal COM windows API.

```rust
use windows::Win32::Graphics::Direct3D::Dxc;

let mut dxcompiler = std::ptr::null_mut();
mach_dxcompiler_rs::DxcCreateInstance(&Dxc::CLSID_DxcCompiler, &Dxc::IDxcCompiler3::IID, &mut dxcompiler);
// ... use the shader compiler ...
```

### Visual Studio Requirement

For MSVC users, ensure that your version of Visual Studio is at least 17.11 or higher. This is necessary for compatibility with the build tools and libraries used in this project.
