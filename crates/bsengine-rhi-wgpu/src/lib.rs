//! `wgpu`-based GPU surface, pipelines, and buffers for BSEngine.
//!
//! `WgpuRHIPlugin` creates the render target (swapchain or offscreen
//! texture); `GpuMeshRegistry`/`GpuTextureRegistry` and
//! `WgpuSurfaceResource` manage GPU-side mesh, texture, and swapchain
//! state. Also hosts the editor's viewport gizmo and Inspector panel
//! rendering (`gizmo`, `panels` modules), since both need direct GPU
//! surface access.
// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]
#![warn(missing_docs)]

/// Screen-space translate/rotate gizmo math and drawing.
pub mod gizmo;
/// GPU mesh generation and the mesh registry.
pub mod mesh;
/// Minimal glTF parsing + a small dedicated render pipeline for the Asset
/// Browser's mesh thumbnails.
pub mod mesh_thumbnail;
/// Where a rendered frame goes (window swapchain or offscreen texture) and how
/// to read it back.
mod output;
/// Editor-only egui panels (asset browser, dock, hierarchy, inspector, viewport).
pub mod panels;
/// Billboarded particle quads, drawn instanced.
pub mod particles;
/// The Bevy plugin that creates the render target and wires up resize handling.
pub mod plugin;
/// Post-processing render passes (bloom, tonemapping, etc).
pub mod post_process;
/// Frame/GPU statistics: texture memory tracking, draw-call/triangle
/// counting, and feature-gated GPU pass timing.
pub mod profiler;
/// Swapchain/frame lifecycle and the main scene render pass.
pub mod surface;
pub mod taa_jitter;
/// GPU texture loading and the texture registry.
pub mod texture;
pub mod theme;
pub use mesh::{
    capsule_vertices, cube_vertices, plane_vertices, sphere_vertices, triangle_vertices,
    GpuMeshRegistry, Vertex,
};
pub use plugin::{GpuQueueResource, WgpuRHIPlugin};
pub use surface::{
    LightData, MaterialParams, PointLightEntry, SpotLightEntry, WgpuSurfaceResource,
};
pub use texture::GpuTextureRegistry;
