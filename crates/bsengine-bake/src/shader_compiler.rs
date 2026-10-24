//! Which compiler D3D12 turns this engine's shaders into GPU code with, and
//! the one `wgpu::InstanceDescriptor` every device in the engine is made from.
//!
//! # DXC, with FXC as the fallback
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
//! Unreal both, offline, shipping the bytecode; Godot at runtime, shipping
//! `dxil.dll` to sign what it makes. wgpu compiles at runtime, so DXC means the
//! two libraries it loads -- `dxcompiler.dll`, the compiler, and `dxil.dll`,
//! the validator that signs its output (D3D12 refuses unsigned DXIL) -- have
//! to be on the machine. A Windows build ships them beside its executable
//! (`--package`, through [`locate`]).
//!
//! Without them a device falls back to FXC: wgpu does so itself when it cannot
//! load either library, and [`dx12_compiler`] asks for FXC outright when
//! [`locate`] found none, so the fallback is a choice made here rather than a
//! load failure. A single-file build is the one that always takes it (one
//! file cannot carry two libraries beside itself), which is why every shader
//! must still compile under FXC too.
//!
//! # Why this lives in the bake crate
//!
//! It is the lowest crate that opens a wgpu device -- the GPU baker does --
//! and the renderer depends on it already, for the tracer's shared WGSL.

use std::path::{Path, PathBuf};

/// The two libraries DXC is, as wgpu loads them.
pub const DXC_LIBRARIES: [&str; 2] = ["dxcompiler.dll", "dxil.dll"];

/// A directory to take the libraries from ahead of every other place.
pub const DXC_DIR_ENV: &str = "BSENGINE_DXC_DIR";

/// The directory both [`DXC_LIBRARIES`] are in, the first of:
///
/// 1. [`DXC_DIR_ENV`], for a machine that keeps them somewhere of its own;
/// 2. beside this executable, where a packaged build has them;
/// 3. the newest Windows SDK's `bin\<version>\x64`, where a machine that
///    builds this engine already has them (the SDK comes with the MSVC
///    toolchain Rust uses on Windows).
///
/// `None` anywhere but Windows, and on a Windows machine with neither.
pub fn locate() -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    let mut candidates = Vec::new();
    if let Some(dir) = std::env::var_os(DXC_DIR_ENV) {
        candidates.push(PathBuf::from(dir));
    }
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
    {
        candidates.push(dir);
    }
    if let Some(root) = std::env::var_os("ProgramFiles(x86)") {
        candidates.extend(sdk_bin_dirs(&Path::new(&root).join(r"Windows Kits\10\bin")));
    }
    first_with_both(&candidates)
}

/// The first of `candidates` holding both libraries. One alone is no use:
/// wgpu needs the validator to sign what the compiler makes.
pub fn first_with_both(candidates: &[PathBuf]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|dir| DXC_LIBRARIES.iter().all(|lib| dir.join(lib).is_file()))
        .cloned()
}

/// An SDK `bin` root's per-version x64 directories, newest version first.
///
/// Versions compared as numbers (`10.0.26100.0` after `10.0.9600.0`), not as
/// text, which would put 9600 ahead.
pub fn sdk_bin_dirs(bin_root: &Path) -> Vec<PathBuf> {
    let arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    };
    let mut versions: Vec<(Vec<u32>, PathBuf)> = std::fs::read_dir(bin_root)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().into_string().ok()?;
            let version = name
                .split('.')
                .map(|part| part.parse().ok())
                .collect::<Option<Vec<u32>>>()?;
            Some((version, entry.path().join(arch)))
        })
        .collect();
    versions.sort_by(|a, b| b.0.cmp(&a.0));
    versions.into_iter().map(|(_, dir)| dir).collect()
}

/// The compiler D3D12 devices use: DXC from [`locate`]'s directory, or FXC
/// when there is none.
///
/// `WGPU_DX12_COMPILER` (`dxc` or `fxc`, wgpu's own variable) overrides it,
/// to compare the two or to rule DXC out of a fault. Asked for DXC that way
/// with nothing located, wgpu searches the usual DLL paths itself.
pub fn dx12_compiler() -> wgpu::Dx12Compiler {
    let forced = wgpu::util::dx12_shader_compiler_from_env();
    if matches!(forced, Some(wgpu::Dx12Compiler::Fxc)) {
        return wgpu::Dx12Compiler::Fxc;
    }
    match locate() {
        Some(dir) => wgpu::Dx12Compiler::Dxc {
            dxil_path: Some(dir.clone()),
            dxc_path: Some(dir),
        },
        None => forced.unwrap_or(wgpu::Dx12Compiler::Fxc),
    }
}

/// The descriptor every instance in the engine is made from: every backend,
/// and D3D12 through [`dx12_compiler`].
///
/// `WGPU_BACKEND` (`dx12`, `vulkan`, ... -- wgpu's own variable) narrows the
/// backends, so a fault seen on one can be reproduced on it alone; on Windows
/// a machine otherwise gets whichever adapter comes first, usually Vulkan's.
pub fn instance_descriptor() -> wgpu::InstanceDescriptor {
    wgpu::InstanceDescriptor {
        backends: wgpu::util::backend_bits_from_env().unwrap_or(wgpu::Backends::all()),
        dx12_shader_compiler: dx12_compiler(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir_with(libraries: &[&str]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("a temp dir");
        for lib in libraries {
            std::fs::write(dir.path().join(lib), b"MZ").expect("write a stand-in library");
        }
        dir
    }

    /// A directory with only one of the two is passed over for the next:
    /// the compiler without its validator makes code D3D12 will not run.
    #[test]
    fn the_first_directory_with_both_libraries_wins() {
        let (none, one, both, also) = (
            dir_with(&[]),
            dir_with(&["dxcompiler.dll"]),
            dir_with(&DXC_LIBRARIES),
            dir_with(&DXC_LIBRARIES),
        );
        let paths = |dirs: &[&tempfile::TempDir]| -> Vec<PathBuf> {
            dirs.iter().map(|d| d.path().to_path_buf()).collect()
        };
        assert_eq!(
            first_with_both(&paths(&[&none, &one, &both, &also])).as_deref(),
            Some(both.path())
        );
        assert_eq!(first_with_both(&paths(&[&none, &one])), None);
        // The other one alone is passed over too.
        let validator_only = dir_with(&["dxil.dll"]);
        assert_eq!(first_with_both(&paths(&[&validator_only])), None);
    }

    /// Newest SDK first, by version number: `10.0.26100.0` is newer than
    /// `10.0.9600.0`, though it sorts before it as text. Entries that are not
    /// versions (the SDK keeps `x64` and `arm64` tools beside them) are left out.
    #[test]
    fn sdk_versions_are_ordered_by_number_newest_first() {
        let root = tempfile::tempdir().expect("a temp dir");
        for name in ["10.0.9600.0", "10.0.26100.0", "10.0.22621.0", "x64"] {
            std::fs::create_dir(root.path().join(name)).expect("an SDK version dir");
        }
        let dirs = sdk_bin_dirs(root.path());
        let versions: Vec<String> = dirs
            .iter()
            .map(|d| {
                d.parent()
                    .and_then(|p| p.file_name())
                    .and_then(|n| n.to_str())
                    .expect("a version dir")
                    .to_string()
            })
            .collect();
        assert_eq!(versions, ["10.0.26100.0", "10.0.22621.0", "10.0.9600.0"]);
        assert!(dirs
            .iter()
            .all(|d| d.ends_with(if cfg!(target_arch = "aarch64") {
                "arm64"
            } else {
                "x64"
            })));
    }
}
