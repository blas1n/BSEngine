//! What the adapter under a renderer can do that the renderer adapts to.
//!
//! Decided once, from the adapter, when the device is made: every pipeline
//! that has a fallback is built for one side of it. Each fallback is valid
//! WebGPU on every backend, so a test can turn a capability *off* on the
//! adapter CI has (`WgpuSurface::offscreen_for_testing_with_caps`) and
//! compare the fallback's pixels with the normal path's. That is how a path
//! only an OpenGL ES device takes is exercised at all: no CI runner has one.

/// See the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RenderCaps {
    /// `wgpu::Features::TIMESTAMP_QUERY`: the profiler's GPU pass times.
    pub timestamps: bool,
    /// Storage buffers in the vertex stage
    /// (`DownlevelFlags::VERTEX_STORAGE`): the shadow passes batch their
    /// draws into instanced ones.
    pub instancing: bool,
    /// 4x MSAA on every target the geometry passes draw into.
    pub msaa: bool,
    /// A texture viewed in another format than its own
    /// (`DownlevelFlags::VIEW_FORMATS`): the decal buffer's sRGB colour
    /// layer and the colour LUT's linear reading. OpenGL ES has no texture
    /// views of another format, so there the decal shader encodes sRGB
    /// itself and the LUT is copied into a linear texture.
    pub view_formats: bool,
    /// Realtime probe GI. Off on OpenGL ES: wgpu's GL backend has one
    /// context, which every call takes a lock on, and the GI pipelines --
    /// built on a thread so a frame never waits for them -- held it past the
    /// lock's timeout (one second under WGL, six under EGL), and the frame
    /// thread panicked ("Could not lock adapter context"). Unreal has no
    /// Lumen on its mobile renderer, nor Godot SDFGI on its mobile ones.
    pub realtime_gi: bool,
    /// Cube-map arrays (`DownlevelFlags::CUBE_ARRAY_TEXTURES`): every
    /// reflection probe's capture bound as one `texture_cube_array`. OpenGL
    /// ES has them only from 3.2 (or with `GL_EXT_texture_cube_map_array`),
    /// so on 3.1 the same 24 faces are bound as a 2D array and the mesh
    /// shader picks the face and its texel itself (`REFLECTION_FACES_WGSL`).
    pub cube_arrays: bool,
}

impl RenderCaps {
    /// What `adapter` can do. `msaa_formats` are the formats every
    /// multisampled target is made in, which all have to support it.
    pub(crate) fn of(adapter: &wgpu::Adapter, msaa_formats: &[wgpu::TextureFormat]) -> Self {
        let flags = adapter.get_downlevel_capabilities().flags;
        Self {
            timestamps: adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY),
            // Storage buffers in the vertex stage are an adapter capability,
            // not something `required_limits` can ask for, and a pipeline
            // whose vertex shader reads storage fails at *creation* where it
            // is missing -- so this has to be decided before the shadow
            // pipelines are built, not at draw time.
            instancing: flags.contains(wgpu::DownlevelFlags::VERTEX_STORAGE),
            msaa: msaa_formats.iter().all(|f| {
                adapter
                    .get_texture_format_features(*f)
                    .flags
                    .sample_count_supported(crate::msaa::MSAA_SAMPLES)
            }),
            view_formats: flags.contains(wgpu::DownlevelFlags::VIEW_FORMATS),
            realtime_gi: adapter.get_info().backend != wgpu::Backend::Gl,
            cube_arrays: flags.contains(wgpu::DownlevelFlags::CUBE_ARRAY_TEXTURES),
        }
    }
}

/// Whether wgpu's OpenGL backend makes a texture of this shape a cube-map
/// array, whatever it is later viewed as: wgpu-hal's
/// `gles::Texture::get_info_from_desc` takes any square, single-sample 2D
/// texture with a layer count that is a multiple of six for a cube map (six
/// layers) or a cube-map array (more), "a HACK" by its own comment.
///
/// OpenGL ES 3.1 has no cube-map arrays, so there such a texture cannot be
/// made, and a 2D-array view of one does not bind on any GL ("wgpu-hal
/// heuristics assumed that the view dimension will be equal to `CubeArray`
/// rather than `D2Array`", the emulator's log). Six layers are a plain cube
/// map, which every GLES has.
#[cfg(test)] // only the recording in `profiler` asks, in tests
pub(crate) fn gl_makes_cube_array(desc: &wgpu::TextureDescriptor) -> bool {
    desc.dimension == wgpu::TextureDimension::D2
        && desc.sample_count == 1
        && desc.size.width == desc.size.height
        && desc.size.depth_or_array_layers > 6
        && desc.size.depth_or_array_layers.is_multiple_of(6)
}

/// The layer count to make a 2D array of `layers` six-face groups with --
/// point shadows, probe captures -- so that wgpu's OpenGL backend makes a
/// plain `GL_TEXTURE_2D_ARRAY` of it ([`gl_makes_cube_array`]): one spare
/// layer when `layers` is a multiple of six, which nothing reads. Spent on
/// every backend: one layer is cheaper than a second code path, and a
/// texture's shape that differed between backends would be one CI could not
/// see.
pub(crate) fn plain_array_layers(layers: u32) -> u32 {
    if layers.is_multiple_of(6) {
        layers + 1
    } else {
        layers
    }
}
