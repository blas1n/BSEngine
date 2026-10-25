//! D3D12 compiles this engine's shaders with DXC where its libraries are
//! found (`bsengine_bake::shader_compiler`).
//!
//! What a device shows of its compiler is the shader model it offers: wgpu
//! caps a D3D12 adapter at shader model 5.1 under FXC, and offers 6.x -- and
//! with it `SUBGROUP` and `SHADER_INT64`, where the hardware has them --
//! only when DXC loaded. Asking for DXC is not enough to get it: wgpu falls
//! back to FXC, with a log line and nothing else, when it cannot load either
//! library -- which is exactly the failure this file exists to catch.
#![cfg(windows)]

use bsengine_bake::shader_compiler::{instance_descriptor, locate};

/// The features only shader model 6 brings, of the adapter `descriptor` (on
/// D3D12 alone) gives.
fn shader_model_6_features(mut descriptor: wgpu::InstanceDescriptor) -> wgpu::Features {
    descriptor.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("Windows has a D3D12 adapter (WARP at worst); a skip would look like a pass");
    adapter.features() & (wgpu::Features::SUBGROUP | wgpu::Features::SHADER_INT64)
}

/// The engine's devices are made with DXC when its libraries are on the
/// machine. Where they are not, the test says so and passes -- a developer's
/// machine without a Windows SDK is an ordinary one -- unless
/// `BSENGINE_REQUIRE_DXC` is set, as Windows CI sets it: there a missing DXC
/// would mean every other test on that runner quietly measured FXC.
#[test]
fn d3d12_compiles_with_dxc_where_its_libraries_are() {
    if locate().is_none() {
        assert!(
            std::env::var_os("BSENGINE_REQUIRE_DXC").is_none(),
            "BSENGINE_REQUIRE_DXC is set, and no dxcompiler.dll and dxil.dll were found \
             (BSENGINE_DXC_DIR, beside the executable, or a Windows SDK)"
        );
        eprintln!("no DXC on this machine: nothing to check");
        return;
    }
    let fxc = shader_model_6_features({
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        d.backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;
        d
    });
    assert!(
        fxc.is_empty(),
        "premise: under FXC the adapter stops at shader model 5.1: {fxc:?}"
    );
    let engine = shader_model_6_features(instance_descriptor());
    assert!(
        !engine.is_empty(),
        "the engine's devices should compile with DXC, and offer shader model 6 \
         (subgroups or 64-bit integers); this one offers neither"
    );
}

/// And the renderer itself is made that way: a game's renderer, through
/// `new_offscreen`'s own instance, on D3D12 (`WGPU_BACKEND`, which the
/// engine's descriptor honours -- otherwise a Windows machine would likely
/// hand it Vulkan, where the question does not arise).
#[test]
fn a_renderer_on_d3d12_is_made_with_dxc() {
    if locate().is_none() {
        assert!(
            std::env::var_os("BSENGINE_REQUIRE_DXC").is_none(),
            "BSENGINE_REQUIRE_DXC is set, and no DXC was found"
        );
        eprintln!("no DXC on this machine: nothing to check");
        return;
    }
    // Rust 2021: not `unsafe` yet. The other test here sets its backends
    // itself, so the two cannot disagree.
    std::env::set_var("WGPU_BACKEND", "dx12");
    let surface = pollster::block_on(bsengine_rhi_wgpu::surface::WgpuSurface::new_offscreen(
        64, 64, false,
    ))
    .expect("a renderer on D3D12");
    assert_eq!(
        surface.adapter_backend(),
        Some(wgpu::Backend::Dx12),
        "premise: the renderer is on D3D12 -- Vulkan offers subgroups too"
    );
    let sm6 =
        surface.adapter_features() & (wgpu::Features::SUBGROUP | wgpu::Features::SHADER_INT64);
    assert!(
        !sm6.is_empty(),
        "the renderer's device should compile with DXC and offer shader model 6; \
         its adapter offers {:?}",
        surface.adapter_features()
    );
}
