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
    /// Present for a texture whose mips are streamed; see [`Streamed`].
    streamed: Option<Streamed>,
}

/// The streaming side of a texture: the whole mip chain, kept on the CPU so
/// levels can be brought in and dropped, and which of them the GPU currently
/// holds.
///
/// # Residency is a texture object, not a flag
///
/// A `wgpu::Texture` allocates every mip level it is declared with, so a
/// texture "with only its small mips resident" cannot be one object with
/// some levels left unwritten -- the memory would be spent regardless, and
/// the point of streaming is the memory. Unreal reallocates a streamed
/// texture at each residency change and this does the same: the GPU object
/// is exactly as large as the resident levels, and raising or lowering
/// residency builds a new one. `resident_base` is the index into `levels` of
/// the largest resident level; `0` is fully resident.
///
/// The chain stays in RAM for now, which is the price of streaming from
/// memory rather than from disk. Unity and Unreal stream from disk; that is
/// the next step, and it changes nothing about how residency is expressed.
struct Streamed {
    /// Level 0 first, down to 1x1, as [`mip_chain`] produces them.
    levels: Vec<(u32, u32, Vec<u8>)>,
    resident_base: u32,
    /// The chain index the texture *ought* to have resident, from the
    /// largest it is drawn on screen: what [`GpuTextureRegistry::set_wants`]
    /// records each frame and [`GpuTextureRegistry::step_streaming`] moves
    /// `resident_base` towards. `0` -- whole -- until something says
    /// otherwise, so a texture nothing on screen uses (a UI image, a
    /// particle sheet) streams to full as it did before wants existed.
    wanted_base: u32,
}

/// What one [`GpuTextureRegistry::step_streaming`] call did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamingStep {
    /// Brought the next larger level of this texture in.
    Raised(u64),
    /// Dropped the largest resident level of this texture.
    Lowered(u64),
}

/// The chain index of the smallest level that is at least `pixels` across,
/// then `bias` levels smaller, clamped to the chain. `pixels` is how large
/// the texture is drawn on screen; a level that size has one texel per
/// pixel, which is where more texels stop being visible.
///
/// The levels' `(width, height)` are level 0 first, halving. A non-finite
/// or huge `pixels` wants level 0; zero or negative wants the last level.
fn wanted_base_for(levels: &[(u32, u32, Vec<u8>)], pixels: f32, bias: i32) -> u32 {
    let last = levels.len().saturating_sub(1) as i64;
    // The last index whose larger dimension still covers `pixels`.
    let base = levels
        .iter()
        .rposition(|(w, h, _)| (*w).max(*h) as f32 >= pixels)
        .unwrap_or(0) as i64;
    (base + bias as i64).clamp(0, last) as u32
}

/// Largest dimension of the levels a streamed texture starts with: enough to
/// draw a recognisable stand-in immediately, small enough that a scene full of
/// streamed textures costs almost nothing to bring up. Unity's streaming
/// starts from its "minimum mip" the same way.
pub const STREAMING_INITIAL_MAX_DIM: u32 = 64;

