//! Pipelines, and what they are made of, shared between renderers on one
//! device.
//!
//! # Why
//!
//! A renderer builds about seventy pipelines, and on D3D12 every one is a
//! shader compile. On the software adapter Windows CI runs every GPU test on,
//! that was 2 s a renderer even with DXC -- 70% of a pixel test's time, in a
//! suite where nearly every test makes its own renderer on the one device the
//! tests share. Pipelines are immutable, so a second renderer on the same
//! device has no reason to compile any of them again.
//!
//! # Which devices
//!
//! Only those registered with [`share_across_renderers`], which the tests'
//! shared device is. Everything this module makes for a registered device is
//! kept for the life of the process, which is right for a device that lives
//! that long anyway and wrong for one that does not: a runtime test app makes
//! a device per app and drops it, and caching for those would keep every
//! dead device's pipelines -- and through them the device's memory -- alive,
//! which on Windows CI's device budget ends in `OutOfMemory`. On any other
//! device each call simply creates, as `wgpu::Device` would.
//!
//! # What makes two requests the same
//!
//! A shader module: its label and WGSL source. A bind group layout: its label
//! and entries. A pipeline layout: its label, push constants and bind group
//! layouts; a pipeline: every field of its descriptor. The objects a
//! descriptor refers to (modules, layouts) are told apart by address, which is
//! only sound for objects this cache made and therefore keeps alive -- a
//! descriptor naming any other object is created, not cached, since an
//! address freed and reused would otherwise return another pipeline.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

/// One registered device's objects.
struct DeviceCache {
    device: Arc<wgpu::Device>,
    modules: HashMap<String, Arc<wgpu::ShaderModule>>,
    bind_group_layouts: HashMap<String, Arc<wgpu::BindGroupLayout>>,
    pipeline_layouts: HashMap<String, Arc<wgpu::PipelineLayout>>,
    render_pipelines: HashMap<String, Arc<wgpu::RenderPipeline>>,
    compute_pipelines: HashMap<String, Arc<wgpu::ComputePipeline>>,
    /// The addresses of every module and layout above: what a pipeline's
    /// descriptor may refer to and still be cached.
    owned: HashSet<usize>,
    stats: CacheStats,
}

/// What a registered device's cache has done, for tests.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct CacheStats {
    /// Requests answered with an object made earlier.
    pub hits: u64,
    /// Objects made and kept.
    pub misses: u64,
    /// Requests that could not be cached (a descriptor naming an object this
    /// cache did not make, or a source other than WGSL), and were created.
    pub uncached: u64,
}

static CACHES: Mutex<Vec<DeviceCache>> = Mutex::new(Vec::new());

/// Every WGSL module this crate asked a device for, label and source, in
/// this crate's test build -- what `gles_gate` translates to GLSL ES.
///
/// Recorded here, where every module is made ([`create_shader_module`] and
/// [`CachedCreate::cached_shader_module`]; this crate's `clippy.toml`
/// refuses `wgpu::Device::create_shader_module` anywhere else), so a new
/// shader is checked without anyone adding it to a list. A list was the
/// alternative, and a shader forgotten on it is a shader GLES never sees
/// checked: no CI runner has a GLES device, so nothing else would notice.
#[cfg(test)]
pub(crate) static RECORDED_WGSL: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

#[cfg(test)]
fn record(desc: &wgpu::ShaderModuleDescriptor) {
    if let wgpu::ShaderSource::Wgsl(source) = &desc.source {
        let mut recorded = RECORDED_WGSL.lock().unwrap_or_else(|e| e.into_inner());
        let label = desc.label.unwrap_or("").to_string();
        if !recorded.iter().any(|(l, s)| *l == label && s == source) {
            recorded.push((label, source.to_string()));
        }
    }
}

