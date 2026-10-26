//! Which compiler D3D12 turns this engine's shaders into GPU code with, and
//! the one `wgpu::InstanceDescriptor` every device in the engine is made from.
//!
//! # DXC, linked into the executable
//!
//! wgpu translates WGSL to HLSL on D3D12 and hands it to one of Microsoft's
//! two compilers. FXC, wgpu's default, is the old one (shader model 5), and it
//! is slow at exactly what this engine's heaviest shaders do: the GI tracer's
//! BVH walk keeps an indexed stack, and FXC took 9.1 s over it on a desktop
//! GPU against DXC's 1.4 s -- 9.1 s in which a player's realtime GI volume
//! does not update. On the software adapter Windows CI runs every test on,
//! every renderer's pipelines took 3.8 s under FXC against 1.9 s under DXC,
//! and the GI tests as a whole 801 s against 496 s.
//!
//! DXC is what the reference engines compile D3D12 shaders with: Unity and
//! Unreal both, offline, shipping the bytecode; Godot at runtime. wgpu
//! compiles at runtime, so DXC has to be in the running program. Until wgpu
//! 30 that meant `dxcompiler.dll` and `dxil.dll` beside the executable --
//! found in a Windows SDK, shipped by `--package` -- and a single-file build,
//! one file that cannot carry two more beside it, ran FXC. wgpu 30's
//! `static-dxc` feature (on in the workspace's `wgpu`) links DXC into the
//! executable instead, so every build of every mode compiles with it and none
//! ships or looks for a library. It costs the executable about 22 MB, where
//! the two libraries were 16 MB beside it; the shaders it makes run exactly as
//! fast as the SDK's DXC's (WARP, 1280x720, eight samples: 57.1 ms both).
//!
//! The DXC linked in is `mach-dxcompiler`, DXC built with Zig as one static
//! library, which `mach-dxcompiler-rs`' build script downloads from its GitHub
//! release -- checksum pinned, and refused unless the release is immutable.
//! A clean build therefore needs the network, as the `v8` crate's already
//! does.
//!
//! # Where FXC is left
//!
//! wgpu-hal links DXC only on Windows for x86_64 with MSVC ([`STATIC_DXC`]);
//! on Windows for ARM64, or with the GNU toolchain, devices compile with FXC.
//! `WGPU_DX12_COMPILER=fxc` asks for FXC anywhere, to rule DXC out of a fault.
//! FXC rejects things DXC takes (an array-returning function, X3017, was one),
//! so every shader must still compile under it: `fxc_fallback` and
//! `realtime_gi::the_update_compiles_for_d3d12` build them that way.
//!
//! # Why this lives in the bake crate
//!
//! It is the lowest crate that opens a wgpu device -- the GPU baker does --
//! and the renderer depends on it already, for the tracer's shared WGSL.

/// Whether this build has DXC linked in: wgpu-hal's own condition for its
/// `static_dxc` cfg, less the feature, which the workspace always enables.
///
/// ⚠️ Asking wgpu for `Dx12Compiler::StaticDxc` where this is false does not
/// fall back -- wgpu-hal panics ("the static-dxc feature was not enabled") --
/// so [`dx12_compiler`] must only ask where it is true. Keep it in step with
/// wgpu-hal's `build.rs` on a wgpu upgrade.
pub const STATIC_DXC: bool = cfg!(all(
    target_os = "windows",
    not(target_arch = "aarch64"),
    target_env = "msvc"
));

/// The compiler D3D12 devices use: the DXC linked in where there is one
/// ([`STATIC_DXC`]), FXC otherwise.
///
/// `WGPU_DX12_COMPILER` (wgpu's own variable: `fxc`, or `dxc` for a
/// `dxcompiler.dll` on the DLL search path) overrides it, to compare compilers
/// or rule DXC out of a fault.
pub fn dx12_compiler() -> wgpu::Dx12Compiler {
    match wgpu::Dx12Compiler::from_env() {
        // The one choice that would panic rather than fall back.
        Some(wgpu::Dx12Compiler::StaticDxc) if !STATIC_DXC => wgpu::Dx12Compiler::Fxc,
        Some(forced) => forced,
        None if STATIC_DXC => wgpu::Dx12Compiler::StaticDxc,
        None => wgpu::Dx12Compiler::Fxc,
    }
}

/// The descriptor every instance in the engine is made from: every backend,
/// and D3D12 through [`dx12_compiler`].
///
/// `WGPU_BACKEND` (`dx12`, `vulkan`, ... -- wgpu's own variable) narrows the
/// backends, so a fault seen on one can be reproduced on it alone; on Windows
/// a machine otherwise gets whichever adapter comes first, usually Vulkan's.
pub fn instance_descriptor() -> wgpu::InstanceDescriptor {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::from_env().unwrap_or(wgpu::Backends::all());
    descriptor.backend_options.dx12.shader_compiler = dx12_compiler();
    descriptor
}
