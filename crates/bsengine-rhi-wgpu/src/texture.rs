use bsengine_core::{TextureFilter, TextureImportSettings, TextureWrap};
use bsengine_ecs::Resource;
use std::collections::HashMap;
use std::sync::Arc;

struct GpuTexture {
    _texture: crate::profiler::TrackedTexture,
    _view: wgpu::TextureView,
    /// Per texture, because filter and wrap are import settings: two
    /// textures on screen at once can legitimately want different ones, and
    /// one registry-wide sampler could only ever honour one of them.
    _sampler: wgpu::Sampler,
    bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
    settings: TextureImportSettings,
}

/// Owns every GPU texture uploaded for the running app, keyed by a registry-assigned id.
#[derive(Resource)]
pub struct GpuTextureRegistry {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    textures: HashMap<u64, GpuTexture>,
    next_id: u64,
}

impl GpuTextureRegistry {
    /// Creates an empty registry bound to the given wgpu device/queue.
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let bgl = Self::create_bgl(&device);
        Self {
            device,
            queue,
            bgl,
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
    /// RGBA8 texture with [`TextureImportSettings::raw`], and returns its
    /// assigned id.
    pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<u64, String> {
        let img = image::load_from_memory(bytes).map_err(|e| format!("image decode: {e}"))?;
        let rgba = img.to_rgba8();
        let (width, height) = rgba.dimensions();
        Ok(self.load_from_rgba(width, height, &rgba))
    }

    /// Uploads already-decoded RGBA8 pixel data as a new texture with
    /// [`TextureImportSettings::raw`] -- linear, unmipped, clamped, exactly
    /// what every upload was before import settings existed -- and returns
    /// its assigned id. A texture that came from a file goes through
    /// [`load_with`](Self::load_with) with its sidecar's settings instead.
    pub fn load_from_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> u64 {
        self.load_with(width, height, rgba, TextureImportSettings::raw())
    }

    /// Uploads already-decoded RGBA8 pixel data with the given import
    /// settings and returns the new texture's assigned id.
    pub fn load_with(
        &mut self,
        width: u32,
        height: u32,
        rgba: &[u8],
        settings: TextureImportSettings,
    ) -> u64 {
        let tex = self.build(width, height, rgba, settings);
        let id = self.next_id;
        self.next_id += 1;
        self.textures.insert(id, tex);
        id
    }

    /// Rebuilds an already-loaded texture from new pixels, keeping its id and
    /// its import settings. Returns whether `id` was loaded.
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
        let Some(settings) = self.textures.get(&id).map(|t| t.settings) else {
            return false;
        };
        self.replace_with(id, width, height, rgba, settings)
    }

    /// [`replace`](Self::replace) with new import settings as well as new
    /// pixels -- what a reload driven by an edited sidecar needs.
    #[must_use]
    pub fn replace_with(
        &mut self,
        id: u64,
        width: u32,
        height: u32,
        rgba: &[u8],
        settings: TextureImportSettings,
    ) -> bool {
        if !self.textures.contains_key(&id) {
            return false;
        }
        let tex = self.build(width, height, rgba, settings);
        self.textures.insert(id, tex);
        true
    }

