//! Baked lightmaps on the GPU: one array every lightmapped renderer reads.
//!
//! The shape is [`crate::cookie::CookieAtlas`]'s, for the same reason: WGSL
//! cannot index a variable number of separately bound textures, and the
//! sampled-texture budget is spent (`light_bgl_entries` plus the terrain's own
//! five sit exactly at the 16 `downlevel_defaults` allows -- see
//! `every_pipeline_fits_webgpus_sampled_texture_limit`). So each distinct
//! lightmap texture is drawn into a layer of one array, and a renderer carries
//! the layer index and the rectangle of the layer it owns.
//!
//! Where it differs from the cookie atlas: lightmaps do not change from frame
//! to frame, and a layer is 1024² (4 MiB) rather than 256², so a layer is
//! redrawn only when the texture behind it is rebuilt -- a hot reload, or a
//! streamed mip level in or out ([`crate::texture::GpuTextureRegistry::generation`]).
//!
//! # Encoding
//!
//! Layers hold RGBM ([`bsengine_core::LIGHTMAP_RGBM_RANGE`]) in a **linear**
//! 8-bit format. A directional lightmap's directions (`Lightmap::direction_path`,
//! `v * 0.5 + 0.5`) sit in the layer [`MAX_LIGHTMAPS`] above their
//! irradiance's -- the same array, so no binding more.
//! The source texture must therefore be linear too (`srgb:
//! false` in its sidecar): the blit copies whatever the sampler returns, and a
//! source that was decoded from sRGB on the way in would be stored as if its
//! encoded values were the light.

use crate::pipeline_cache::CachedCreate;
/// How many distinct lightmap textures a frame can use. A level with more
/// atlases than this should pack them tighter: the extra renderers shade
/// without a lightmap rather than with another renderer's.
pub const MAX_LIGHTMAPS: usize = 4;

/// The side of one layer, in texels. A source of another size is stretched
/// (or reduced through its mips) to it: lightmap coordinates are normalised,
/// so a renderer's rectangle stays correct either way.
pub const LIGHTMAP_SIZE: u32 = 1024;

/// Linear, so the RGBM values the baker wrote survive the copy unchanged.
const ATLAS_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// What a renderer carries to be lit from a lightmap: which texture, and
/// which rectangle of it, as `[scale_x, scale_y, offset_x, offset_y]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LightmapParams {
    /// The texture's id in the [`crate::texture::GpuTextureRegistry`].
    pub texture_id: u64,
    /// Its directions' texture, for a directional lightmap; `None` shades
    /// with the irradiance alone.
    pub direction_id: Option<u64>,
    /// Applied as `uv2 * scale + offset`.
    pub scale_offset: [f32; 4],
}

/// The array's declaration and the one function that reads it, for the mesh
/// shader. `spot_cookie_sampler` (bilinear, clamped) is declared by
/// `cookie_wgsl!`, which must come first: a lightmap wants exactly that
/// sampler -- bilinear so texels blend, clamped so a renderer's rectangle
/// never bleeds in the one beside it from the far edge.
///
/// `LIGHTMAP_RGBM_RANGE` here is [`bsengine_core::LIGHTMAP_RGBM_RANGE`]
/// (`the_wgsl_range_is_the_cores` holds them together).
macro_rules! lightmap_wgsl {
    () => {
        r#"
@group(2) @binding(22) var lightmap_atlas: texture_2d_array<f32>;
const LIGHTMAP_RGBM_RANGE: f32 = 8.0;

// The indirect light a lightmapped surface receives, in the same units the
// irradiance cube and the probe grid store (E / PI): the baker writes it that
// way so this replaces either without a conversion.
fn lightmap_irradiance(layer: f32, uv: vec2<f32>) -> vec3<f32> {
    let texel = textureSample(lightmap_atlas, spot_cookie_sampler, uv, i32(layer));
    return texel.rgb * texel.a * LIGHTMAP_RGBM_RANGE;
}

// The directional factor at a surface facing `n` whose bake faced `n_bake`:
// `(1 + v.n) / (1 + v.n_bake)`, `v` the texel's mean incoming direction.
// Linear in the normal, as Unity's dominant-direction lightmap and Unreal's
// L1 lightmap are; exactly 1 where the normal is the bake's, so a flat
// surface is lit as the plain lightmap lights it; at most 2, since `v` is no
// longer than 1 and comes from `n_bake`'s side. The directions sit
// `LIGHTMAP_DIRECTION_OFFSET` layers above their irradiance.
const LIGHTMAP_DIRECTION_OFFSET: i32 = 4;
fn lightmap_directionality(layer: f32, uv: vec2<f32>, n: vec3<f32>, n_bake: vec3<f32>) -> f32 {
    let v = textureSample(
        lightmap_atlas, spot_cookie_sampler, uv, i32(layer) + LIGHTMAP_DIRECTION_OFFSET
    ).xyz * 2.0 - vec3<f32>(1.0);
    return max(1.0 + dot(v, n), 0.0) / max(1.0 + dot(v, n_bake), 1e-4);
}
"#
    };
}
pub(crate) use lightmap_wgsl;

