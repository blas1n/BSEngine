#![recursion_limit = "2048"]
// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]
// bsengine-scripting
pub mod ops;
pub mod plugin;
pub mod runtime;
pub mod save;
pub use plugin::{
    load_scripts, ProjectDir, Script, ScriptRuntimeResource, ScriptingPlugin, SoundHandles,
};
pub use runtime::ScriptRuntime;