/// Owns every GPU texture uploaded for the running app, keyed by a registry-assigned id.
#[derive(Resource)]
pub struct GpuTextureRegistry {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    bgl: wgpu::BindGroupLayout,
    textures: HashMap<u64, GpuTexture>,
    next_id: u64,
    /// The streamed texture `raise_next_pending` last brought a level in
    /// on, so the next call moves on to another one.
    last_raised: u64,
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
            last_raised: 0,
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
        let levels: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> = if settings.mipmaps {
            mip_chain(width, height, rgba)
        } else {
            vec![(width, height, std::borrow::Cow::Borrowed(rgba))]
        };
        // A streamed texture starts with only its small levels on the GPU
        // and keeps the chain to bring the rest in from. Streaming a texture
        // with no chain would have nothing to stream, so it uploads whole.
        let streamed = if settings.streaming && levels.len() > 1 {
            let resident_base = levels
                .iter()
                .position(|(w, h, _)| (*w).max(*h) <= STREAMING_INITIAL_MAX_DIM)
                .unwrap_or(levels.len() - 1) as u32;
            Some(Streamed {
                levels: levels
                    .iter()
                    .map(|(w, h, p)| (*w, *h, p.to_vec()))
                    .collect(),
                resident_base,
                wanted_base: 0,
            })
        } else {
            None
        };
        let resident_base = streamed.as_ref().map_or(0, |s| s.resident_base);
        let (texture, view) = self.upload_levels(&levels, resident_base, settings);
        let (sampler, bind_group) = self.sampler_and_bind_group(&view, settings);
        GpuTexture {
            _texture: texture,
            _view: view,
            _sampler: sampler,
            bind_group,
            width,
            height,
            settings,
            streamed,
        }
    }

    /// Creates the GPU texture holding `levels[resident_base..]` -- exactly
    /// those, so its footprint is what is resident -- writes them, and views
    /// the whole object. Level `resident_base` of the chain is level 0 of the
    /// object; the sampler never knows the larger levels exist.
    fn upload_levels(
        &self,
        levels: &[(u32, u32, std::borrow::Cow<'_, [u8]>)],
        resident_base: u32,
        settings: TextureImportSettings,
    ) -> (crate::profiler::TrackedTexture, wgpu::TextureView) {
        // The format is what makes `srgb` mean anything: an `…Srgb` view
        // decodes on sample, so the shader receives linear light without a
        // `pow` of its own. Uploaded bytes are identical either way.
        let format = if settings.srgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        };
        let resident = &levels[resident_base as usize..];
        let (width, height, _) = &resident[0];
        let texture = crate::profiler::create_tracked_texture(
            &self.device,
            &wgpu::TextureDescriptor {
                label: Some("user texture"),
                size: wgpu::Extent3d {
                    width: *width,
                    height: *height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: resident.len() as u32,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        for (level, (w, h, pixels)) in resident.iter().enumerate() {
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
        (texture, view)
    }

    /// Whether `id` is a streamed texture.
    pub fn is_streaming(&self, id: u64) -> bool {
        self.textures.get(&id).is_some_and(|t| t.streamed.is_some())
    }

    /// A streamed texture's residency as `(resident_base, level_count)`:
    /// the chain index of the largest level on the GPU (`0` when fully
    /// resident) and how many levels the chain has. `None` for a texture
    /// that is not streamed.
    pub fn residency(&self, id: u64) -> Option<(u32, u32)> {
        let s = self.textures.get(&id)?.streamed.as_ref()?;
        Some((s.resident_base, s.levels.len() as u32))
    }

    /// Brings one more level of a streamed texture onto the GPU -- the next
    /// larger one -- rebuilding the GPU object at the new size, so its
    /// footprint grows by exactly that level. Returns whether a level was
    /// brought in: `false` for a texture that is not streamed or is already
    /// fully resident.
    pub fn raise_residency(&mut self, id: u64) -> bool {
        self.set_residency(id, |base| base.checked_sub(1))
    }

    /// Drops the largest resident level of a streamed texture, rebuilding
    /// the GPU object without it. The smallest level always stays, so the
    /// texture keeps drawing something. Returns whether a level was dropped.
    pub fn lower_residency(&mut self, id: u64) -> bool {
        self.set_residency(id, |base| Some(base + 1))
    }

    fn set_residency(&mut self, id: u64, next: impl FnOnce(u32) -> Option<u32>) -> bool {
        let Some(tex) = self.textures.get(&id) else {
            return false;
        };
        let Some(streamed) = tex.streamed.as_ref() else {
            return false;
        };
        let Some(base) = next(streamed.resident_base) else {
            return false;
        };
        if base as usize >= streamed.levels.len() {
            return false;
        }
        let levels: Vec<(u32, u32, std::borrow::Cow<'_, [u8]>)> = streamed
            .levels
            .iter()
            .map(|(w, h, p)| (*w, *h, std::borrow::Cow::Borrowed(p.as_slice())))
            .collect();
        let settings = tex.settings;
        // Rebuilt from the chain rather than copied GPU-to-GPU: the levels
        // below the one being brought in are a third of its size put
        // together, so the re-upload costs about what the new level does,
        // and it keeps this to one queue write per level with no encoder.
        let (texture, view) = self.upload_levels(&levels, base, settings);
        let (sampler, bind_group) = self.sampler_and_bind_group(&view, settings);
        drop(levels);
        let tex = self.textures.get_mut(&id).expect("looked up above");
        tex.streamed.as_mut().expect("checked above").resident_base = base;
        // The bind group is what a draw binds; swapping the texture and view
        // without it would leave every material sampling the old object,
        // which the old view keeps alive -- the footprint would grow and
        // nothing on screen would change.
        tex._texture = texture;
        tex._view = view;
        tex._sampler = sampler;
        tex.bind_group = bind_group;
        true
    }

    /// Brings one level in on the streamed texture that has been waiting
    /// longest -- round-robin over every streamed texture that is not yet
    /// fully resident -- and returns its id. `None` when every streamed
    /// texture is fully resident. What the progressive loader did before
    /// wants existed; [`step_streaming`](Self::step_streaming) with every
    /// want at `0` and no budget does the same.
    pub fn raise_next_pending(&mut self) -> Option<u64> {
        let mut pending: Vec<u64> = self
            .textures
            .iter()
            .filter(|(_, t)| t.streamed.as_ref().is_some_and(|s| s.resident_base > 0))
            .map(|(id, _)| *id)
            .collect();
        pending.sort_unstable();
        let next = *pending
            .iter()
            .find(|id| **id > self.last_raised)
            .or_else(|| pending.first())?;
        self.last_raised = next;
        self.raise_residency(next).then_some(next)
    }

    /// Records, for every streamed texture, how large it is drawn on screen
    /// this frame -- `pixels` is the largest on-screen extent of anything
    /// using it -- and so which level it wants resident (see
    /// [`wanted_base_for`]). A streamed texture absent from `wants` is
    /// wanted whole: nothing measured says it can be smaller.
    ///
    /// `bias` is the project's mip bias, in levels; positive wants smaller.
    pub fn set_wants(&mut self, wants: &HashMap<u64, f32>, bias: i32) {
        for (id, tex) in &mut self.textures {
            let Some(streamed) = tex.streamed.as_mut() else {
                continue;
            };
            streamed.wanted_base = match wants.get(id) {
                Some(pixels) => wanted_base_for(&streamed.levels, *pixels, bias),
                None => 0,
            };
        }
    }

    /// The chain index a streamed texture currently wants resident; `None`
    /// for a texture that is not streamed.
    pub fn wanted(&self, id: u64) -> Option<u32> {
        Some(self.textures.get(&id)?.streamed.as_ref()?.wanted_base)
    }

    /// Bytes the streamed textures hold on the GPU between them, as the
    /// profiler counts each one.
    pub fn streamed_resident_bytes(&self) -> u64 {
        self.textures
            .values()
            .filter(|t| t.streamed.is_some())
            .map(|t| t._texture.size_bytes())
            .sum()
    }

    /// How many streamed textures hold less than they want.
    pub fn textures_below_wanted(&self) -> u32 {
        self.textures
            .values()
            .filter(|t| {
                t.streamed
                    .as_ref()
                    .is_some_and(|s| s.resident_base > s.wanted_base)
            })
            .count() as u32
    }

    /// Moves residency one level towards what the wants and the budget
    /// say, on one texture, and reports what it did. What the streaming
    /// system calls once per frame; one level per frame for the reason the
    /// progressive loader gave -- each change is an allocation and a
    /// re-upload, and spreading them is what keeps a frame from stalling.
    ///
    /// In order of precedence, as Unreal's pool and Unity's budget behave:
    ///
    /// 1. **Over budget**: drop the largest resident level of the texture
    ///    holding the most it does not want (the one furthest from the
    ///    camera), or, when every texture is at or below its want, of the
    ///    one with the largest resident level. The budget wins over wants.
    /// 2. **A texture wants more**: bring the next level in on the one
    ///    furthest below its want, ties taken in turn -- if that level fits
    ///    the budget. If it does not, a texture holding a level it does
    ///    not want gives that up instead, so room appears next frame.
    /// 3. **A texture holds more than a level above its want**: drop its
    ///    largest level. One level of slack, so a texture whose want
    ///    flickers across a level boundary as the camera moves is not
    ///    rebuilt every frame.
    ///
    /// `budget` is the most bytes streamed textures may hold between them;
    /// `None` is no limit. Returns `None` when nothing needed doing.
    pub fn step_streaming(&mut self, budget: Option<u64>) -> Option<StreamingStep> {
        // (id, resident_base, wanted_base, last chain index, largest resident level bytes, next level bytes)
        let mut streamed: Vec<(u64, u32, u32, u32, u64, u64)> = self
            .textures
            .iter()
            .filter_map(|(id, t)| {
                let s = t.streamed.as_ref()?;
                let last = s.levels.len() as u32 - 1;
                let level_bytes = |i: u32| {
                    let (w, h, _) = &s.levels[i as usize];
                    *w as u64 * *h as u64 * 4
                };
                let next = s.resident_base.checked_sub(1).map_or(0, level_bytes);
                Some((
                    *id,
                    s.resident_base,
                    s.wanted_base,
                    last,
                    level_bytes(s.resident_base),
                    next,
                ))
            })
            .collect();
        streamed.sort_unstable_by_key(|s| s.0);
        let resident = self.streamed_resident_bytes();

        // The texture holding the most it does not want, largest level
        // first among equals; `None` when none can be lowered.
        let most_surplus = |min_surplus: u32| {
            streamed
                .iter()
                .filter(|(_, base, wanted, last, _, _)| {
                    base < last && wanted.saturating_sub(*base) >= min_surplus
                })
                .max_by_key(|(id, base, wanted, _, bytes, _)| {
                    (wanted.saturating_sub(*base), *bytes, std::cmp::Reverse(*id))
                })
                .map(|s| s.0)
        };

        if let Some(budget) = budget {
            if resident > budget {
                let victim = most_surplus(1).or_else(|| {
                    streamed
                        .iter()
                        .filter(|(_, base, _, last, _, _)| base < last)
                        .max_by_key(|(id, _, _, _, bytes, _)| (*bytes, std::cmp::Reverse(*id)))
                        .map(|s| s.0)
                })?;
                return self
                    .lower_residency(victim)
                    .then_some(StreamingStep::Lowered(victim));
            }
        }

        // The texture furthest below its want; ties go to the one after the
        // last raised, so textures brought up together each get a turn.
        let deficit = streamed
            .iter()
            .filter(|(_, base, wanted, _, _, _)| base > wanted)
            .map(|(id, base, wanted, _, _, _)| (base - wanted, *id))
            .max_by_key(|(gap, _)| *gap)
            .map(|(gap, _)| gap);
        if let Some(gap) = deficit {
            let mut candidates: Vec<&(u64, u32, u32, u32, u64, u64)> = streamed
                .iter()
                .filter(|(_, base, wanted, _, _, _)| base > wanted && base - wanted == gap)
                .collect();
            candidates.sort_unstable_by_key(|s| s.0);
            let next = candidates
                .iter()
                .find(|s| s.0 > self.last_raised)
                .or_else(|| candidates.first())
                .copied()
                .expect("a deficit means at least one candidate");
            let fits = budget.map_or(true, |b| resident + next.5 <= b);
            if fits {
                self.last_raised = next.0;
                return self
                    .raise_residency(next.0)
                    .then_some(StreamingStep::Raised(next.0));
            }
            if let Some(victim) = most_surplus(1) {
                return self
                    .lower_residency(victim)
                    .then_some(StreamingStep::Lowered(victim));
            }
            return None;
        }

        let victim = most_surplus(2)?;
        self.lower_residency(victim)
            .then_some(StreamingStep::Lowered(victim))
    }

    fn sampler_and_bind_group(
        &self,
        view: &wgpu::TextureView,
        settings: TextureImportSettings,
    ) -> (wgpu::Sampler, wgpu::BindGroup) {
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
                    resource: wgpu::BindingResource::TextureView(view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        (sampler, bind_group)
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

    /// The GPU object's own level-0 dimensions and footprint in bytes, as the
    /// profiler counts it. For an unstreamed texture that is the image's
    /// size; for a streamed one it is the largest *resident* level, which is
    /// what makes residency observable from outside without trusting the
    /// registry's own bookkeeping.
    pub fn get_gpu_footprint(&self, id: u64) -> Option<(u32, u32, u64)> {
        self.textures.get(&id).map(|t| {
            (
                t._texture.width(),
                t._texture.height(),
                t._texture.size_bytes(),
            )
        })
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

    fn streamed() -> TextureImportSettings {
        TextureImportSettings {
            srgb: false,
            mipmaps: true,
            streaming: true,
            ..Default::default()
        }
    }

    /// Bytes of an RGBA8 mip chain from a square power-of-two `top` level
    /// down to 1x1, as the profiler's geometric-series estimate counts it.
    fn chain_bytes(top: u32, levels: u32) -> u64 {
        let texels = (top * top) as f64 * (1.0 - 0.25f64.powi(levels as i32)) / 0.75;
        texels as u64 * 4
    }

    fn within_one_percent(actual: u64, expected: u64) -> bool {
        (actual as f64 - expected as f64).abs() <= expected as f64 * 0.01
    }

    /// The whole point of streaming, in bytes: a 256x256 streamed texture
    /// starts on the GPU as the 64x64 object holding levels 2..9 -- about
    /// 21 KiB where the full chain is 341 KiB -- and the profiler counts
    /// only that. Its logical size stays 256x256, because materials and UI
    /// lay out against the image, not against what is resident.
    #[test]
    fn a_streamed_texture_starts_with_only_its_small_levels_on_the_gpu() {
        let mut reg = make_registry();
        let pixels = vec![200u8; 256 * 256 * 4];

        let whole = reg.load_with(256, 256, &pixels, TextureImportSettings::default());
        let (_, _, whole_bytes) = reg.get_gpu_footprint(whole).unwrap();
        assert!(
            within_one_percent(whole_bytes, chain_bytes(256, 9)),
            "premise: the unstreamed chain is 9 levels of 256x256, {whole_bytes} bytes"
        );
        assert!(!reg.is_streaming(whole));
        assert_eq!(reg.residency(whole), None);

        let id = reg.load_with(256, 256, &pixels, streamed());
        assert!(reg.is_streaming(id));
        assert_eq!(
            reg.residency(id),
            Some((2, 9)),
            "levels 256, 128 wait; 64 is the first at or under {STREAMING_INITIAL_MAX_DIM}"
        );
        assert_eq!(reg.get_size(id), Some((256, 256)), "the logical size");
        let (w, h, bytes) = reg.get_gpu_footprint(id).unwrap();
        assert_eq!(
            (w, h),
            (64, 64),
            "the GPU object is the largest resident level"
        );
        assert_eq!(reg.get_gpu_shape(id).map(|s| s.1), Some(7), "levels 64..1");
        assert!(
            within_one_percent(bytes, chain_bytes(64, 7)),
            "and it costs only those levels: {bytes} bytes, not {whole_bytes}"
        );
        assert_eq!(reg.get_settings(id), Some(streamed()));
    }

    /// Each raise brings in exactly the next larger level -- the GPU object
    /// grows by that level's bytes and nothing else -- until level 0, after
    /// which raising reports nothing to do. Lowering is the reverse, and
    /// stops at the smallest level so the texture never becomes nothing.
    #[test]
    fn raising_and_lowering_residency_move_one_level_at_a_time() {
        let mut reg = make_registry();
        let id = reg.load_with(256, 256, &vec![9u8; 256 * 256 * 4], streamed());
        let bytes = |reg: &GpuTextureRegistry| reg.get_gpu_footprint(id).unwrap().2;
        let at_64 = bytes(&reg);

        assert!(reg.raise_residency(id));
        assert_eq!(reg.residency(id), Some((1, 9)));
        assert_eq!(
            reg.get_gpu_footprint(id).map(|f| (f.0, f.1)),
            Some((128, 128))
        );
        assert_eq!(reg.get_gpu_shape(id).map(|s| s.1), Some(8));
        let at_128 = bytes(&reg);
        assert!(
            within_one_percent(at_128 - at_64, 128 * 128 * 4),
            "one raise adds the 128x128 level and nothing else: {at_64} -> {at_128}"
        );

        assert!(reg.raise_residency(id));
        assert_eq!(reg.residency(id), Some((0, 9)), "fully resident");
        let at_256 = bytes(&reg);
        assert!(within_one_percent(at_256 - at_128, 256 * 256 * 4));
        assert!(
            !reg.raise_residency(id),
            "nothing above level 0 to bring in"
        );
        assert_eq!(reg.residency(id), Some((0, 9)));
        assert_eq!(bytes(&reg), at_256, "a refused raise must not rebuild");

        assert!(reg.lower_residency(id));
        assert_eq!(reg.residency(id), Some((1, 9)));
        assert_eq!(bytes(&reg), at_128, "lowering gives the level's bytes back");

        for _ in 0..7 {
            assert!(reg.lower_residency(id));
        }
        assert_eq!(reg.residency(id), Some((8, 9)), "down to the 1x1 level");
        assert!(!reg.lower_residency(id), "which always stays");
        assert_eq!(reg.get_gpu_footprint(id).map(|f| (f.0, f.1)), Some((1, 1)));

        // Neither call means anything for a texture that is not streamed.
        let plain = reg.load_with(8, 8, &[0u8; 8 * 8 * 4], TextureImportSettings::default());
        assert!(!reg.raise_residency(plain));
        assert!(!reg.lower_residency(plain));
        assert!(!reg.raise_residency(4242), "nor for an id nobody holds");
    }

    /// `streaming` without `mipmaps` has nothing to stream: one level is
    /// uploaded whole and the texture is not streamed, rather than being a
    /// streamed texture that can never change. And an image already at or
    /// under the initial size starts fully resident, so nothing is pending.
    #[test]
    fn streaming_needs_a_chain_and_a_small_image_is_resident_from_the_start() {
        let mut reg = make_registry();
        let unmipped = reg.load_with(
            256,
            256,
            &vec![0u8; 256 * 256 * 4],
            TextureImportSettings {
                mipmaps: false,
                ..streamed()
            },
        );
        assert!(!reg.is_streaming(unmipped));
        assert_eq!(reg.get_gpu_shape(unmipped).map(|s| s.1), Some(1));
        assert_eq!(reg.get_gpu_footprint(unmipped).map(|f| f.0), Some(256));

        let small = reg.load_with(64, 32, &[0u8; 64 * 32 * 4], streamed());
        assert!(reg.is_streaming(small), "streamed, with nothing waiting");
        assert_eq!(reg.residency(small), Some((0, 7)));
        assert!(!reg.raise_residency(small));
        assert_eq!(reg.raise_next_pending(), None);
    }

    /// The progressive loader's step: one level, on the streamed texture
    /// that has waited longest, so two textures brought up together each
    /// get a level in turn rather than one finishing before the other
    /// starts. Textures that are not streamed, or are already whole, are
    /// never picked, and the call says so once nothing is left.
    #[test]
    fn raise_next_pending_takes_turns_over_the_waiting_textures() {
        let mut reg = make_registry();
        let plain = reg.load_with(
            256,
            256,
            &vec![0u8; 256 * 256 * 4],
            TextureImportSettings::default(),
        );
        let a = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let b = reg.load_with(128, 128, &vec![0u8; 128 * 128 * 4], streamed());
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((2, 9)), Some((1, 8))),
            "premise: a needs two raises, b one"
        );

        let order: Vec<Option<u64>> = (0..4).map(|_| reg.raise_next_pending()).collect();
        assert_eq!(
            order,
            vec![Some(a), Some(b), Some(a), None],
            "a, then b, then a's last level, then nothing"
        );
        assert_eq!(reg.residency(a), Some((0, 9)));
        assert_eq!(reg.residency(b), Some((0, 8)));
        assert_eq!(
            reg.get_gpu_footprint(plain).map(|f| f.0),
            Some(256),
            "the unstreamed texture was never touched"
        );
    }

    /// The level a screen size asks for: the smallest level at least that
    /// many pixels across, so a texture drawn 100 px tall wants its 128
    /// level and not its 256; anything at or over level 0's size wants
    /// level 0; the bias moves the answer along the chain and stops at
    /// its ends.
    #[test]
    fn the_wanted_level_is_the_smallest_that_covers_the_screen_size() {
        let chain = mip_chain(256, 256, &[0u8; 256 * 256 * 4]);
        let levels: Vec<(u32, u32, Vec<u8>)> = chain
            .into_iter()
            .map(|(w, h, p)| (w, h, p.into_owned()))
            .collect();
        let want = |pixels: f32, bias: i32| wanted_base_for(&levels, pixels, bias);
        assert_eq!(want(100.0, 0), 1, "128 covers 100; 64 does not");
        assert_eq!(want(128.0, 0), 1, "exactly 128 is still the 128 level");
        assert_eq!(want(129.0, 0), 0);
        assert_eq!(want(300.0, 0), 0, "more than level 0 is still level 0");
        assert_eq!(want(f32::INFINITY, 0), 0);
        assert_eq!(want(10.0, 0), 4, "16 covers 10");
        assert_eq!(want(1.0, 0), 8, "the 1x1 level");
        assert_eq!(want(0.0, 0), 8);
        assert_eq!(want(100.0, 1), 2, "one level smaller than asked");
        assert_eq!(want(100.0, -1), 0);
        assert_eq!(want(300.0, -3), 0, "clamped at the top");
        assert_eq!(want(1.0, 5), 8, "and at the bottom");

        // A non-square chain is measured by its larger side.
        let chain = mip_chain(256, 64, &[0u8; 256 * 64 * 4]);
        let levels: Vec<(u32, u32, Vec<u8>)> = chain
            .into_iter()
            .map(|(w, h, p)| (w, h, p.into_owned()))
            .collect();
        assert_eq!(wanted_base_for(&levels, 100.0, 0), 1);
    }

    /// Wants steer the step: a texture drawn small settles at the level its
    /// screen size asks for and no higher, the texture furthest below its
    /// want is served first, a texture that comes to want less gives its
    /// levels back one per step, and one level of slack keeps a want that
    /// moves by one from rebuilding anything. A streamed texture nothing
    /// measured is wanted whole, as before.
    #[test]
    fn step_streaming_moves_each_texture_towards_its_want() {
        let mut reg = make_registry();
        let a = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let b = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let c = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        assert_eq!(reg.residency(a), Some((2, 9)), "premise: 64 resident");

        // a is drawn 300 px tall (wants everything), b 40 px (wants 64:
        // exactly what it holds), c is not drawn at all.
        let wants = HashMap::from([(a, 300.0), (b, 40.0)]);
        reg.set_wants(&wants, 0);
        assert_eq!(
            (reg.wanted(a), reg.wanted(b), reg.wanted(c)),
            (Some(0), Some(2), Some(0))
        );
        assert_eq!(reg.textures_below_wanted(), 2, "a and c");

        // a and c both want two more levels; they take turns, b is never
        // touched, and the step reports nothing once both are whole.
        let steps: Vec<Option<StreamingStep>> = (0..5).map(|_| reg.step_streaming(None)).collect();
        assert_eq!(
            steps,
            vec![
                Some(StreamingStep::Raised(a)),
                Some(StreamingStep::Raised(c)),
                Some(StreamingStep::Raised(a)),
                Some(StreamingStep::Raised(c)),
                None
            ]
        );
        assert_eq!(reg.residency(a), Some((0, 9)));
        assert_eq!(reg.residency(c), Some((0, 9)));
        assert_eq!(reg.residency(b), Some((2, 9)), "b holds what it wants");
        assert_eq!(reg.textures_below_wanted(), 0);

        // a moves away: drawn 10 px, it wants the 16 level (index 4). It
        // gives levels back one per step and stops one short (index 3) --
        // the slack that stops a want flickering across a boundary from
        // rebuilding every frame.
        reg.set_wants(&HashMap::from([(a, 10.0), (b, 40.0), (c, 300.0)]), 0);
        assert_eq!(reg.wanted(a), Some(4));
        let steps: Vec<Option<StreamingStep>> = (0..5).map(|_| reg.step_streaming(None)).collect();
        assert_eq!(
            steps,
            vec![
                Some(StreamingStep::Lowered(a)),
                Some(StreamingStep::Lowered(a)),
                Some(StreamingStep::Lowered(a)),
                None,
                None
            ]
        );
        assert_eq!(reg.residency(a), Some((3, 9)));

        // Wanting exactly one level less than held changes nothing.
        reg.set_wants(&HashMap::from([(a, 10.0), (b, 20.0), (c, 300.0)]), 0);
        assert_eq!(reg.wanted(b), Some(3), "b holds 2, wants 3");
        assert_eq!(reg.step_streaming(None), None);
        assert_eq!(reg.residency(b), Some((2, 9)));

        // A raise takes precedence over a lowering when both are due.
        reg.set_wants(&HashMap::from([(a, 300.0), (b, 5.0), (c, 300.0)]), 0);
        assert_eq!(reg.step_streaming(None), Some(StreamingStep::Raised(a)));
    }

    /// The budget wins over wants, as Unreal's pool does: a level that would
    /// exceed it is not brought in; over it, the texture holding the most it
    /// does not want gives a level back first, and when nothing has a
    /// surplus the largest resident level goes; a budget of `None` limits
    /// nothing. The bias is applied to every want.
    #[test]
    fn the_budget_caps_residency_and_evicts_the_least_wanted_first() {
        let mut reg = make_registry();
        let a = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let b = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        let at_64 = reg.streamed_resident_bytes();
        assert!(
            within_one_percent(at_64, 2 * chain_bytes(64, 7)),
            "premise: two 64-level chains, {at_64} bytes"
        );

        // Both want everything, but the budget has room for one 128 level
        // and not two: a is raised, b is refused, and nothing is lowered
        // because nothing holds more than it wants.
        reg.set_wants(&HashMap::from([(a, 300.0), (b, 300.0)]), 0);
        let budget = at_64 + 128 * 128 * 4 + 1024;
        assert_eq!(
            reg.step_streaming(Some(budget)),
            Some(StreamingStep::Raised(a))
        );
        assert_eq!(
            reg.step_streaming(Some(budget)),
            None,
            "b's 128 level does not fit"
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((1, 9)), Some((2, 9)))
        );
        assert_eq!(reg.textures_below_wanted(), 2, "both are held down");

        // Now a wants less (drawn 40 px: the 64 level) and b still wants
        // everything: a's surplus level goes, which makes room for b next.
        reg.set_wants(&HashMap::from([(a, 40.0), (b, 300.0)]), 0);
        assert_eq!(
            reg.step_streaming(Some(budget)),
            Some(StreamingStep::Lowered(a))
        );
        assert_eq!(
            reg.step_streaming(Some(budget)),
            Some(StreamingStep::Raised(b))
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((2, 9)), Some((1, 9)))
        );

        // The budget shrinks below what is held: b, holding a level it
        // wants, is still the one with the largest resident level once a
        // has no surplus, so b gives it back.
        let tiny = Some(at_64);
        assert_eq!(reg.step_streaming(tiny), Some(StreamingStep::Lowered(b)));
        assert_eq!(reg.residency(b), Some((2, 9)));
        assert!(reg.streamed_resident_bytes() <= at_64);
        // Still over? No: exactly at. Nothing more is dropped, and b is not
        // raised either since its level would not fit.
        assert_eq!(reg.step_streaming(tiny), None);

        // The bias: with +1 every want is one level smaller, so b, drawn
        // 300 px, wants the 128 level and stops there under no budget.
        reg.set_wants(&HashMap::from([(a, 40.0), (b, 300.0)]), 1);
        assert_eq!((reg.wanted(a), reg.wanted(b)), (Some(3), Some(1)));
        assert_eq!(reg.step_streaming(None), Some(StreamingStep::Raised(b)));
        assert_eq!(reg.step_streaming(None), None);
        assert_eq!(reg.residency(b), Some((1, 9)));

        // Eviction order: b whole and wanted whole, a holding one level it
        // does not want. Over budget, a's unwanted level goes before b's
        // larger one -- the least wanted first, not the largest.
        reg.set_wants(&HashMap::from([(a, 20.0), (b, 300.0)]), 0);
        assert_eq!(
            (reg.residency(a), reg.wanted(a)),
            (Some((2, 9)), Some(3)),
            "premise: a holds 64 and wants 32"
        );
        assert_eq!(reg.step_streaming(None), Some(StreamingStep::Raised(b)));
        assert_eq!(
            reg.step_streaming(None),
            None,
            "a's one level of surplus is within the slack"
        );
        let over = Some(reg.streamed_resident_bytes() - 1);
        assert_eq!(
            reg.step_streaming(over),
            Some(StreamingStep::Lowered(a)),
            "the least wanted level goes, not the largest"
        );
        assert_eq!(
            (reg.residency(a), reg.residency(b)),
            (Some((3, 9)), Some((0, 9)))
        );

        // A texture that is not streamed is never in any of this.
        let plain = reg.load_with(8, 8, &[0u8; 8 * 8 * 4], TextureImportSettings::default());
        assert_eq!(reg.wanted(plain), None);
        assert_eq!(
            reg.step_streaming(Some(1)),
            Some(StreamingStep::Lowered(b)),
            "over a budget of 1 byte, b (largest) goes"
        );
    }

    /// A reload -- new pixels, or a sidecar edit -- rebuilds a streamed
    /// texture at its initial residency, and turning `streaming` off on
    /// reload uploads it whole: what is resident follows the settings in
    /// force, not the history of raises.
    #[test]
    fn replacing_a_streamed_texture_restarts_it_and_can_stop_streaming_it() {
        let mut reg = make_registry();
        let id = reg.load_with(256, 256, &vec![0u8; 256 * 256 * 4], streamed());
        assert!(reg.raise_residency(id) && reg.raise_residency(id));
        assert_eq!(reg.residency(id), Some((0, 9)), "premise: fully resident");

        assert!(reg.replace(id, 256, 256, &vec![7u8; 256 * 256 * 4]));
        assert_eq!(
            reg.residency(id),
            Some((2, 9)),
            "back to the initial levels"
        );
        assert_eq!(reg.get_gpu_footprint(id).map(|f| f.0), Some(64));

        assert!(reg.replace_with(
            id,
            256,
            256,
            &vec![7u8; 256 * 256 * 4],
            TextureImportSettings::default()
        ));
        assert!(!reg.is_streaming(id));
        assert_eq!(reg.get_gpu_footprint(id).map(|f| f.0), Some(256));
    }
}