/// `device.create_shader_module(desc)`, for a module that is not to be shared
/// between renderers (a game's own shader, a one-off compile). The one place
/// besides the cache that makes a module -- see [`RECORDED_WGSL`] for why.
pub fn create_shader_module(
    device: &wgpu::Device,
    desc: wgpu::ShaderModuleDescriptor,
) -> wgpu::ShaderModule {
    #[cfg(test)]
    record(&desc);
    #[allow(clippy::disallowed_methods)] // the one sanctioned caller
    device.create_shader_module(desc)
}

/// The label of every render pipeline this crate made whose colour targets
/// do not all share one blend state and write mask, in this crate's test
/// build -- what `gles_gate` holds to a known list.
///
/// Such a pipeline needs `DownlevelFlags::INDEPENDENT_BLEND`, which GLES 3.1
/// does not have (it is core only from 3.2): there it is an invalid pipeline,
/// and on every backend CI has it is fine. wgpu compares the
/// `Option<BlendState>` itself, so `REPLACE` beside `None` counts as
/// different although both write the same pixels.
#[cfg(test)]
pub(crate) static MIXED_TARGET_PIPELINES: Mutex<Vec<String>> = Mutex::new(Vec::new());

#[cfg(test)]
fn record_targets(desc: &wgpu::RenderPipelineDescriptor) {
    let targets: Vec<_> = desc
        .fragment
        .as_ref()
        .map(|f| f.targets.iter().flatten().collect())
        .unwrap_or_default();
    // The same test wgpu-core applies before asking for the flag.
    let mixed = targets.split_first().is_some_and(|(first, rest)| {
        rest.iter()
            .any(|t| t.write_mask != first.write_mask || t.blend != first.blend)
    });
    if mixed {
        let label = desc.label.unwrap_or("").to_string();
        let mut recorded = MIXED_TARGET_PIPELINES
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if !recorded.contains(&label) {
            recorded.push(label);
        }
    }
}

/// `device.create_render_pipeline(desc)`, for a pipeline not to be shared
/// between renderers. With the cache, the one place a render pipeline is
/// made -- see [`MIXED_TARGET_PIPELINES`] for why that matters.
pub fn create_render_pipeline(
    device: &wgpu::Device,
    desc: &wgpu::RenderPipelineDescriptor,
) -> wgpu::RenderPipeline {
    #[cfg(test)]
    record_targets(desc);
    #[allow(clippy::disallowed_methods)] // the one sanctioned caller
    device.create_render_pipeline(desc)
}

/// `device.create_shader_module_trusted(desc, checks)`, recorded like
/// [`create_shader_module`].
///
/// # Safety
///
/// As `wgpu::Device::create_shader_module_trusted`: the checks `checks`
/// turns off must be ones the shader cannot need.
pub unsafe fn create_shader_module_trusted(
    device: &wgpu::Device,
    desc: wgpu::ShaderModuleDescriptor,
    checks: wgpu::ShaderRuntimeChecks,
) -> wgpu::ShaderModule {
    #[cfg(test)]
    record(&desc);
    // SAFETY: passed on to this function's caller.
    #[allow(clippy::disallowed_methods)] // the one sanctioned caller
    unsafe {
        device.create_shader_module_trusted(desc, checks)
    }
}

/// Shares `device`'s pipelines between every renderer made on it, from now
/// on, for the rest of the process. For a device that lives that long (see
/// the module docs); a second call is a no-op.
pub fn share_across_renderers(device: &Arc<wgpu::Device>) {
    let mut caches = CACHES.lock().unwrap_or_else(|e| e.into_inner());
    if caches.iter().any(|c| Arc::ptr_eq(&c.device, device)) {
        return;
    }
    caches.push(DeviceCache {
        device: Arc::clone(device),
        modules: HashMap::new(),
        bind_group_layouts: HashMap::new(),
        pipeline_layouts: HashMap::new(),
        render_pipelines: HashMap::new(),
        compute_pipelines: HashMap::new(),
        owned: HashSet::new(),
        stats: CacheStats::default(),
    });
}

