//! Downloads and statically links the `mach-dxcompiler` C library.
#![cfg_attr(feature = "internal_update_targets", allow(dead_code, unused_imports))]

/// Generated release data: the current release tag and its per-target checksums.
mod targets;
/// The `internal_update_targets` feature's tool for regenerating `targets.rs`.
#[cfg(feature = "internal_update_targets")]
mod update_targets;

use std::process::Command;
use std::{
    env, fs,
    io::ErrorKind::NotFound,
    path::{Path, PathBuf},
};
use targets::{AVAILABLE_TARGETS, RELEASE_TAG};

/// Owner of the GitHub repository hosting the prebuilt `mach-dxcompiler` releases.
const RELEASE_REPO_OWNER: &str = "DouglasDwyer";
/// Name of the GitHub repository hosting the prebuilt `mach-dxcompiler` releases.
const RELEASE_REPO_NAME: &str = "mach-dxcompiler";

/// A prebuilt archive this crate can download for one target triple and CRT linkage.
struct Target {
    /// Target triple, e.g. `"x86_64-linux-gnu"`.
    pub name: &'static str,
    /// Whether this archive links the CRT statically. Only targets that publish more
    /// than one archive (currently just MSVC) select between builds using this;
    /// every other target's sole entry is used regardless of its value.
    pub static_crt: bool,
    /// SHA-256 of the archive.
    pub sha256: &'static str,
}

/// A prebuilt release archive to download and link.
struct ReleaseArtifact {
    /// URL of the `.tar.gz` archive.
    pub url: String,
    /// Expected SHA-256 of the archive.
    pub sha256: &'static str,
}

/// Downloads and links the static DXC binary.
#[cfg(not(feature = "internal_update_targets"))]
fn main() {
    println!("cargo:rerun-if-changed=msvc_version.c");
    println!("cargo:rerun-if-changed=mach_dxc.h");
    println!("cargo:rerun-if-changed=targets.rs");
    #[cfg(all(feature = "msvc_version_validation", target_env = "msvc"))]
    validate_msvc_version();
    verify_release_is_immutable();
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("Failed to get OUT_DIR environment"));
    let artifact = get_target_artifact(static_crt());
    let file_path = out_dir.join("machdxcompiler.tar.gz");
    download_released_library(&artifact.url, &file_path);
    verify_checksum(&file_path, artifact.sha256);
    extract_tar_gz(&file_path, &out_dir);
    #[cfg(feature = "cbindings")]
    generate_bindings();
    link_binary(&out_dir);
}

/// Emits the `cargo:rustc-link-*` directives needed to statically link `machdxcompiler`.
fn link_binary(out_dir: &Path) {
    let os = env::var("CARGO_CFG_TARGET_OS").expect("Failed to get os");
    let abi = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();

    if os == "windows" && abi == "gnu" {
        // The Zig compiler suffixes Windows/GNU libraries with `.lib`, but Rust expects `.a`
        println!("cargo:rustc-link-lib=static:+verbatim=machdxcompiler.lib");
    } else {
        println!("cargo:rustc-link-lib=static=machdxcompiler");
    }

    println!("cargo:rustc-link-search=native={}", out_dir.display());

    if os == "windows" && abi == "gnu" {
        // `-bundle` defers resolution to the final link instead of rustc eagerly packing
        // the archive into this crate's own rlib.
        println!("cargo:rustc-link-lib=static:-bundle,+whole-archive=stdc++");
        println!("cargo:rustc-link-lib=static:-bundle,+whole-archive=gcc_eh");
        // COM APIs used by DXC (SysAllocStringLen/SysFreeString, CoTaskMemAlloc/Free/Realloc).
        println!("cargo:rustc-link-lib=dylib=ole32");
        println!("cargo:rustc-link-lib=dylib=oleaut32");
    } else if os == "linux" && abi == "gnu" {
        println!("cargo:rustc-link-lib=dylib=stdc++");
    }
}

/// Regenerates `targets.rs` instead of building, since the two are mutually exclusive:
/// there's no library to link until `targets.rs` reflects a real, immutable release.
#[cfg(feature = "internal_update_targets")]
fn main() {
    update_targets::run();
}

/// Generates C API bindings.
#[cfg(feature = "cbindings")]
fn generate_bindings() {
    let bindings = bindgen::Builder::default()
        .rust_target(bindgen::RustTarget::Stable_1_73)
        .header("mach_dxc.h")
        .generate()
        .expect("Unable to generate bindings");

    let out_path =
        PathBuf::from(env::var("OUT_DIR").expect("Failed to get OUT_DIR environment variable"));
    bindings
        .write_to_file(out_path.join("cbindings.rs"))
        .expect("Couldn't write bindings!");
}

