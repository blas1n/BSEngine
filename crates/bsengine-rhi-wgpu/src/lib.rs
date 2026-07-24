//! `wgpu`-based implementation of the `bsengine-rhi` abstract GPU
//! interface.
//!
//! `WgpuRHIPlugin`/`WgpuRHI` implement the RHI traits; `GpuMeshRegistry`/
//! `GpuTextureRegistry` and `WgpuSurfaceResource` manage GPU-side mesh,
//! texture, and swapchain state. Also hosts the editor's viewport gizmo
//! and Inspector panel rendering (`gizmo`, `panels` modules), since both
//! need direct GPU surface access.
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
/// Editor-only egui panels (asset browser, dock, hierarchy, inspector, viewport).
pub mod panels;
/// The Bevy plugin that wires the wgpu RHI into the app.
pub mod plugin;
/// Post-processing render passes (bloom, tonemapping, etc).
pub mod post_process;
/// The `WgpuRHI` implementation of the abstract RHI traits.
pub mod rhi;
/// Swapchain/frame lifecycle and the main scene render pass.
pub mod surface;
/// GPU texture loading and the texture registry.
pub mod texture;
pub mod theme;
pub use mesh::{
    capsule_vertices, cube_vertices, plane_vertices, sphere_vertices, triangle_vertices,
    GpuMeshRegistry, Vertex,
};
pub use plugin::{RhiResource, WgpuRHIPlugin};
pub use rhi::WgpuRHI;
pub use surface::{
    LightData, MaterialParams, PointLightEntry, SpotLightEntry, WgpuSurfaceResource,
};
pub use texture::GpuTextureRegistry;
