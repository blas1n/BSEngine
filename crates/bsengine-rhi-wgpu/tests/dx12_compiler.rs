//! D3D12 compiles this engine's shaders with the DXC linked into it
//! (`bsengine_bake::shader_compiler`).
//!
//! What a device shows of its compiler is the shader model it offers: wgpu
//! caps a D3D12 adapter at shader model 5.1 under FXC, and offers 6.x -- and
//! with it `SUBGROUP` and `SHADER_INT64`, where the hardware has them --
//! only under DXC.
//!
//! No skip where DXC is missing, as there was while DXC was two libraries a
//! machine might not have: wherever [`STATIC_DXC`] holds it is in the
//! executable, so a device without it is the failure. That is also what a
//! packaged build -- the same executable, single-file included -- runs.
#![cfg(windows)]

use bsengine_bake::shader_compiler::{instance_descriptor, STATIC_DXC};

/// The features only shader model 6 brings, of the adapter `descriptor` (on
/// D3D12 alone) gives.
fn shader_model_6_features(mut descriptor: wgpu::InstanceDescriptor) -> wgpu::Features {
    descriptor.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default()))
        .expect("Windows has a D3D12 adapter (WARP at worst); a skip would look like a pass");
    adapter.features() & (wgpu::Features::SUBGROUP | wgpu::Features::SHADER_INT64)
}

/// The engine's devices are made with DXC, and the premise that the features
/// read are DXC's: the same adapter under FXC has none of them.
#[test]
fn d3d12_compiles_with_the_dxc_linked_in() {
    if !STATIC_DXC {
        eprintln!("this build has no DXC linked in (ARM64 or not MSVC): devices use FXC");
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
    let descriptor = instance_descriptor();
    assert!(
        matches!(
            descriptor.backend_options.dx12.shader_compiler,
            wgpu::Dx12Compiler::StaticDxc
        ),
        "the engine's descriptor should ask for the DXC linked in"
    );
    let engine = shader_model_6_features(descriptor);
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
    if !STATIC_DXC {
        eprintln!("this build has no DXC linked in (ARM64 or not MSVC): devices use FXC");
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
