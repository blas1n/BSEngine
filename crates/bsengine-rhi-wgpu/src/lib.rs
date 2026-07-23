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

pub mod gizmo;
pub mod mesh;
pub mod panels;
pub mod plugin;
pub mod post_process;
pub mod rhi;
pub mod surface;
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
