// Bevy ECS system params (Query<(A, B, C, ...)>, ParamSet<(...)>) routinely
// exceed clippy's type-complexity threshold; that's the idiom, not a real
// complexity problem. Bevy itself disables this lint crate-wide for the
// same reason.
#![allow(clippy::type_complexity)]

pub mod gizmo;
pub mod mesh;
pub mod plugin;
pub mod post_process;
pub mod rhi;
pub mod surface;
pub mod texture;
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
