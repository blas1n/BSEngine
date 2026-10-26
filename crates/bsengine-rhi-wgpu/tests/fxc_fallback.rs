//! Every renderer pipeline still compiles under FXC.
//!
//! The engine's devices compile D3D12 shaders with the DXC linked into it
//! (`bsengine_bake::shader_compiler`), and everything else in this crate is
//! tested that way on Windows CI. FXC is still what a build without DXC runs
//! -- Windows on ARM64, or not MSVC -- and what `WGPU_DX12_COMPILER=fxc` asks
//! for, and FXC rejects things DXC takes (an array-returning function, X3017,
//! was one). So one renderer is built here
//! under FXC, on D3D12, with every pipeline it makes; a pipeline FXC rejects
//! is a validation error, and wgpu panics on one nobody catches.
//!
//! Its own test binary, because it chooses the compiler and the backend
//! through the process's environment, which every test in a binary shares.
//! The realtime GI update's pipelines are built under FXC by
//! `realtime_gi::the_update_compiles_for_d3d12`.
#![cfg(windows)]

use bsengine_rhi_wgpu::surface::WgpuSurface;

#[test]
fn every_renderer_pipeline_compiles_under_fxc() {
    // Rust 2021: not `unsafe` yet, and nothing else in this binary runs.
    std::env::set_var("WGPU_DX12_COMPILER", "fxc");
    std::env::set_var("WGPU_BACKEND", "dx12");
    let descriptor = bsengine_bake::shader_compiler::instance_descriptor();
    assert!(
        matches!(
            descriptor.backend_options.dx12.shader_compiler,
            wgpu::Dx12Compiler::Fxc
        ) && descriptor.backends == wgpu::Backends::DX12,
        "premise: the engine's descriptor asks for FXC on D3D12 alone"
    );
    // Through the path a game takes: its own instance, from that descriptor.
    let surface = pollster::block_on(WgpuSurface::new_offscreen(64, 64, false))
        .expect("a renderer on D3D12 under FXC");
    assert_eq!(surface.width(), 64, "the renderer was built");
}
