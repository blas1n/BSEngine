#![recursion_limit = "2048"]
// bsengine-scripting
pub mod ops;
pub mod plugin;
pub mod runtime;
pub mod save;
pub use plugin::{
    load_scripts, ProjectDir, Script, ScriptRuntimeResource, ScriptingPlugin, SoundHandles,
};
pub use runtime::ScriptRuntime;