/// This checks if the installed version of the Microsoft Visual C++ compiler (MSVC)
/// is compatible with the features introduced in Visual Studio 2022 version 17.11.
/// It retrieves the compiler version using the `cl.exe` executable and compares it against
/// a predefined minimum version. Panics if the current version is lower than the required minimum version.
#[cfg(all(feature = "msvc_version_validation", target_env = "msvc"))]
fn validate_msvc_version() {
    let target = std::env::var("TARGET").expect("Faild to get TARGET environment");

    // https://learn.microsoft.com/en-us/cpp/build/reference/ep-preprocess-to-stdout-without-hash-line-directives?view=msvc-170
    // Preprocess the _MSC_VER macro to get current version
    const CL_ARGS: &[&str] = &["/nologo", "/EP", "msvc_version.c"];

    // https://learn.microsoft.com/en-us/cpp/overview/compiler-versions?view=msvc-170#version-macros
    // MSVC version of Visual Studio 2022 version 17.11
    const MINIMUM_MSVC_VERSION: usize = 1941;

    let status = cc::windows_registry::find(&target, "cl.exe")
        .expect("Failed to locate cl.exe.\nPlease ensure it is installed and included in your system's PATH environment variable.")
        .args(CL_ARGS)
        .output()
        .expect(&format!("Failed to run cl.exe with args: {CL_ARGS:?}"));
    if status.stdout.is_empty() {
        panic!("Output from cl.exe is empty");
    }
    let output_str = match std::str::from_utf8(&status.stdout) {
        Ok(s) => s,
        Err(e) => panic!(
            "Failed to parse output of cl.exe as utf-8:\nsrc{:?}\nerror:\n{e:?}",
            status.stdout
        ),
    };

    let version_str = output_str.trim();
    let current_version = version_str.parse::<usize>().expect(&format!(
        "Failed to parse version from output of cl.exe: {version_str}"
    ));
    if current_version < MINIMUM_MSVC_VERSION {
        panic!("Please upgrade the Visual Studio, the minimum supported version of MSVC is {MINIMUM_MSVC_VERSION}, which is correspond to [Visual Studio 2022 version 17.11], but the current version of MSVC is {current_version}\nCheck versions on https://learn.microsoft.com/en-us/cpp/overview/compiler-versions?view=msvc-170#version-macros");
    }
}

/// Fetches the JSON body of `GET /repos/{owner}/{repo}/releases/{path}` from the
/// GitHub REST API. Panics if the request fails or the response isn't valid UTF-8.
fn fetch_release_json(path: &str) -> String {
    let api_url = format!(
        "https://api.github.com/repos/{RELEASE_REPO_OWNER}/{RELEASE_REPO_NAME}/releases/{path}"
    );

    let output = Command::new("curl")
        .arg("--location")
        .arg(&api_url)
        .output()
        .expect("Failed to start Curl to query the GitHub releases API");
    if !output.status.success() {
        panic!(
            "Failed to query the GitHub releases API at {api_url}:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    String::from_utf8(output.stdout).expect("GitHub releases API returned a non-UTF-8 response")
}

/// Verifies that [`RELEASE_TAG`] is an [immutable release], so its assets cannot be
/// swapped out after publication. Panics if the release is not immutable or its status
/// cannot be determined via the GitHub REST API.
///
/// [immutable release]: https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases
fn verify_release_is_immutable() {
    let body = fetch_release_json(&format!("tags/{RELEASE_TAG}"));

    if !release_json_is_immutable(&body) {
        panic!(
            "Refusing to download a mutable GitHub release.\n\n\
             `{RELEASE_REPO_OWNER}/{RELEASE_REPO_NAME}` release `{RELEASE_TAG}` is not an immutable \
             release, so its assets could be replaced after publication without any change to this \
             crate's source. Publish an immutable release and bump `RELEASE_TAG` before building.\n\
             https://docs.github.com/en/code-security/concepts/supply-chain-security/immutable-releases"
        );
    }
}

/// Returns `true` if the release API JSON reports `"immutable": true`.
fn release_json_is_immutable(body: &str) -> bool {
    json_field(body, "immutable").is_some_and(|value| value.starts_with("true"))
}

/// Returns the JSON text immediately following a top-level `"key":`, with leading
/// whitespace trimmed, scanning the payload directly to avoid a JSON-parser build
/// dependency.
fn json_field<'a>(json: &'a str, key: &str) -> Option<&'a str> {
    let (_, after_key) = json.split_once(&format!("\"{key}\""))?;
    Some(after_key.trim_start().strip_prefix(':')?.trim_start())
}

