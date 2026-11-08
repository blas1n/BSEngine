//! A terrain's four layers of one kind -- their colours, or their normal maps
//! -- as one texture array the terrain shader indexes by layer.
//!
//! Why arrays: WebGPU allows sixteen sampled textures per shader stage
//! (`downlevel_defaults`), and a terrain bound one texture per layer per
//! kind. Four colours, four normal maps and the weights were nine, with six
//! more in its lighting group: fifteen, so a third kind of map (the layers'
//! ORM maps) had nowhere to go. One array per kind is one binding however
//! many layers it holds. Unity and Unreal bind each layer's textures
//! separately -- their desktop limits are far above sixteen -- while Godot's
//! Terrain3D keeps each kind in a `Texture2DArray`, as this does.
//!
//! The arrays are copies of the layers' own textures, which stay where the
//! `TextureCache` keeps them (a material may share one). A copy is redone
//! when one of its sources' registry generation changes -- a hot reload, a
//! streamed level in or out -- and dropped in the frame no terrain asks for
//! it. A video playing on a terrain layer rewrites its texture in place
//! without a new generation, so it would show its first frame; the cookie
//! atlas, which does take video, redraws every frame instead, which for a
//! terrain's full-size layers and their mips would cost every frame.
//!
//! Two ways to fill one. When every layer's texture has the same format,
//! size and mip count -- what a terrain authored with one texture size has
//! -- each level is copied as it is: exact, and a block-compressed layer
//! stays compressed. Otherwise the layers are drawn into an uncompressed
//! array at the largest layer's size, each through its own mips, and the
//! array's mips are drawn down from its level 0: layers authored at
//! different sizes still work, at the memory of an uncompressed copy.
//! Terrain3D asks for one size and format instead; resampling costs a
//! little sharpness on the smaller layers and refuses no terrain.

use std::collections::HashMap;

/// Which of a layer's textures an array holds, which decides what format a
/// drawn (not copied) array is stored in.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum LayerKind {
    /// The layers' colours: sRGB, as each layer texture was.
    Colour,
    /// The layers' normal maps: linear, as `TextureCache::upload_data`
    /// uploads them.
    Data,
}

impl LayerKind {
    /// The format an array of this kind is drawn into.
    fn drawn_format(self) -> wgpu::TextureFormat {
        match self {
            LayerKind::Colour => wgpu::TextureFormat::Rgba8UnormSrgb,
            LayerKind::Data => wgpu::TextureFormat::Rgba8Unorm,
        }
    }
}

/// How the arrays have been filled, for tests: a cache that rebuilt every
/// frame and one that never rebuilt both draw a correct first frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TerrainLayerArrayStats {
    /// Arrays alive now.
    pub live: usize,
    /// Arrays filled by copying the layers level by level, ever.
    pub copied: u64,
    /// Arrays filled by drawing the layers into them, ever.
    pub drawn: u64,
}

/// One array, and what it was made from.
struct Entry {
    /// Each layer's source generation when the array was filled, `None` for a
    /// layer with no texture (none named, or not on the GPU yet). Any
    /// difference from the registry's now means the array is stale.
    generations: [Option<u64>; 4],
    /// Kept alive for the view; never read directly.
    _texture: crate::profiler::TrackedTexture,
    view: wgpu::TextureView,
    /// Asked for since the last [`TerrainLayerArrays::sweep`].
    used: bool,
}

/// The terrain's layer arrays, keyed by kind and the four layers' texture
/// ids, and the pass that draws layers into them.
pub(crate) struct TerrainLayerArrays {
    entries: HashMap<(LayerKind, [Option<u64>; 4]), Entry>,
    /// Draws one texture into one layer (`cookie::BLIT_WGSL`), one pipeline
    /// per drawn format.
    pipelines: HashMap<wgpu::TextureFormat, std::sync::Arc<wgpu::RenderPipeline>>,
    blit_layout: std::sync::Arc<wgpu::BindGroupLayout>,
    /// Trilinear and clamped: a source larger than the array is read through
    /// its mips, and an edge texel does not bleed the opposite edge in.
    blit_sampler: wgpu::Sampler,
    copied: u64,
    drawn: u64,
}