/// `device`'s cache statistics; `None` for a device that is not registered.
pub fn stats(device: &wgpu::Device) -> Option<CacheStats> {
    let caches = CACHES.lock().unwrap_or_else(|e| e.into_inner());
    caches
        .iter()
        .find(|c| std::ptr::eq(Arc::as_ptr(&c.device), device))
        .map(|c| c.stats)
}

fn address<T>(object: &T) -> usize {
    object as *const T as usize
}

/// `constants` in a fixed order: a `HashMap`'s own order changes between runs.
fn options_key(options: &wgpu::PipelineCompilationOptions) -> String {
    let mut constants: Vec<_> = options.constants.iter().collect();
    constants.sort_by(|a, b| a.0.cmp(b.0));
    format!("{constants:?}|{}", options.zero_initialize_workgroup_memory)
}

/// Runs `f` with `device`'s cache, or returns `None` for an unregistered one.
fn with_cache<R>(device: &wgpu::Device, f: impl FnOnce(&mut DeviceCache) -> R) -> Option<R> {
    let mut caches = CACHES.lock().unwrap_or_else(|e| e.into_inner());
    caches
        .iter_mut()
        .find(|c| std::ptr::eq(Arc::as_ptr(&c.device), device))
        .map(f)
}

/// Looks `key` up in one of the cache's maps, making and keeping the object
/// on a miss. `own` records its address as one pipelines may refer to.
fn get_or_make<T>(
    cache: &mut DeviceCache,
    map: fn(&mut DeviceCache) -> &mut HashMap<String, Arc<T>>,
    key: String,
    own: bool,
    make: impl FnOnce(&wgpu::Device) -> T,
) -> Arc<T> {
    if let Some(found) = map(cache).get(&key) {
        let found = Arc::clone(found);
        cache.stats.hits += 1;
        return found;
    }
    let made = Arc::new(make(&cache.device));
    if own {
        cache.owned.insert(Arc::as_ptr(&made) as usize);
    }
    cache.stats.misses += 1;
    map(cache).insert(key, Arc::clone(&made));
    made
}

/// `wgpu::Device`'s constructors for what a renderer builds, through the
/// device's cache when it has one (see the module docs).
pub trait CachedCreate {
    /// `create_shader_module`, shared for WGSL sources.
    fn cached_shader_module(&self, desc: wgpu::ShaderModuleDescriptor) -> Arc<wgpu::ShaderModule>;
    /// `create_bind_group_layout`, shared.
    fn cached_bind_group_layout(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> Arc<wgpu::BindGroupLayout>;
    /// `create_pipeline_layout`, shared when every layout in it was.
    fn cached_pipeline_layout(
        &self,
        desc: &wgpu::PipelineLayoutDescriptor,
    ) -> Arc<wgpu::PipelineLayout>;
    /// `create_render_pipeline`, shared when its layout and modules were.
    fn cached_render_pipeline(
        &self,
        desc: &wgpu::RenderPipelineDescriptor,
    ) -> Arc<wgpu::RenderPipeline>;
    /// `create_compute_pipeline`, shared when its layout and module were.
    fn cached_compute_pipeline(
        &self,
        desc: &wgpu::ComputePipelineDescriptor,
    ) -> Arc<wgpu::ComputePipeline>;
}

impl CachedCreate for wgpu::Device {
    fn cached_shader_module(&self, desc: wgpu::ShaderModuleDescriptor) -> Arc<wgpu::ShaderModule> {
        let key = match &desc.source {
            wgpu::ShaderSource::Wgsl(source) => Some(format!("{:?}|{source}", desc.label)),
            #[allow(unreachable_patterns)]
            _ => None,
        };
        let Some(key) = key else {
            with_cache(self, |c| c.stats.uncached += 1);
            return Arc::new(create_shader_module(self, desc));
        };
        let mut desc = Some(desc);
        with_cache(self, |c| {
            get_or_make(
                c,
                |c| &mut c.modules,
                key,
                true,
                |d| create_shader_module(d, desc.take().expect("made once")),
            )
        })
        .unwrap_or_else(|| Arc::new(create_shader_module(self, desc.take().expect("made once"))))
    }

