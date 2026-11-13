//! Picks the native script engine: `bsengine_quickjs` when scripts run in
//! QuickJS rather than V8.
//!
//! That is every Android and iOS build -- rusty_v8 ships no library for
//! either, and iOS forbids an app's own JIT -- and a desktop build with the
//! `quickjs` feature, which is how the mobile engine is tested on the machines
//! CI has. A feature alone cannot say "on Android": Cargo has no
//! target-conditional features, and the dependency table can add `rquickjs`
//! per target but cannot switch code. So the code switches on this one cfg,
//! and `#[script_op]` expands against it in this crate.

fn main() {
    println!("cargo::rustc-check-cfg=cfg(bsengine_quickjs)");
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let target_arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let feature = std::env::var_os("CARGO_FEATURE_QUICKJS").is_some();
    // A browser build keeps the page's engine whatever the features say: a
    // workspace-wide `--features quickjs` must not give it a second backend.
    if target_arch != "wasm32" && (feature || target_os == "android" || target_os == "ios") {
        println!("cargo::rustc-cfg=bsengine_quickjs");
    }
}
