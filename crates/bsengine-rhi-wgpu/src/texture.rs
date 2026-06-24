use bsengine_ecs::Resource;
use std::collections::HashMap;
use std::sync::Arc;

struct GpuTexture {
    _texture: wgpu::Texture,
    _view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
}

#[derive(Resource)]
pub struct GpuTextureRegistry {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<u64, GpuTexture>,
    next_id: u64,
}

impl GpuTextureRegistry {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let bgl = Self::create_bgl(&device);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("tex reg sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            device,
            queue,
            bgl,
            sampler,
            textures: HashMap::new(),
            next_id: 1,
        }
    }

    fn create_bgl(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("tex reg bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        })
    }

    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<u64, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("image decode: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("user texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            texture.as_image_copy(),
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: None,
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("user tex bg"),
            layout: &self.bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(
            id,
            GpuTexture {
                _texture: texture,
                _view: view,
                bind_group,
            },
        );
        Ok(id)
    }

    pub fn get_bind_group(&self, id: u64) -> Option<&wgpu::BindGroup> {
        self.textures.get(&id).map(|t| &t.bind_group)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rhi::WgpuRHI;
    use std::sync::Arc;

    fn make_registry() -> GpuTextureRegistry {
        let rhi = pollster::block_on(WgpuRHI::new_headless()).expect("headless rhi");
        GpuTextureRegistry::new(Arc::new(rhi.device), Arc::new(rhi.queue))
    }

    #[test]
    fn get_unknown_id_returns_none() {
        let reg = make_registry();
        assert!(reg.get_bind_group(999).is_none());
    }

    #[test]
    fn load_invalid_bytes_returns_err() {
        let mut reg = make_registry();
        assert!(reg.load_from_bytes(b"not an image").is_err());
    }
}