    fn cached_bind_group_layout(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> Arc<wgpu::BindGroupLayout> {
        let key = format!("{:?}|{:?}", desc.label, desc.entries);
        with_cache(self, |c| {
            get_or_make(
                c,
                |c| &mut c.bind_group_layouts,
                key,
                true,
                |d| d.create_bind_group_layout(desc),
            )
        })
        .unwrap_or_else(|| Arc::new(self.create_bind_group_layout(desc)))
    }

    fn cached_pipeline_layout(
        &self,
        desc: &wgpu::PipelineLayoutDescriptor,
    ) -> Arc<wgpu::PipelineLayout> {
        // A slot left empty (`None`) is part of the layout too.
        let layouts: Vec<Option<usize>> = desc
            .bind_group_layouts
            .iter()
            .map(|l| l.map(address))
            .collect();
        let key = format!("{:?}|{layouts:?}|{}", desc.label, desc.immediate_size);
        with_cache(self, |c| {
            if !layouts.iter().flatten().all(|l| c.owned.contains(l)) {
                c.stats.uncached += 1;
                return Arc::new(c.device.create_pipeline_layout(desc));
            }
            get_or_make(
                c,
                |c| &mut c.pipeline_layouts,
                key,
                true,
                |d| d.create_pipeline_layout(desc),
            )
        })
        .unwrap_or_else(|| Arc::new(self.create_pipeline_layout(desc)))
    }

    fn cached_render_pipeline(
        &self,
        desc: &wgpu::RenderPipelineDescriptor,
    ) -> Arc<wgpu::RenderPipeline> {
        let mut refers_to = vec![address(desc.vertex.module)];
        refers_to.extend(desc.layout.map(address));
        refers_to.extend(desc.fragment.as_ref().map(|f| address(f.module)));
        let fragment = desc.fragment.as_ref().map(|f| {
            format!(
                "{}|{:?}|{}|{:?}",
                address(f.module),
                f.entry_point,
                options_key(&f.compilation_options),
                f.targets
            )
        });
        let key = format!(
            "{:?}|{:?}|{}|{:?}|{}|{:?}|{:?}|{:?}|{:?}|{fragment:?}|{:?}",
            desc.label,
            desc.layout.map(address),
            address(desc.vertex.module),
            desc.vertex.entry_point,
            options_key(&desc.vertex.compilation_options),
            desc.vertex.buffers,
            desc.primitive,
            desc.depth_stencil,
            desc.multisample,
            desc.multiview_mask,
        );
        with_cache(self, |c| {
            if desc.cache.is_some() || !refers_to.iter().all(|r| c.owned.contains(r)) {
                c.stats.uncached += 1;
                return Arc::new(create_render_pipeline(&c.device, desc));
            }
            get_or_make(
                c,
                |c| &mut c.render_pipelines,
                key,
                false,
                |d| create_render_pipeline(d, desc),
            )
        })
        .unwrap_or_else(|| Arc::new(create_render_pipeline(self, desc)))
    }

    fn cached_compute_pipeline(
        &self,
        desc: &wgpu::ComputePipelineDescriptor,
    ) -> Arc<wgpu::ComputePipeline> {
        let mut refers_to = vec![address(desc.module)];
        refers_to.extend(desc.layout.map(address));
        let key = format!(
            "{:?}|{:?}|{}|{:?}|{}",
            desc.label,
            desc.layout.map(address),
            address(desc.module),
            desc.entry_point,
            options_key(&desc.compilation_options),
        );
        with_cache(self, |c| {
            if desc.cache.is_some() || !refers_to.iter().all(|r| c.owned.contains(r)) {
                c.stats.uncached += 1;
                return Arc::new(c.device.create_compute_pipeline(desc));
            }
            get_or_make(
                c,
                |c| &mut c.compute_pipelines,
                key,
                false,
                |d| d.create_compute_pipeline(desc),
            )
        })
        .unwrap_or_else(|| Arc::new(self.create_compute_pipeline(desc)))
    }
}