    fn build(
        &self,
        width: u32,
        height: u32,
        rgba: &[u8],
        settings: TextureImportSettings,
    ) -> GpuTexture {
        // The format is what makes `srgb` mean anything: an `…Srgb` view
        // decodes on sample, so the shader receives linear light without a
        // `pow` of its own. Uploaded bytes are identical either way.
        let format = if settings.srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let levels: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> = if settings.mipmaps {
            mip_chain(width, height, rgba)
        } else {
            vec![(width, height, std::borrow::Cow::Borrowed(rgba))]
        };
        let texture = crate::profiler::create_tracked_texture(
            &self.device,
            &wgpu::TextureDescriptor {
                label: Some("user texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: levels.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        for (level, (w, h, pixels)) in levels.iter().enumerate() {
            self.queue.write_texture(
                wgpu::ImageCopyTexture {
                    texture: &texture,
                    mip_level: level as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                pixels,
                wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(4 * w),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: *w,
                    height: *h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let (mag_filter, min_filter) = match settings.filter {
            TextureFilter::Linear => (wgpu::FilterMode::Linear, wgpu::FilterMode::Linear),
            TextureFilter::Nearest => (wgpu::FilterMode::Nearest, wgpu::FilterMode::Nearest),
        };
        let address_mode = match settings.wrap {
            TextureWrap::Repeat => wgpu::AddressMode::Repeat,
            TextureWrap::Clamp => wgpu::AddressMode::ClampToEdge,
            TextureWrap::Mirror => wgpu::AddressMode::MirrorRepeat,
        };
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("user texture sampler"),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            address_mode_w: address_mode,
            mag_filter,
            min_filter,
            // Between mip levels, not within one: with a single level this
            // never engages, and with a chain it is what stops the level
            // switch from showing as a seam.
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
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
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        GpuTexture {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group,
            width,
            height,
            settings,
        }
    }

    /// Looks up a previously loaded texture's bind group by id.
    pub fn get_bind_group(&self, id: u64) -> Option<&wgpu::BindGroup> {
        self.textures.get(&id).map(|t| &t.bind_group)
    }

    /// Looks up a previously loaded texture's raw view by id. Used by
    /// terrain rendering, which needs several textures' views in one
    /// bind group rather than this registry's own 2-entry (texture,
    /// sampler) layout.
    pub fn get_view(&self, id: u64) -> Option<&wgpu::TextureView> {
        self.textures.get(&id).map(|t| &t._view)
    }

    /// Looks up a previously loaded texture's pixel dimensions by id.
    pub fn get_size(&self, id: u64) -> Option<(u32, u32)> {
        self.textures.get(&id).map(|t| (t.width, t.height))
    }

    /// The import settings a texture was uploaded with.
    pub fn get_settings(&self, id: u64) -> Option<TextureImportSettings> {
        self.textures.get(&id).map(|t| t.settings)
    }

    /// What the settings turned into on the GPU: the texture's format and
    /// how many mip levels it holds. What a test can observe; a sampler's
    /// filter and wrap can only be seen by rendering with it.
    pub fn get_gpu_shape(&self, id: u64) -> Option<(wgpu::TextureFormat, u32)> {
        self.textures
            .get(&id)
            .map(|t| (t._texture.format(), t._texture.mip_level_count()))
    }
}

/// The full mip chain of an RGBA8 image, level 0 first, down to 1x1.
///
/// Built on the CPU with a box filter rather than on the GPU with a render
/// pass per level: user textures are uploaded once, the chain costs a third
/// of the image again in bytes, and a compute or blit path would need a
/// pipeline and a bind group per level for the same result.
fn mip_chain(width: u32, height: u32, rgba: &[u8]) -> Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> {
    let mut levels: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> =
        vec![(width, height, std::borrow::Cow::Borrowed(rgba))];
    let (mut w, mut h) = (width, height);
    while w > 1 || h > 1 {
        let (nw, nh, next) = {
            let (_, _, prev) = levels.last().expect("level 0 is always present");
            downsample(w, h, prev)
        };
        levels.push((nw, nh, std::borrow::Cow::Owned(next)));
        w = nw;
        h = nh;
    }
    levels
}

/// Halves an RGBA8 image with a 2x2 box filter, rounding odd dimensions up
/// so a 3-wide row still yields 2 texels, the second averaging the clamped
/// edge with itself.
fn downsample(width: u32, height: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    let nw = (width / 2).max(1);
    let nh = (height / 2).max(1);
    let mut out = Vec::with_capacity((nw * nh * 4) as usize);
    let texel = |x: u32, y: u32| -> [u32; 4] {
        let x = x.min(width - 1);
        let y = y.min(height - 1);
        let i = ((y * width + x) * 4) as usize;
        [
            rgba[i] as u32,
            rgba[i + 1] as u32,
            rgba[i + 2] as u32,
            rgba[i + 3] as u32,
        ]
    };
    for y in 0..nh {
        for x in 0..nw {
            let (sx, sy) = (x * 2, y * 2);
            let a = texel(sx, sy);
            let b = texel(sx + 1, sy);
            let c = texel(sx, sy + 1);
            let d = texel(sx + 1, sy + 1);
            for ch in 0..4 {
                out.push(((a[ch] + b[ch] + c[ch] + d[ch] + 2) / 4) as u8);
            }
        }
    }
    (nw, nh, out)
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

    #[test]
    fn get_view_resolves_a_loaded_texture_and_none_for_unknown() {
        let mut reg = make_registry();
        assert!(reg.get_view(999).is_none(), "unknown id must return None");

        let id = reg.load_from_rgba(2, 2, &[0u8; 16]);
        assert!(
            reg.get_view(id).is_some(),
            "a loaded texture's view must resolve"
        );
    }

    /// The two settings the GPU object itself records. `srgb` picks the
    /// format and `mipmaps` the level count; each is asserted against its
    /// opposite so a build that ignored the setting fails on one side.
    #[test]
    fn srgb_and_mipmaps_become_the_textures_format_and_level_count() {
        let mut reg = make_registry();
        let pixels = [128u8; 8 * 4 * 4];

        let colour = reg.load_with(8, 4, &pixels, TextureImportSettings::default());
        assert_eq!(
            reg.get_gpu_shape(colour),
            Some((wgpu::TextureFormat::Rgba8UnormSrgb, 4)),
            "sRGB format, and 8x4 has levels 8x4, 4x2, 2x1, 1x1"
        );

        let data = reg.load_with(
            8,
            4,
            &pixels,
            TextureImportSettings {
                srgb: false,
                mipmaps: false,
                ..Default::default()
            },
        );
        assert_eq!(
            reg.get_gpu_shape(data),
            Some((wgpu::TextureFormat::Rgba8Unorm, 1))
        );

        let raw = reg.load_from_rgba(8, 4, &pixels);
        assert_eq!(
            reg.get_settings(raw),
            Some(TextureImportSettings::raw()),
            "the settings-less path is the old upload, not the new default"
        );
        assert_eq!(
            reg.get_gpu_shape(raw),
            Some((wgpu::TextureFormat::Rgba8Unorm, 1))
        );
    }

    /// `replace` keeps the settings the texture was uploaded with;
    /// `replace_with` changes them. A reload that silently dropped a texture
    /// back to `raw` would undo its sidecar on the first edit of its pixels.
    #[test]
    fn replace_keeps_import_settings_and_replace_with_changes_them() {
        let mut reg = make_registry();
        let id = reg.load_with(2, 2, &[0u8; 16], TextureImportSettings::default());

        assert!(reg.replace(id, 1, 1, &[0, 0, 0, 255]));
        assert_eq!(reg.get_settings(id), Some(TextureImportSettings::default()));
        assert_eq!(
            reg.get_gpu_shape(id),
            Some((wgpu::TextureFormat::Rgba8UnormSrgb, 1)),
            "still sRGB; a 1x1 image has one level whatever `mipmaps` says"
        );

        let data = TextureImportSettings {
            srgb: false,
            ..Default::default()
        };
        assert!(reg.replace_with(id, 2, 2, &[0u8; 16], data));
        assert_eq!(reg.get_settings(id), Some(data));
        assert_eq!(
            reg.get_gpu_shape(id),
            Some((wgpu::TextureFormat::Rgba8Unorm, 2))
        );
    }

    /// The box filter, on numbers small enough to check by hand -- and on an
    /// odd width, where the last output texel averages the clamped edge.
    #[test]
    fn downsample_averages_two_by_two_blocks_and_clamps_odd_edges() {
        // 3x2 image: rows of [0, 100, 200] and [0, 100, 200] in R, opaque.
        let mut rgba = Vec::new();
        for _ in 0..2 {
            for r in [0u8, 100, 200] {
                rgba.extend_from_slice(&[r, 0, 0, 255]);
            }
        }
        let (w, h, out) = downsample(3, 2, &rgba);
        assert_eq!((w, h), (1, 1), "3x2 halves to 1x1 (3/2 = 1)");
        assert_eq!(out, vec![50, 0, 0, 255], "(0 + 100 + 0 + 100 + 2) / 4 = 50");

        let (w, h, out) = downsample(
            4,
            1,
            &[0, 0, 0, 255, 200, 0, 0, 255, 60, 0, 0, 255, 0, 0, 0, 255],
        );
        assert_eq!((w, h), (2, 1));
        assert_eq!(
            out[0], 100,
            "(0 + 200 + 0 + 200 + 2) / 4, the row clamped below itself"
        );
        assert_eq!(out[4], 30, "(60 + 0 + 60 + 0 + 2) / 4");

        let chain = mip_chain(5, 3, &[255u8; 5 * 3 * 4]);
        let sizes: Vec<(u32, u32)> = chain.iter().map(|(w, h, _)| (*w, *h)).collect();
        assert_eq!(sizes, vec![(5, 3), (2, 1), (1, 1)]);
    }
}
