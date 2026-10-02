//! The editor binary. Native only: it hosts the MCP server and the editor UI,
//! neither of which a browser build has (the browser runs the game through
//! `bsengine-runtime`, not this), so on wasm32 it is an empty program and its
//! editor-only dependencies are not built at all.

#[cfg(not(target_arch = "wasm32"))]
mod editor_main;

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    editor_main::main();
}

#[cfg(target_arch = "wasm32")]
fn main() {}