/// Verifies that the downloaded archive's SHA-256 matches `expected`, pinning the
/// exact native-binary bytes that get linked. Panics if the archive cannot be read
/// or its digest does not match.
fn verify_checksum(file_path: &Path, expected: &str) {
    let bytes = fs::read(file_path).unwrap_or_else(|e| {
        panic!(
            "Failed to read downloaded archive {} for checksum verification: {e}",
            file_path.display()
        )
    });
    let actual = sha256_hex(&bytes);

    if !actual.eq_ignore_ascii_case(expected) {
        let _ = fs::remove_file(file_path);
        panic!(
            "Checksum mismatch for downloaded archive {}.\n\
             \n\
             expected SHA-256: {expected}\n\
             actual SHA-256:   {actual}\n\
             \n\
             The downloaded archive does not match the checksum pinned in build.rs. The \
             release asset may have been tampered with, or `AVAILABLE_TARGETS` may be stale \
             for release `{RELEASE_TAG}`. Refusing to link an unverified binary.",
            file_path.display()
        );
    }
}

/// Computes the SHA-256 digest of `data`, returned as lowercase hex.
fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;

    let mut hex = String::with_capacity(64);
    for byte in lhash::sha256(data) {
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

/// Gets the download URL and metadata for the appropriate DXC binary.
fn get_target_artifact(static_crt: bool) -> ReleaseArtifact {
    let base_url = format!("https://github.com/{RELEASE_REPO_OWNER}/{RELEASE_REPO_NAME}/releases");
    let arch = env::var("CARGO_CFG_TARGET_ARCH").expect("Failed to get architecture");
    let mut os = env::var("CARGO_CFG_TARGET_OS").expect("Failed to get os");
    if env::var("CARGO_CFG_TARGET_VENDOR").unwrap_or_default() == "apple" && os == "darwin" {
        os = "macos".to_owned();
    }
    let mut abi = env::var("CARGO_CFG_TARGET_ENV").unwrap_or_default();
    if abi.is_empty() {
        abi = "none".to_owned();
    }
    let target_name = format!("{arch}-{os}-{abi}");

    let candidates: Vec<_> = AVAILABLE_TARGETS
        .iter()
        .filter(|t| t.name == target_name)
        .collect();
    let target = candidates
        .iter()
        .copied()
        .max_by_key(|t| t.static_crt == static_crt)
        .unwrap_or_else(|| {
            panic!("Unsupported target: {target_name}\nCheck supported targets on {base_url}")
        });

    let crt = if candidates.len() > 1 && !target.static_crt {
        "Dynamic_lib"
    } else {
        "lib"
    };

    ReleaseArtifact {
        url: format!("{base_url}/download/{RELEASE_TAG}/{target_name}_ReleaseFast_{crt}.tar.gz"),
        sha256: target.sha256,
    }
}

/// Downloads the provided URL to a file.
fn download_released_library(url: &str, file_path: &Path) {
    match Command::new("curl")
        .arg("--location")
        .arg("-o")
        .arg(file_path)
        .arg(url)
        .spawn()
        .expect("Failed to start Curl to download DXC binary")
        .wait()
    {
        Ok(result) => {
            if !result.success() {
                if let Err(e) = fs::remove_file(file_path) {
                    if e.kind() != NotFound {
                        panic!("Failed to remove incomplete file");
                    }
                }
                panic!("{result}");
            }
        }
        Err(_) => {
            if let Err(e) = fs::remove_file(file_path) {
                if e.kind() != NotFound {
                    panic!("Failed to remove incomplete file");
                }
            }
            panic!("Failed to download DXC binary");
        }
    }
}

/// Extracts the file at the provided path as a `.tar.gz` file.
/// The contents are extracted to the current directory.
fn extract_tar_gz(path: &Path, output_dir: &Path) {
    let result = Command::new("tar")
        .current_dir(output_dir)
        .arg("-xzf")
        .arg(path)
        .spawn()
        .expect("Failed to start Tar to extract DXC binary")
        .wait()
        .expect("Failed to extract DXC binary");
    if !result.success() {
        panic!("{result}");
    }
}

/// Determines whether the CRT is being statically or dynamically linked.
fn static_crt() -> bool {
    env::var("CARGO_ENCODED_RUSTFLAGS")
        .map(|flags| flags.contains("target-feature=+crt-static"))
        .unwrap_or(false)
}
