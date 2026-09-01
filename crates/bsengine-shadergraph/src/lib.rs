//! Node-graph shader authoring: a graph in, a WGSL string out.
//!
//! A shader is authored as a `.shadergraph.ron` file -- the same RON
//! convention the engine already uses for scenes and prefabs -- and compiled
//! to a `.wgsl` file that `CustomShader.path` points at exactly as it would
//! at a hand-written shader. Nothing downstream changes: the existing shader
//! loading, caching and hot-reload paths work unmodified, which keeps the
//! generated route an *addition* to the WGSL text path rather than a
//! replacement for it.
//!
//! The crate deliberately depends on no ECS, GPU or UI types. That keeps it a
//! leaf both the editor UI and asset tooling can use, and it makes
//! compilation a pure function that unit tests can drive without a GPU
//! adapter.

#![deny(missing_docs)]

pub mod compile;
pub mod graph;

pub use compile::compile;
pub use graph::{Edge, GraphError, GraphNode, NodeKind, ShaderGraph};
