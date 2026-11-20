//! Sets `astc_encoder` where the ASTC encoder (`ctt-astcenc`) is a
//! dependency: an x86_64 or aarch64 target that is not Android or iOS -- the
//! same condition as the target table in this crate's Cargo.toml, which can
//! add the dependency per target but cannot switch code. `src/astc.rs`
//! switches on this one cfg, so the two never disagree about where an
//! encoder exists.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(astc_encoder)");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    if matches!(target_arch.as_str(), "x86_64" | "aarch64")
        && !matches!(target_os.as_str(), "android" | "ios")
    {
        println!("cargo::rustc-cfg=astc_encoder");
    }
}
