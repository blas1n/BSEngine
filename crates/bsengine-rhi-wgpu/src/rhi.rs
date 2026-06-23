use bsengine_rhi::{RHIMesh, RHIShader, RHITexture, RHI};
use wgpu::{Device, Queue};

pub struct WgpuRHI {
    pub(crate) device: Device,
    pub(crate) queue: Queue,
}

impl WgpuRHI {
    pub async fn new_headless() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::None,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .ok_or("No adapter found")?;

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("BSEngine headless device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    memory_hints: wgpu::MemoryHints::default(),
                },
                None,
            )
            .await
            .map_err(|e| format!("Device request failed: {e}"))?;

        Ok(Self { device, queue })
    }
}

struct WgpuMesh;
impl RHIMesh for WgpuMesh {}

struct WgpuShader;
impl RHIShader for WgpuShader {}

struct WgpuTexture;
impl RHITexture for WgpuTexture {}

impl RHI for WgpuRHI {
    fn create_mesh(&self) -> Box<dyn RHIMesh> {
        Box::new(WgpuMesh)
    }
    fn create_shader(&self, _src: &str) -> Box<dyn RHIShader> {
        Box::new(WgpuShader)
    }
    fn create_texture(&self) -> Box<dyn RHITexture> {
        Box::new(WgpuTexture)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_rhi::RHI;

    fn headless_rhi() -> WgpuRHI {
        pollster::block_on(WgpuRHI::new_headless()).expect("Failed to create headless WgpuRHI")
    }

    #[test]
    fn can_create_headless() {
        let _rhi = headless_rhi();
    }

    #[test]
    fn create_mesh_returns_object() {
        let rhi = headless_rhi();
        let _mesh = rhi.create_mesh();
    }

    #[test]
    fn create_shader_returns_object() {
        let rhi = headless_rhi();
        let _shader = rhi.create_shader("// stub");
    }

    #[test]
    fn create_texture_returns_object() {
        let rhi = headless_rhi();
        let _texture = rhi.create_texture();
    }
}
