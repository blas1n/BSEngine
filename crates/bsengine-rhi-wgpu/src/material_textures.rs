//! A material's textures, as one bind group: group 3 of the mesh pipelines.
//!
//! Group 3 used to be the albedo texture alone, and every texture in the
//! registry carried a bind group of exactly that shape. A material with a
//! normal map needs two textures in one group -- WebGPU has four bind groups
//! and the other three are taken -- so the group is now the *material's*, built
//! here from whichever textures it names, and the default for any it does not:
//! white for the albedo, the ORM and the emissive map, and a flat normal for
//! the normal map.
//!
//! Each texture keeps its own sampler (filter and wrap are its import
//! settings), so a group pairs every view with its sampler.
//!
//! The groups are cached by what they hold -- each texture's id and its
//! [`GpuTextureRegistry::generation`], which changes whenever the object
//! behind the id does (hot reload, a streamed level), so a stale view is never
//! bound -- and a group no draw asked for in a frame is dropped at the next
//! [`MaterialTextures::begin_frame`], so the cache holds the materials on
//! screen and not every combination ever drawn.

use crate::pipeline_cache::CachedCreate;
use crate::texture::GpuTextureRegistry;
use std::collections::{HashMap, HashSet};

/// The textures one draw samples, by registry id; `None` is the default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MaterialTextureIds {
    /// The base colour.
    pub albedo: Option<u64>,
    /// The tangent-space normal map.
    pub normal: Option<u64>,
    /// Occlusion, roughness and metallic in R, G and B.
    pub orm: Option<u64>,
    /// The emitted colour.
    pub emissive: Option<u64>,
}

/// How many textures a material's group holds.
const TEXTURES: usize = 4;

/// What a cached group was built from: each texture's id and generation.
type Key = [Option<(u64, u64)>; TEXTURES];

/// The layout's entries: per texture, its view at `2 k` and its sampler at
/// `2 k + 1` -- albedo, normal, ORM, emissive. Albedo at 0/1, as the
/// single-texture group had it, so a custom shader that declares only those
/// still matches.
pub fn layout_entries() -> Vec<wgpu::BindGroupLayoutEntry> {
    (0..TEXTURES as u32)
        .flat_map(|k| {
            [
                wgpu::BindGroupLayoutEntry {
                    binding: 2 * k,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2 * k + 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ]
        })
        .collect()
}

/// Group 3's layout, its defaults, and the groups built so far.
pub struct MaterialTextures {
    layout: std::sync::Arc<wgpu::BindGroupLayout>,
    white: wgpu::TextureView,
    flat_normal: wgpu::TextureView,
    sampler: wgpu::Sampler,
    groups: HashMap<Key, wgpu::BindGroup>,
    used: HashSet<Key>,
    default_group: wgpu::BindGroup,
}

impl MaterialTextures {
    /// The layout and the two 1 x 1 defaults.
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let layout = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material textures"),
            entries: &layout_entries(),
        });
        let one_texel = |label, rgba: [u8; 4]| {
            let texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // Linear: neither default is a colour that wants decoding.
                // White is white either way; the flat normal must read back as
                // the (0.5, 0.5, 1) it encodes, not that curve's image.
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                texture.as_image_copy(),
                &rgba,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: None,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );
            texture.create_view(&wgpu::TextureViewDescriptor::default())
        };
        let white = one_texel("default albedo", [255, 255, 255, 255]);
        // Never sampled for a draw without a normal map -- the shader skips
        // the map when `normal_scale` is 0, because (128, 128, 255) decodes to
        // 0.0039 rather than 0 in x and y and would tilt every surface. Bound
        // only so the group is complete.
        let flat_normal = one_texel("default normal", [128, 128, 255, 255]);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("material default sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // White for the ORM and emissive maps too: every channel decodes to
        // exactly 1.0, so a material without one keeps its factors as they
        // are (occlusion 1, roughness and metallic times 1, the emissive
        // colour times 1).
        let default_group = Self::build(
            device,
            &layout,
            [
                (&white, &sampler),
                (&flat_normal, &sampler),
                (&white, &sampler),
                (&white, &sampler),
            ],
        );
        Self {
            layout,
            white,
            flat_normal,
            sampler,
            groups: HashMap::new(),
            used: HashSet::new(),
            default_group,
        }
    }

    /// Group 3's layout, for the pipelines.
    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    /// The group with every default: what a draw naming no texture binds.
    pub fn default_group(&self) -> &wgpu::BindGroup {
        &self.default_group
    }

    /// How many groups are cached, for tests.
    #[doc(hidden)]
    pub fn cached(&self) -> usize {
        self.groups.len()
    }

    fn build(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        textures: [(&wgpu::TextureView, &wgpu::Sampler); TEXTURES],
    ) -> wgpu::BindGroup {
        let entries: Vec<wgpu::BindGroupEntry> = textures
            .iter()
            .enumerate()
            .flat_map(|(k, (view, sampler))| {
                [
                    wgpu::BindGroupEntry {
                        binding: 2 * k as u32,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2 * k as u32 + 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ]
            })
            .collect();
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material textures"),
            layout,
            entries: &entries,
        })
    }

    /// Starts a frame: the groups the last frame did not use are dropped.
    pub fn begin_frame(&mut self) {
        let used = std::mem::take(&mut self.used);
        self.groups.retain(|key, _| used.contains(key));
    }

    /// What a group for `ids` is built from now: a texture the registry does
    /// not hold (not uploaded yet, or never) is the default.
    fn key(ids: MaterialTextureIds, registry: Option<&GpuTextureRegistry>) -> Key {
        let resolve = |id: Option<u64>| {
            let id = id?;
            Some((id, registry?.generation(id)?))
        };
        [
            resolve(ids.albedo),
            resolve(ids.normal),
            resolve(ids.orm),
            resolve(ids.emissive),
        ]
    }

    /// Builds (or keeps) the group for `ids`, and marks it used this frame.
    /// Call for every draw before the passes that bind [`Self::group`].
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        ids: MaterialTextureIds,
        registry: Option<&GpuTextureRegistry>,
    ) {
        let key = Self::key(ids, registry);
        self.used.insert(key);
        if key == [None; TEXTURES] || self.groups.contains_key(&key) {
            return;
        }
        let group = {
            let sampler = &self.sampler;
            let pick =
                |slot: Option<(u64, u64)>, default| -> (&wgpu::TextureView, &wgpu::Sampler) {
                    slot.and_then(|(id, _)| {
                        let r = registry?;
                        Some((r.get_view(id)?, r.get_sampler(id)?))
                    })
                    .unwrap_or((default, sampler))
                };
            let [albedo, normal, orm, emissive] = key;
            Self::build(
                device,
                &self.layout,
                [
                    pick(albedo, &self.white),
                    pick(normal, &self.flat_normal),
                    pick(orm, &self.white),
                    pick(emissive, &self.white),
                ],
            )
        };
        self.groups.insert(key, group);
    }

    /// The group for `ids`, as prepared this frame; the default group for one
    /// that was not (which a caller that prepared every draw never sees).
    pub fn group(
        &self,
        ids: MaterialTextureIds,
        registry: Option<&GpuTextureRegistry>,
    ) -> &wgpu::BindGroup {
        self.groups
            .get(&Self::key(ids, registry))
            .unwrap_or(&self.default_group)
    }
}