impl TerrainLayerArrays {
    pub(crate) fn new(device: &wgpu::Device) -> Self {
        use crate::pipeline_cache::CachedCreate;
        let blit_layout = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("terrain layer blit bgl"),
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
            label: Some("terrain layer blit"),
            source: wgpu::ShaderSource::Wgsl(crate::cookie::BLIT_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("terrain layer blit layout"),
            bind_group_layouts: &[Some(&blit_layout)],
            immediate_size: 0,
        });
        let pipelines = [LayerKind::Colour, LayerKind::Data]
            .into_iter()
            .map(|kind| {
                let format = kind.drawn_format();
                let pipeline = device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("terrain layer blit"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &module,
                        entry_point: Some("vs"),
                        buffers: &[],
                        compilation_options: Default::default(),
                    },
                    fragment: Some(wgpu::FragmentState {
                        module: &module,
                        entry_point: Some("fs"),
                        targets: &[Some(wgpu::ColorTargetState {
                            format,
                            blend: None,
                            write_mask: wgpu::ColorWrites::ALL,
                        })],
                        compilation_options: Default::default(),
                    }),
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    multiview_mask: None,
                    cache: None,
                });
                (format, pipeline)
            })
            .collect();
        let blit_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("terrain layer blit sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        Self {
            entries: HashMap::new(),
            pipelines,
            blit_layout,
            blit_sampler,
            copied: 0,
            drawn: 0,
        }
    }

    /// The array holding `ids`' textures as layers 0-3, filled now if it is
    /// new or stale, and which layers have a texture in it (bit `l` for layer
    /// `l`). A layer whose id is `None`, or whose texture is not on the GPU,
    /// is left empty: what the shader does with an empty layer is the
    /// caller's to decide, by the mask.
    pub(crate) fn get(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        registry: &crate::texture::GpuTextureRegistry,
        kind: LayerKind,
        ids: [Option<u64>; 4],
    ) -> (wgpu::TextureView, u32) {
        let generations = ids.map(|id| id.and_then(|id| registry.generation(id)));
        let mask = generations
            .iter()
            .enumerate()
            .filter(|(_, g)| g.is_some())
            .fold(0, |mask, (l, _)| mask | 1 << l);
        let key = (kind, ids);
        let stale = self
            .entries
            .get(&key)
            .is_none_or(|e| e.generations != generations);
        if stale {
            let sources: [Option<(&wgpu::Texture, &wgpu::TextureView)>; 4] =
                std::array::from_fn(|l| {
                    generations[l]?;
                    let id = ids[l]?;
                    Some((registry.get_texture(id)?, registry.get_view(id)?))
                });
            let (texture, view) = self.fill(device, queue, kind, &sources);
            self.entries.insert(
                key,
                Entry {
                    generations,
                    _texture: texture,
                    view,
                    used: false,
                },
            );
        }
        let entry = self.entries.get_mut(&key).expect("filled above");
        entry.used = true;
        (entry.view.clone(), mask)
    }

    /// Drops every array nothing asked for since the last sweep: a terrain
    /// that was removed, or one whose layers changed to other textures.
    /// Called once a frame, after the terrain is drawn.
    pub(crate) fn sweep(&mut self) {
        self.entries.retain(|_, e| std::mem::take(&mut e.used));
    }

    pub(crate) fn stats(&self) -> TerrainLayerArrayStats {
        TerrainLayerArrayStats {
            live: self.entries.len(),
            copied: self.copied,
            drawn: self.drawn,
        }
    }

    /// A new array holding `sources` as its layers; see the module docs for
    /// which of the two ways it is filled.
    fn fill(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        kind: LayerKind,
        sources: &[Option<(&wgpu::Texture, &wgpu::TextureView)>; 4],
    ) -> (crate::profiler::TrackedTexture, wgpu::TextureView) {
        let present: Vec<(u32, &wgpu::Texture, &wgpu::TextureView)> = sources
            .iter()
            .enumerate()
            .filter_map(|(l, s)| s.map(|(t, v)| (l as u32, t, v)))
            .collect();
        let shape = |t: &wgpu::Texture| (t.format(), t.width(), t.height(), t.mip_level_count());
        let copyable = present
            .first()
            .is_some_and(|(_, first, _)| present.iter().all(|(_, t, _)| shape(t) == shape(first)));
        let (format, width, height, mips) = match present.first() {
            Some((_, first, _)) if copyable => shape(first),
            _ => {
                let width = present.iter().map(|(_, t, _)| t.width()).max().unwrap_or(1);
                let height = present
                    .iter()
                    .map(|(_, t, _)| t.height())
                    .max()
                    .unwrap_or(1);
                // A full chain when any layer had mips, so a layer is not
                // read at its full size into the distance; one level when
                // none did, which is what each of them asked for.
                let mips = if present.iter().any(|(_, t, _)| t.mip_level_count() > 1) {
                    width.max(height).ilog2() + 1
                } else {
                    1
                };
                (kind.drawn_format(), width, height, mips)
            }
        };
        let texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("terrain layer array"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 4,
                },
                mip_level_count: mips,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_DST
                    | if copyable {
                        wgpu::TextureUsages::empty()
                    } else {
                        wgpu::TextureUsages::RENDER_ATTACHMENT
                    },
                view_formats: &[],
            },
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("terrain layer array"),
        });
        if copyable {
            for &(layer, source, _) in &present {
                for level in 0..mips {
                    // Whole blocks for a compressed level smaller than one,
                    // as `GpuTextureRegistry` uploads them.
                    let extent = wgpu::Extent3d {
                        depth_or_array_layers: 1,
                        ..source
                            .size()
                            .mip_level_size(level, wgpu::TextureDimension::D2)
                    }
                    .physical_size(format);
                    encoder.copy_texture_to_texture(
                        wgpu::TexelCopyTextureInfo {
                            texture: source,
                            mip_level: level,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        wgpu::TexelCopyTextureInfo {
                            texture: &texture,
                            mip_level: level,
                            origin: wgpu::Origin3d {
                                x: 0,
                                y: 0,
                                z: layer,
                            },
                            aspect: wgpu::TextureAspect::All,
                        },
                        extent,
                    );
                }
            }
            self.copied += 1;
        } else {
            let pipeline = self.pipelines[&format].clone();
            let level_view = |layer: u32, level: u32| {
                texture.create_view(&wgpu::TextureViewDescriptor {
                    label: Some("terrain layer array level"),
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            };
            for &(layer, _, source_view) in &present {
                // Level 0 from the layer's own texture, through its mips;
                // each level after from the one above it.
                for level in 0..mips {
                    let from = (level > 0).then(|| level_view(layer, level - 1));
                    let from = from.as_ref().unwrap_or(source_view);
                    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("terrain layer blit bg"),
                        layout: &self.blit_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(from),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(&self.blit_sampler),
                            },
                        ],
                    });
                    let target = level_view(layer, level);
                    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("terrain layer blit"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &target,
                            resolve_target: None,
                            depth_slice: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(&pipeline);
                    pass.set_bind_group(0, &bind_group, &[]);
                    pass.draw(0..3, 0..1);
                }
            }
            self.drawn += 1;
        }
        queue.submit(Some(encoder.finish()));
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("terrain layer array view"),
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        (texture, view)
    }
}
