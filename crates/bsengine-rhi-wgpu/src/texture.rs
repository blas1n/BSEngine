use bsengine_ecs::Resource;
use std::collections::HashMap;
use std::sync::Arc;

struct GpuTexture {
    _texture: crate::profiler::TrackedTexture,
    _view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

/// Owns every GPU texture uploaded for the running app, keyed by a registry-assigned id.
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
    /// Creates an empty registry bound to the given wgpu device/queue.
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

    /// Decodes an in-memory image file (PNG, JPEG, etc), uploads it as an
    /// RGBA8 texture, and returns its assigned id.
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<u64, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("image decode: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(self.load_from_rgba(width, height, &rgba))
    }

    /// Uploads already-decoded RGBA8 pixel data as a new texture and returns its assigned id.
    pub fn load_from_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> u64 {
        let tex = self.build(width, height, rgba);
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, tex);
        id
    }

    /// Rebuilds an already-loaded texture from new pixels, keeping its id.
    /// Returns whether `id` was loaded.
    ///
    /// Hot reload uses this rather than `load_from_rgba` because `Material`
    /// stores the id: replacing under the same id updates every material using
    /// the texture at once. The new image may have different dimensions.
    ///
    /// The returned flag is `#[must_use]` for the same reason as
    /// `GpuMeshRegistry::replace`: `false` means an id a caller recorded at load
    /// time is no longer loaded, and dropping it turns that into a reload that
    /// appears to work and silently keeps the old pixels.
    #[must_use]
    pub fn replace(&mut self, id: u64, width: u32, height: u32, rgba: &[u8]) -> bool {
        if !self.textures.contains_key(&id) {
            return false;
        }
        let tex = self.build(width, height, rgba);
        self.textures.insert(id, tex);
        true
    }

    fn build(&self, width: u32, height: u32, rgba: &[u8]) -> GpuTexture {
        let texture = crate::profiler::create_tracked_texture(
            &self.device,
            &wgpu::TextureDescriptor {
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
            },
        );
        self.queue.write_texture(
            texture.as_image_copy(),
            rgba,
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
        GpuTexture {
            _texture: texture,
            _view: view,
            bind_group,
            width,
            height,
        }
    }

    /// Looks up a previously loaded texture's bind group by id.
    pub fn get_bind_group(&self, id: u64) -> Option<&wgpu::BindGroup> {
        self.textures.get(&id).map(|t| &t.bind_group)
    }

    /// Looks up a previously loaded texture's pixel dimensions by id.
    pub fn get_size(&self, id: u64) -> Option<(u32, u32)> {
        self.textures.get(&id).map(|t| (t.width, t.height))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::WgpuSurface;

    fn make_registry() -> GpuTextureRegistry {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        GpuTextureRegistry::new(device, queue)
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

    #[test]
    fn replace_swaps_texture_contents_under_the_same_id() {
        let mut reg = make_registry();

        let id = reg.load_from_rgba(1, 1, &[255, 0, 0, 255]);
        assert!(reg.get_bind_group(id).is_some());

        assert!(
            reg.replace(id, 2, 2, &[0u8; 16]),
            "replace must report success for an id that exists"
        );
        assert!(
            reg.get_bind_group(id).is_some(),
            "the id must still resolve after replace -- Material.texture_id \
             stores it and is not rewritten"
        );
        assert_eq!(
            reg.get_size(id),
            Some((2, 2)),
            "replace must actually rebuild the texture with the new dimensions \
             -- a no-op that returns true would leave the old 1x1 texture in place"
        );
    }

    #[test]
    fn replace_reports_failure_for_an_unknown_texture_id() {
        let mut reg = make_registry();
        assert!(!reg.replace(9999, 1, 1, &[0, 0, 0, 255]));
        assert!(reg.get_bind_group(9999).is_none());
    }
}
