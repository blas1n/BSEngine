pub mod mesh;
pub mod plugin;
pub mod rhi;
pub mod surface;
pub mod texture;
pub use mesh::{cube_vertices, triangle_vertices, GpuMeshRegistry, Vertex};
pub use plugin::{RhiResource, WgpuRHIPlugin};
pub use rhi::WgpuRHI;
pub use surface::{
    LightData, MaterialParams, PointLightEntry, SpotLightEntry, WgpuSurfaceResource,
};
pub use texture::GpuTextureRegistry;
