//! Scene rendering pipeline for BSEngine, built on `bsengine-rhi`.
//!
//! `RenderPlugin` drives the per-frame render systems (transform
//! propagation, draw-call collection, lighting) against the abstract GPU
//! interface; `MeshRenderer` is the ECS-facing component marking an
//! entity as drawable.
// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]

/// ECS components describing renderable entities (currently `MeshRenderer`).
pub mod components;
/// The Bevy `Plugin` wiring transform propagation and frame rendering into the app schedule.
pub mod plugin;
/// Custom WGSL shader source asset and its loader.
pub mod shader_asset;

pub use components::MeshRenderer;
pub use plugin::RenderPlugin;
pub use shader_asset::{load_shader_source, ShaderSource, ShaderSourceLoader};