/// The array every lightmapped renderer reads, and the pass that fills it.
pub(crate) struct LightmapAtlas {
    /// Kept alive for the views; never read directly.
    _texture: wgpu::Texture,
    /// All layers, as the light bind group's binding 22 sees them.
    pub(crate) array_view: wgpu::TextureView,
    /// One render-target view per layer, for the blit.
    layer_views: Vec<wgpu::TextureView>,
    blit_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    blit_layout: std::sync::Arc<wgpu::BindGroupLayout>,
    /// Trilinear, so a source larger than a layer is read through its mips.
    blit_sampler: wgpu::Sampler,
    /// What each layer was last drawn from: `(texture id, its generation)`.
    resident: Vec<Option<(u64, u64)>>,
    /// How many layers have been drawn, ever. For tests: a layer that is
    /// redrawn every frame would otherwise be invisible.
    blits: u64,
}

impl LightmapAtlas {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("lightmap atlas"),
            size: wgpu::Extent3d {
                width: LIGHTMAP_SIZE,
                height: LIGHTMAP_SIZE,
                // The irradiance layers, then each one's directions.
                depth_or_array_layers: 2 * MAX_LIGHTMAPS as u32,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: ATLAS_FORMAT,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let array_view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("lightmap atlas view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let layer_views = (0..2 * MAX_LIGHTMAPS as u32)
            .map(|layer| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("lightmap layer"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
            .collect();
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("lightmap blit sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let blit_layout = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("lightmap blit bgl"),
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
        });
        let module = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lightmap blit"),
            source: wgpu::ShaderSource::Wgsl(crate::cookie::BLIT_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lightmap blit layout"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("lightmap blit"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: "vs",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: "fs",
                targets: &[Some(wgpu::ColorTargetState {
                    format: ATLAS_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Self {
            _texture: texture,
            array_view,
            layer_views,
            blit_pipeline,
            blit_layout,
            blit_sampler,
            resident: vec![None; 2 * MAX_LIGHTMAPS],
            blits: 0,
        }
    }

    /// Draws the textures this frame's renderers name into the array and
    /// returns each renderer's layer, `-1.0` for none -- the value the shader
    /// reads as "no lightmap" -- and whether its directions are there too.
    ///
    /// Renderers sharing a texture share its layer (and its directions, the
    /// first renderer's to name them). A texture that is not on the GPU yet
    /// (still loading, or a bad path) is no lightmap: the surface shades as
    /// it would without one rather than going black while it loads; a
    /// direction texture not there yet is a non-directional lightmap. A layer
    /// already holding the texture at its current generation is left alone,
    /// so a steady scene draws nothing here.
    pub(crate) fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: Option<&crate::texture::GpuTextureRegistry>,
        textures: &[Option<(u64, Option<u64>)>],
    ) -> Vec<(f32, bool)> {
        let mut layers = vec![(-1.0, false); textures.len()];
        let Some(registry) = registry else {
            return layers;
        };
        let on_gpu = |id: u64| registry.get_view(id).and(registry.generation(id));
        // Per layer: the irradiance (id, generation), and its directions'.
        let mut assigned: Vec<((u64, u64), Option<(u64, u64)>)> = Vec::new();
        for (slot, entry) in textures.iter().enumerate() {
            let Some((id, direction)) = *entry else {
                continue;
            };
            let Some(generation) = on_gpu(id) else {
                continue;
            };
            let layer = match assigned.iter().position(|((a, _), _)| *a == id) {
                Some(layer) => layer,
                None if assigned.len() == MAX_LIGHTMAPS => continue,
                None => {
                    let dir = direction.and_then(|d| Some((d, on_gpu(d)?)));
                    assigned.push(((id, generation), dir));
                    assigned.len() - 1
                }
            };
            layers[slot] = (layer as f32, assigned[layer].1.is_some());
        }

        // Every layer to draw: the irradiance at its index, its directions
        // `MAX_LIGHTMAPS` above.
        let wanted: Vec<(usize, (u64, u64))> = assigned
            .iter()
            .enumerate()
            .flat_map(|(layer, (irr, dir))| {
                std::iter::once((layer, *irr)).chain(dir.map(|d| (layer + MAX_LIGHTMAPS, d)))
            })
            .collect();
        let mut encoder = None;
        for &(layer, entry) in &wanted {
            if self.resident[layer] == Some(entry) {
                continue;
            }
            let Some(view) = registry.get_view(entry.0) else {
                continue;
            };
            self.resident[layer] = Some(entry);
            self.blits += 1;
            let encoder = encoder.get_or_insert_with(|| {
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("lightmap blit"),
                })
            });
            let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("lightmap blit bg"),
                layout: &self.blit_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                    },
                ],
            });
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lightmap blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.layer_views[layer],
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.blit_pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        // Layers not drawn into this frame no longer hold what the table
        // says, in the sense that the next frame may assign them something
        // else: forget them so a texture coming back is redrawn.
        for layer in 0..2 * MAX_LIGHTMAPS {
            if !wanted.iter().any(|&(l, _)| l == layer) {
                self.resident[layer] = None;
            }
        }
        if let Some(encoder) = encoder {
            queue.submit(Some(encoder.finish()));
        }
        layers
    }

