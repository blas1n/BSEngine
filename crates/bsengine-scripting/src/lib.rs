//! JavaScript scripting runtime for BSEngine, via Deno Core (V8).
//!
//! `ScriptingPlugin` loads `.js` files (`Script`) and exposes ECS
//! operations to scripts as async Deno ops (`ops` module);
//! `ScriptRuntime` wraps the underlying V8 isolate, and `save` handles
//! script-driven save-game serialization.
#![recursion_limit = "2048"]
// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]
// bsengine-scripting
pub mod ops;
pub mod plugin;
pub mod runtime;
pub mod save;
pub use plugin::{
    load_scripts, ProjectDir, Script, ScriptRuntimeResource, ScriptingPlugin, SoundHandles,
    KEY_MAPPINGS,
};
pub use runtime::ScriptRuntime;
