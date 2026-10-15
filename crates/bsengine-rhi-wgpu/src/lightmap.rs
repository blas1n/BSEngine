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
//! 8-bit format. The source texture must therefore be linear too (`srgb:
//! false` in its sidecar): the blit copies whatever the sampler returns, and a
//! source that was decoded from sRGB on the way in would be stored as if its
//! encoded values were the light.

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
    blit_pipeline: wgpu::RenderPipeline,
    blit_layout: wgpu::BindGroupLayout,
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
                depth_or_array_layers: MAX_LIGHTMAPS as u32,
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
        let layer_views = (0..MAX_LIGHTMAPS as u32)
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
        let blit_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lightmap blit"),
            source: wgpu::ShaderSource::Wgsl(crate::cookie::BLIT_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("lightmap blit layout"),
            bind_group_layouts: &[&blit_layout],
            push_constant_ranges: &[],
        });
        let blit_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            resident: vec![None; MAX_LIGHTMAPS],
            blits: 0,
        }
    }

    /// Draws the textures this frame's renderers name into the array and
    /// returns each renderer's layer, `-1.0` for none -- the value the shader
    /// reads as "no lightmap".
    ///
    /// Renderers sharing a texture share its layer. A texture that is not on
    /// the GPU yet (still loading, or a bad path) is no lightmap: the surface
    /// shades as it would without one rather than going black while it loads.
    /// A layer already holding the texture at its current generation is left
    /// alone, so a steady scene draws nothing here.
    pub(crate) fn update(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: Option<&crate::texture::GpuTextureRegistry>,
        textures: &[Option<u64>],
    ) -> Vec<f32> {
        let mut layers = vec![-1.0; textures.len()];
        let Some(registry) = registry else {
            return layers;
        };
        let mut assigned: Vec<(u64, u64)> = Vec::new();
        for (slot, id) in textures.iter().enumerate() {
            let Some(id) = *id else { continue };
            let (Some(_), Some(generation)) = (registry.get_view(id), registry.generation(id))
            else {
                continue;
            };
            if let Some(layer) = assigned.iter().position(|(a, _)| *a == id) {
                layers[slot] = layer as f32;
                continue;
            }
            if assigned.len() == MAX_LIGHTMAPS {
                continue;
            }
            layers[slot] = assigned.len() as f32;
            assigned.push((id, generation));
        }

        let mut encoder = None;
        for (layer, entry) in assigned.iter().enumerate() {
            if self.resident[layer] == Some(*entry) {
                continue;
            }
            let Some(view) = registry.get_view(entry.0) else {
                continue;
            };
            self.resident[layer] = Some(*entry);
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
        // Layers past the ones assigned this frame no longer hold what the
        // table says, in the sense that the next frame may assign them
        // something else: forget them so a texture coming back is redrawn.
        for layer in assigned.len()..MAX_LIGHTMAPS {
            self.resident[layer] = None;
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
    use super::*;
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
}