    /// How many layers have been drawn since the atlas was made.
    pub(crate) fn blit_count(&self) -> u64 {
        self.blits
    }
}

#[cfg(test)]
mod tests {
    use bsengine_core::LIGHTMAP_RGBM_RANGE;

    /// The shader's range and the core's are one number, written twice
    /// because WGSL has no way to import a Rust constant.
    #[test]
    fn the_wgsl_range_is_the_cores() {
        let wgsl = lightmap_wgsl!();
        let line = wgsl
            .lines()
            .find(|l| l.contains("const LIGHTMAP_RGBM_RANGE"))
            .expect("the shader declares the range");
        let value: f32 = line
            .split('=')
            .nth(1)
            .and_then(|v| v.trim().trim_end_matches(';').parse().ok())
            .expect("the range parses");
        assert_eq!(value, LIGHTMAP_RGBM_RANGE);
    }

    /// The shader finds a layer's directions `LIGHTMAP_DIRECTION_OFFSET`
    /// above it and `update` draws them `MAX_LIGHTMAPS` above: one number, or
    /// every directional surface reads another lightmap's directions (or an
    /// empty layer, whose `v = -1` darkens it). The pixel test with two
    /// lightmaps sees a mismatch too; this names it.
    #[test]
    fn the_wgsl_direction_offset_is_max_lightmaps() {
        let wgsl = lightmap_wgsl!();
        let line = wgsl
            .lines()
            .find(|l| l.contains("const LIGHTMAP_DIRECTION_OFFSET"))
            .expect("the shader declares the offset");
        let value: usize = line
            .split('=')
            .nth(1)
            .and_then(|v| v.trim().trim_end_matches(';').parse().ok())
            .expect("the offset parses");
        assert_eq!(value, super::MAX_LIGHTMAPS);
    }
}
