use crate::pipeline_cache::CachedCreate;
/// Texture format used for the HDR scene-color render target, before tonemapping.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Format of the screen-space normal and roughness buffer.
///
/// Float rather than 8-bit unorm: a reflection ray is traced *along* this
/// normal, so a direction quantised to 1/255 sends the ray visibly off course
/// over the dozens of steps a march takes. Roughness rides in the alpha
/// channel, which is why this is four components and not two.
pub const NORMAL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Format of the velocity buffer: how far each pixel's surface moved on
/// screen since last frame, in uv units, written by the opaque pass.
///
/// Half floats, as Unity's and Unreal's velocity buffers are: a velocity is
/// a small signed fraction of the screen, which 8-bit unorm would round to
/// whole pixels at best. Two channels, because a screen motion is two
/// numbers.
pub const VELOCITY_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// Format of the GI indirect buffer: in rgb the indirect diffuse light the
/// opaque pass applied -- from the sky, the light probes or the lightmap --
/// so screen-space GI can take it back out where it has better; in alpha the
/// red of the reflectance it was multiplied by (the albedo, times the
/// diffuse share `kd` and the occlusion). HDR light, so half floats.
///
/// ⚠️ Six numbers in two targets, 8 and 4 bytes: the opaque pass then writes
/// 32 bytes a sample -- the HDR colour (8), the normal (8), the velocity (4)
/// and these -- exactly WebGPU's default `maxColorAttachmentBytesPerSample`.
/// A separate 8-bit albedo does not fit: WebGPU counts `rgba8unorm` as 8
/// bytes a sample, not 4 (its blending cost), which made 36 and failed every
/// pipeline. Another target does not fit either; pack first.
pub const GI_INDIRECT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Format of the GI albedo buffer: the green and blue of the reflectance
/// (its red rides in [`GI_INDIRECT_FORMAT`]'s alpha). Half floats, which
/// resolve under MSAA as the others do.
pub const GI_ALBEDO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// The velocity buffer's clear value: "nothing here wrote a velocity" --
/// the sky, terrain, custom shaders, the far side of a translucent pane.
/// The post passes reproject such a pixel by depth and the camera, as they
/// did every pixel before velocities existed. The largest half float, so no
/// real screen motion (a fraction of a screen) can be mistaken for it; the
/// shaders test against 1000.
pub const VELOCITY_NONE: wgpu::Color = wgpu::Color {
    r: 65504.0,
    g: 65504.0,
    b: 0.0,
    a: 0.0,
};
const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const CONFIG_SIZE: u64 = 96;
const SSAO_CAM_SIZE: u64 = 128;
const TAA_CAM_SIZE: u64 = 128;

/// The volumetric-fog apply pass.
///
/// It is the first pass of the post-process chain: the scene renders into
/// `hdr_texture`, this reads that plus the integrated froxel volume and writes
/// `fog_hdr_texture`, and every downstream pass reads that result. Two HDR
/// targets rather than one because a pass may not sample the texture it
/// renders into.
///
/// Composed by [`fog_apply_wgsl`], which prepends the grid constants and the
/// shared froxel declarations -- `fog` at group 3, and `depth_to_froxel_w`,
/// the inverse of the depth slicing the injection pass uses.
///
/// With `fog.enabled == 0u` this returns the scene sample untouched, so the
/// chain produces exactly the image it did before the froxel lookup existed.
/// Every pixel test relies on that: none of them sets `VolumetricFog`.
const FOG_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(0) var integrated_vol: texture_3d<f32>;
@group(2) @binding(1) var vol_sampler: sampler;
// `fog` sits at @group(3) @binding(0); see `fog_uniform_wgsl`.

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

fn load_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(depth_tex, coord, 0);
}

// The camera-to-far-plane vector through this pixel: the exact ray the
// injection pass builds this screen column of froxels along.
fn view_ray(uv: vec2<f32>) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), 1.0, 1.0);
    let far_h = fog.inv_view_proj * ndc;
    return far_h.xyz / far_h.w - fog.camera_pos;
}

// View depth of whatever was drawn at this pixel, in the same units
// `froxel_slice_depth` speaks.
//
// `ray` spans exactly `fog.far` of view depth and the reconstructed point lies
// on it, so the fraction along it scales straight to a view depth -- the exact
// inverse of the injection pass's `world_pos = camera_pos + ray * (depth/far)`.
// Deriving it from the same `inv_view_proj` is what keeps the two in step; a
// separately supplied near/far linearisation would not.
fn surface_view_depth(uv: vec2<f32>, depth: f32, ray: vec3<f32>) -> f32 {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), depth, 1.0);
    let world_h = fog.inv_view_proj * ndc;
    let world = world_h.xyz / world_h.w;
    return fog.far * dot(world - fog.camera_pos, ray) / max(dot(ray, ray), 1e-6);
}

@fragment
fn fs_fog(in: FullscreenOut) -> @location(0) vec4<f32> {
    let scene = textureSample(hdr_tex, tex_sampler, in.uv);
    // The whole pass is a passthrough with fog off, and this is the branch
    // that makes it one. It is also the only reason the pre-existing pixel
    // tests still produce their reference images.
    if fog.enabled == 0u {
        return scene;
    }
    let depth = load_depth(in.uv);
    let ray = view_ray(in.uv);
    // Background pixels (nothing drawn) take the volume's last slice, so the
    // sky is fogged too. Skipping them -- the tempting shortcut, since the
    // SSAO pass in this same file does skip them -- would leave a crisp
    // horizon standing behind thick fog.
    let view_z = select(surface_view_depth(in.uv, depth, ray), fog.far, depth >= 1.0);
    let vol = textureSampleLevel(
        integrated_vol, vol_sampler,
        vec3<f32>(in.uv, depth_to_froxel_w(view_z)), 0.0,
    );
    // RGB is the light in-scattered between the camera and this pixel; A is
    // the transmittance across that same stretch.
    return vec4<f32>(scene.rgb * vol.a + vol.rgb, scene.a);
}
"#;

/// Storage/sample format of the two froxel volumes.
///
/// `Rgba16Float` carries `all_flags` in wgpu's format capability table, so it
/// supports `STORAGE_BINDING` with no extra device feature, and it is
/// filterable, which the Task 5 apply pass needs for its trilinear lookup.
const FROXEL_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// Size of [`FogUniform`], in bytes. See the test of the same name.
const FOG_UNIFORM_SIZE: u64 = 576;

/// Compute workgroup edge, matching the `@workgroup_size(8, 8, 1)` in both
/// froxel shaders. The dispatch counts are derived from it.
const FROXEL_WORKGROUP: u32 = 8;

/// The froxel grid dimensions, emitted as WGSL constants so the two compute
/// shaders cannot drift from [`crate::froxel`]'s Rust ones.
///
/// A disagreement would not fail to compile: it would leave part of the grid
/// unwritten, or place fog at the wrong depth, which reads as a density bug.
fn froxel_wgsl_preamble() -> String {
    format!(
        "const FROXEL_X: u32 = {}u;\nconst FROXEL_Y: u32 = {}u;\nconst FROXEL_Z: u32 = {}u;\n",
        crate::froxel::FROXEL_X,
        crate::froxel::FROXEL_Y,
        crate::froxel::FROXEL_Z,
    )
}

/// The `FogUniform` declaration, matching the Rust [`FogUniform`].
///
/// Split from [`FROXEL_COMMON_WGSL`] only so the binding line can name a
/// different group per shader; see [`fog_uniform_wgsl`].
const FOG_UNIFORM_STRUCT_WGSL: &str = r#"
struct FogUniform {
    inv_view_proj: mat4x4<f32>,
    cascade_view_proj: array<mat4x4<f32>, 4>,
    prev_view_proj: mat4x4<f32>,
    prev_inv_view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    near: f32,
    light_dir: vec3<f32>,
    far: f32,
    light_color: vec3<f32>,
    density: f32,
    fog_color: vec3<f32>,
    anisotropy: f32,
    prev_camera_pos: vec3<f32>,
    cascade_blend: f32,
    enabled: u32,
    frame_index: u32,
    history_valid: u32,
    cascade_count: u32,
    cascade_splits: vec4<f32>,
    cam_forward: vec3<f32>,
    pad0: f32,
}
"#;

/// The `FogUniform` struct plus its binding at `group`.
///
/// The two compute passes take it at group 0, where it is their first group.
/// The apply pass takes it at group 3, its first three being the scene colour,
/// the depth buffer, and the integrated volume.
fn fog_uniform_wgsl(group: u32) -> String {
    format!("{FOG_UNIFORM_STRUCT_WGSL}@group({group}) @binding(0) var<uniform> fog: FogUniform;\n")
}

/// Declarations every froxel shader shares: the depth slicing and its inverse.
///
/// `froxel_slice_depth` here is a direct port of [`crate::froxel::froxel_slice_depth`]
/// -- exponential, not linear -- and `depth_to_froxel_w` inverts it. The
/// mapping exists in the Rust, in this forward function, and in that inverse;
/// all three have to agree or the fog sits at the wrong distance, which reads
/// as a density problem rather than a mapping one. `depth_to_froxel_w` lives
/// here, beside the mapping it inverts, rather than in the apply shader that
/// is its only caller, so the pair cannot drift apart unnoticed.
const FROXEL_COMMON_WGSL: &str = r#"
const PI: f32 = 3.14159265358979;

// Port of `froxel::froxel_slice_depth`: the world-space view depth at the far
// edge of slice `slice`.
fn froxel_slice_depth(slice: u32) -> f32 {
    let t = f32(slice + 1u) / f32(FROXEL_Z);
    return fog.near * pow(fog.far / fog.near, t);
}

// Inverse of `froxel_slice_depth`, as a 0..1 coordinate down the volume's W
// axis. Solving `view_z = near * pow(far/near, t)` for `t` gives
// `log(view_z/near) / log(far/near)`.
//
// Clamped at both ends: nothing nearer than the near plane or beyond the far
// plane has a slice, and the volume's edge texels are what those should read.
fn depth_to_froxel_w(view_z: f32) -> f32 {
    let z = clamp(view_z, fog.near, fog.far);
    return clamp(log(z / fog.near) / log(fog.far / fog.near), 0.0, 1.0);
}

// Port of `froxel::froxel_jitter`: a pseudo-random offset in [0, 1) for one
// froxel on one frame. See the Rust for why it is a hash of the coordinate and
// the frame rather than a random number -- headless replays have to reproduce.
//
// It sits in the shared block rather than beside its only caller in the
// injection shader so `the_wgsl_froxel_jitter_matches_the_rust_one` can reach it
// through `run_froxel_probe`, which composes exactly this text and nothing from
// the injection pass.
fn froxel_jitter(coord: vec3<u32>, frame: u32) -> f32 {
    let h = (coord.x * 73856093u) ^ (coord.y * 19349663u)
          ^ (coord.z * 83492791u) ^ (frame * 2654435761u);
    return f32(h & 0xFFFFu) / 65536.0;
}

// The view depth this froxel samples its lighting at: somewhere strictly inside
// slice `coord.z`, picked by `froxel_jitter`.
//
// Slice 0 starts at the near plane and every later one starts where its
// predecessor ended, which is exactly the span the integration pass integrates
// across -- so interpolating between those two edges keeps the sample inside the
// slice it belongs to. Reaching past an edge would light a froxel from a
// neighbour's depth, which is the one thing worse than the banding this exists
// to remove.
//
// Shared rather than local to the injection pass so
// `the_jittered_sample_stays_inside_its_own_slice` can probe the real function.
fn froxel_sample_depth(coord: vec3<u32>, frame: u32) -> f32 {
    let slice_far = froxel_slice_depth(coord.z);
    let slice_near = select(froxel_slice_depth(coord.z - 1u), fog.near, coord.z == 0u);
    return mix(slice_near, slice_far, froxel_jitter(coord, frame));
}

// The world-space point at view depth `depth` down the screen column of froxel
// column `coord_xy`.
//
// The far-plane point behind that column, scaled back along the ray to `depth`.
// The camera-to-far-plane vector has a view-space z of exactly `far`, so scaling
// it by `depth / far` lands on the right depth without the view matrix having to
// be supplied separately -- the same relation the apply pass's
// `surface_view_depth` runs backwards.
fn froxel_ray_point(coord_xy: vec2<u32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(f32(coord_xy.x), f32(coord_xy.y)) + vec2<f32>(0.5, 0.5))
        / vec2<f32>(f32(FROXEL_X), f32(FROXEL_Y));
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), 1.0, 1.0);
    let far_h = fog.inv_view_proj * ndc;
    let far_world = far_h.xyz / far_h.w;
    return fog.camera_pos + (far_world - fog.camera_pos) * (depth / fog.far);
}

// The world-space point froxel `coord` samples its lighting at on frame `frame`:
// its ray at the dithered depth.
fn froxel_world_pos(coord: vec3<u32>, frame: u32) -> vec3<f32> {
    return froxel_ray_point(coord.xy, froxel_sample_depth(coord, frame));
}

// The centre of froxel `coord`, in world space and deliberately undithered.
//
// This is the point the temporal blend reprojects, and it is NOT
// `froxel_world_pos`. The volume stores one accumulated value per froxel, and
// the point that value describes is the froxel's centre; asking for the history
// at the dithered sample position instead would move the lookup by a jitter's
// worth every frame, and a trilinear read at a wobbling coordinate re-injects
// the very variance the accumulation exists to remove. Measured, not guessed:
// with the dithered position here, two settled frames differed in half as many
// pixels as two raw ones instead of a small fraction of them.
//
// The centre is taken in the volume's own W coordinate rather than halfway
// between the slice's two depths, because the slicing is exponential: the
// arithmetic midpoint of a slice sits nearer its front edge than the texel
// centre does, and a lookup there would straddle two texels forever.
fn froxel_centre_world_pos(coord: vec3<u32>) -> vec3<f32> {
    let t = (f32(coord.z) + 0.5) / f32(FROXEL_Z);
    return froxel_ray_point(coord.xy, fog.near * pow(fog.far / fog.near, t));
}

// Where a world-space point sat in the PREVIOUS frame's froxel grid: `.xyz` is
// the volume coordinate in 0..1 and `.w` is 1.0 when that point was inside the
// previous volume at all, 0.0 when it was not.
//
// The mapping is the forward one `cs_inject` runs, inverted and rebuilt from
// last frame's matrices: the same far-plane ray construction, the same
// `depth_to_froxel_w`. Deriving the depth from `prev_inv_view_proj` rather than
// from a separately supplied linearisation is what keeps the two in step, for
// the same reason the apply pass builds `surface_view_depth` out of
// `inv_view_proj`.
//
// Out-of-volume is a hard rejection, never a clamp. A froxel whose world
// position was off-screen, behind the eye, or outside the near/far range last
// frame has no history at all; clamping would hand it the edge froxel's light
// instead and smear that light along the frustum border. It is the same rule
// the TAA resolve applies to off-screen history, and for the same reason.
fn froxel_history_uvw(world_pos: vec3<f32>) -> vec4<f32> {
    let clip = fog.prev_view_proj * vec4<f32>(world_pos, 1.0);
    if clip.w <= 0.0 {
        return vec4<f32>(0.0);
    }
    let ndc = clip.xyz / clip.w;
    let uv = vec2<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5);
    if uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0 {
        return vec4<f32>(0.0);
    }
    // The previous frame's far-plane point behind this column, and the view
    // depth of `world_pos` along the ray to it -- the same construction
    // `cs_inject` runs forwards to place a froxel in the world.
    let far_h = fog.prev_inv_view_proj * vec4<f32>(ndc.x, ndc.y, 1.0, 1.0);
    let ray = far_h.xyz / far_h.w - fog.prev_camera_pos;
    let view_z = fog.far * dot(world_pos - fog.prev_camera_pos, ray)
        / max(dot(ray, ray), 1e-6);
    if view_z < fog.near || view_z > fog.far {
        return vec4<f32>(0.0);
    }
    return vec4<f32>(uv, depth_to_froxel_w(view_z), 1.0);
}
"#;

/// The shadow resources the injection pass reads, at group 2.
///
/// Every one of them is the *same GPU object* the scene shader's group 2 binds:
/// the same directional depth map, the same comparison sampler, the same point
/// shadow array, and -- crucially -- the same light uniform buffer, not a copy
/// of it. `LightUniform` is 960 bytes of point/spot arrays; duplicating it into
/// [`FogUniform`] would mean a second per-frame upload and two structs that have
/// to be kept in step by hand. The terrain shader already shares this buffer the
/// same way, and its comment says what the rule is: both shaders read one
/// buffer, so both must declare the struct identically. That includes this one.
///
/// `light_view_proj` is the exception and lives in [`FogUniform`]: on the scene
/// side it is a field of the *camera* uniform, not of `LightUniform`.
const FROXEL_SHADOW_WGSL: &str = r#"
struct PointLightEntry {
    position: vec3<f32>,
    // Atlas layer of the light's cookie, or -1.0 for none (`point_cookie`).
    cookie_layer: f32,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    // The light's rotation as a quaternion's x, y, z; w is the positive root.
    cookie_qx: f32,
    cookie_qy: f32,
    cookie_qz: f32,
}
struct SpotLightEntry {
    position: vec3<f32>,
    // Roll about `direction` from `cookie_basis` (see `cookie.rs`).
    cookie_roll: f32,
    direction: vec3<f32>,
    inner_cos: f32,
    color: vec3<f32>,
    outer_cos: f32,
    intensity: f32,
    range: f32,
    // Atlas layer of the light's cookie, or -1.0 for none.
    cookie_layer: f32,
    shadow_layer: f32,
}
// Must match `LightUniformData` in surface.rs field for field, padding
// included: this binds that struct's buffer.
struct LightUniform {
    direction: vec3<f32>,
    _pad0: f32,
    color: vec3<f32>,
    _pad1: f32,
    ambient: vec3<f32>,
    num_point_lights: u32,
    point_lights: array<PointLightEntry, 8>,
    num_spot_lights: u32,
    ibl_enabled: u32,
    ibl_max_mip: f32,
    _pad4: f32,
    spot_lights: array<SpotLightEntry, 8>,
}
@group(2) @binding(0) var shadow_map: texture_depth_2d_array;
@group(2) @binding(1) var shadow_sampler: sampler_comparison;
@group(2) @binding(2) var point_shadow_map: texture_2d_array<f32>;
@group(2) @binding(3) var<uniform> lights: LightUniform;
"#;

/// Injection: one thread per froxel, writing in-scattered light in RGB and
/// extinction in A.
///
/// Each froxel is tested against the shadow maps before it scatters anything,
/// which is what makes this light shafts rather than uniform fog. Two separate
/// lookups do that, because the engine stores the two kinds of shadow in
/// completely different ways: see `directional_shadow_factor` for the one way
/// the sun's lookup has to differ from the scene shader's, and
/// `point_shadow_factor` for the cube-array one, which is a verbatim port.
const FOG_INJECT_WGSL: &str = r#"
@group(1) @binding(0) var injection_vol: texture_storage_3d<rgba16float, write>;
// The other half of the scattering ping-pong: what this pass wrote last frame,
// bound for sampling. A pass cannot read the volume it is writing, so the two
// swap roles every frame.
@group(3) @binding(0) var history_vol: texture_3d<f32>;
@group(3) @binding(1) var history_sampler: sampler;

// How much of the accumulated history survives into each new frame.
//
// High on purpose: the dither above deliberately makes a single frame noisy, and
// only the average over many frames is the answer it is approximating. At 0.9 a
// froxel's value is spread over roughly ten frames, which is enough to resolve
// the noise into a gradient while still following a moving light or a moving
// occluder within a few frames of it moving.
const FOG_HISTORY_WEIGHT: f32 = 0.9;

// Port of `froxel::henyey_greenstein`, 1/(4*PI) included. Dropping that factor
// -- easy, since many references quote the unnormalised form -- makes the fog
// roughly 12x too bright and merely reads as a mistuned density.
fn henyey_greenstein(cos_theta: f32, g_in: f32) -> f32 {
    let g = clamp(g_in, -0.99, 0.99);
    let g2 = g * g;
    let denom = 1.0 + g2 - 2.0 * g * cos_theta;
    return (1.0 - g2) / (4.0 * PI * pow(max(denom, 1e-4), 1.5));
}

// Port of the scene shader's `shadow_factor` (surface.rs), with one mandatory
// change: `textureSampleCompare` needs implicit derivatives and so exists only
// in fragment stages, which makes it unusable here. `textureSampleCompareLevel`
// is its compute-safe form -- it samples level 0 explicitly instead of deriving
// one -- and it is also what makes this legal after the early return below,
// since that return is non-uniform control flow.
//
// Everything else matches the scene shader exactly: the w-divide, the
// (0.5, -0.5) UV flip, the out-of-bounds return of 1.0 ("outside the map is
// lit", the same answer the surface path gives), and the 0.003 depth bias. That
// is the point -- a froxel and a surface at one world position have to agree
// about whether they are in shadow, or the beams will not line up with the
// shadows the geometry casts.
fn fog_cascade_sample(world_pos: vec3<f32>, vp: mat4x4<f32>, layer: i32) -> f32 {
    let lsp = vp * vec4<f32>(world_pos, 1.0);
    let proj = lsp.xyz / lsp.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let depth = proj.z;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0
        || depth < 0.0 || depth > 1.0) {
        return 1.0;
    }
    return textureSampleCompareLevel(shadow_map, shadow_sampler, uv, layer, depth - 0.003);
}

// Cascade selection, and it has to be the *same* selection the scene shader
// makes. The alignment this file has always been careful about now depends on
// two things agreeing rather than one: a froxel that picked a different
// cascade than the surface behind it would sample a map of a different texel
// density, and the beams would part company with the shadows at exactly the
// boundaries where cascades change.
fn fog_cascade_for(view_depth: f32) -> i32 {
    var layer = 0;
    if (view_depth >= fog.cascade_splits.x) { layer = 1; }
    if (view_depth >= fog.cascade_splits.y) { layer = 2; }
    if (view_depth >= fog.cascade_splits.z) { layer = 3; }
    return min(layer, i32(fog.cascade_count) - 1);
}
fn fog_cascade_far(layer: i32) -> f32 {
    if (layer == 0) { return fog.cascade_splits.x; }
    if (layer == 1) { return fog.cascade_splits.y; }
    if (layer == 2) { return fog.cascade_splits.z; }
    return fog.cascade_splits.w;
}
fn directional_shadow_factor(world_pos: vec3<f32>) -> f32 {
    let view_depth = dot(world_pos - fog.camera_pos, fog.cam_forward);
    let layer = fog_cascade_for(view_depth);
    var lit = fog_cascade_sample(world_pos, fog.cascade_view_proj[layer], layer);
    let last = i32(fog.cascade_count) - 1;
    if (fog.cascade_blend > 0.0 && layer < last) {
        let far = fog_cascade_far(layer);
        let band = far * fog.cascade_blend;
        if (band > 0.0 && view_depth > far - band) {
            let t = clamp((view_depth - (far - band)) / band, 0.0, 1.0);
            let next = fog_cascade_sample(world_pos, fog.cascade_view_proj[layer + 1], layer + 1);
            lit = mix(lit, next, t);
        }
    }
    return lit;
}

// Verbatim port of the scene shader's `point_shadow_factor` (surface.rs) --
// body, comment and all. Nothing about it needs changing for compute: it reads
// a raw texel with `textureLoad`, which has no implicit-derivative requirement,
// so unlike the directional lookup above there is no substitute sampler here.
//
// The face selection below is copied, NOT re-derived. Its own comment records
// that it matches `point_light_face_view_projs`'s Rust-side construction and
// was verified by hand against glam's `look_at_rh` convention; a re-derivation
// that picked a neighbouring face would still return plausible-looking shadows
// and would be very hard to catch afterwards. Copying it also keeps the froxel
// and the surface at one world position reading the same texel, which is the
// same reason `directional_shadow_factor` keeps the scene shader's bias.
//
// Linear-distance cube shadow lookup: `to_frag` is the direction from the
// light to the fragment (world-space, unnormalized). Selects the cube face
// whose axis has the largest magnitude, derives that face's UV analytically
// (matching point_light_face_view_projs's Rust-side look_at_rh/perspective
// construction exactly -- verified by hand against glam's look_at_rh
// convention), then compares against the stored linear distance for that
// face/light layer instead of using a depth-compare sampler (R32Float isn't
// natively filterable without an unrequested device feature, so this reads
// a raw texel via textureLoad rather than textureSample).
fn point_shadow_factor(light_index: u32, to_frag: vec3<f32>) -> f32 {
    let ax = abs(to_frag.x);
    let ay = abs(to_frag.y);
    let az = abs(to_frag.z);
    var face: u32;
    var u: f32;
    var v: f32;
    var ma: f32;
    if (ax >= ay && ax >= az) {
        ma = ax;
        if (to_frag.x > 0.0) {
            face = 0u;
            u = -to_frag.z;
            v = -to_frag.y;
        } else {
            face = 1u;
            u = to_frag.z;
            v = -to_frag.y;
        }
    } else if (ay >= ax && ay >= az) {
        ma = ay;
        if (to_frag.y > 0.0) {
            face = 2u;
            u = to_frag.x;
            v = to_frag.z;
        } else {
            face = 3u;
            u = to_frag.x;
            v = -to_frag.z;
        }
    } else {
        ma = az;
        if (to_frag.z > 0.0) {
            face = 4u;
            u = to_frag.x;
            v = -to_frag.y;
        } else {
            face = 5u;
            u = -to_frag.x;
            v = -to_frag.y;
        }
    }
    let ndc = vec2<f32>(u, v) / max(ma, 0.0001);
    let uv = ndc * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    // Must match POINT_SHADOW_MAP_SIZE in bsengine-rhi-wgpu/src/surface.rs.
    let size = 512.0;
    let px = vec2<i32>(clamp(uv, vec2<f32>(0.0), vec2<f32>(0.999999)) * size);
    let layer = i32(light_index * 6u + face);
    let stored = textureLoad(point_shadow_map, px, layer, 0).r;
    let dist = length(to_frag);
    if (dist - 0.1 > stored) {
        return 0.0;
    }
    return 1.0;
}

@compute @workgroup_size(8, 8, 1)
fn cs_inject(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_X || gid.y >= FROXEL_Y || gid.z >= FROXEL_Z {
        return;
    }
    // Dithered, and deliberately bounded to this froxel's own slice: the sample
    // point moves between that slice's two edges and never past them, so no
    // froxel lights from a neighbour's depth. What it removes is the hard band
    // each slice boundary otherwise draws across a shaft edge -- far more
    // visible here than in uniform fog, because a shaft edge is a step in
    // brightness rather than a gradient. The fine noise left behind is what the
    // temporal accumulation below averages back into an even gradient.
    let world_pos = froxel_world_pos(gid, fog.frame_index);

    let scattering = fog.fog_color * fog.density;
    let extinction = fog.density;
    let view_dir = normalize(world_pos - fog.camera_pos);
    let cos_theta = dot(view_dir, fog.light_dir);
    let phase = henyey_greenstein(cos_theta, fog.anisotropy);
    // The shadow factor is what turns uniform fog into shafts: a froxel the sun
    // cannot reach scatters nothing from it, and those unlit froxels are the
    // dark gaps between the beams.
    let sun_shadow = directional_shadow_factor(world_pos);
    // `var`, not `let`: the point and spot lights add their own contributions
    // to this below.
    var in_scatter = fog.light_color * scattering * phase * sun_shadow;

    // Point lights, accumulated exactly as the scene shader accumulates them
    // into `lo`: the same `dist < range` cutoff, the same
    // `intensity * t * t` falloff with `t = 1 - dist / range`, and the same
    // `point_shadow_factor(i, -to_light)` argument. Matching the attenuation
    // is not cosmetic -- a froxel and a surface at one world position have to
    // agree about how bright the light is there, or a lit floor will sit under
    // fog that thinks the same lamp is twice as strong.
    //
    // What replaces the BRDF is the phase function, the same substitution the
    // sun term above makes: fog has no normal, so `n_dot_l` and the
    // GGX/Fresnel factors have no meaning here, and the medium's `scattering`
    // times the phase at this view angle is what stands in for them.
    for (var i: u32 = 0u; i < lights.num_point_lights; i++) {
        let pl = lights.point_lights[i];
        let to_light = pl.position - world_pos;
        let dist = length(to_light);
        if dist < pl.range {
            let l = normalize(to_light);
            let t = 1.0 - dist / pl.range;
            // `dot(view_dir, l)` with `l` pointing *toward* the light, which is
            // the convention `fog.light_dir` uses for the sun (surface.rs
            // uploads `-light.direction`). Using the travel direction for one
            // and the toward direction for the other would mirror the phase
            // lobe between the two kinds of light.
            let pt_phase = henyey_greenstein(dot(view_dir, l), fog.anisotropy);
            let pt_lit = point_shadow_factor(i, -to_light);
            // The cookie too, as the scene shader applies it: a lantern's cage
            // casts its bars through the haze, not just on the walls.
            in_scatter += pl.color * point_cookie(pl, world_pos) * (pl.intensity * t * t)
                * scattering * pt_phase * pt_lit;
        }
    }

    // Spot lights, likewise mirroring the scene shader: the same cone
    // `smoothstep(outer_cos, inner_cos, cos_angle)` and the same
    // `intensity * t * t * spot_factor`, and the same shadow and cookie: the
    // shaft a spot light throws through fog is cut by what cuts its light on
    // the floor, and patterned by its cookie, or the fog would show a beam
    // the lit surfaces below it do not. Hard shadow (`0u`): a froxel is far
    // coarser than a shadow texel, and the temporal blend softens it anyway.
    for (var j: u32 = 0u; j < lights.num_spot_lights; j++) {
        let sl = lights.spot_lights[j];
        let to_light = sl.position - world_pos;
        let dist = length(to_light);
        if dist < sl.range {
            let l = normalize(to_light);
            let cos_angle = dot(-l, sl.direction);
            let spot_factor = smoothstep(sl.outer_cos, sl.inner_cos, cos_angle);
            if spot_factor > 0.0 {
                let t = 1.0 - dist / sl.range;
                let sp_phase = henyey_greenstein(dot(view_dir, l), fog.anisotropy);
                in_scatter += sl.color * spot_cookie(sl, world_pos) * spot_shadow(sl, world_pos, 0u)
                    * (sl.intensity * t * t * spot_factor) * scattering * sp_phase;
            }
        }
    }

    // Rect lights: their radiance over the solid angle the panel fills
    // (`rect_solid_angle`), with the scene shader's range window and, when the
    // light casts one, its shadow. Without this a lit panel lit the floor
    // under the fog and none of the fog between.
    for (var k: u32 = 0u; k < rect_lights.count; k++) {
        let rl = rect_lights.lights[k];
        let d = length(rl.position - world_pos) / max(rl.range, 1e-4);
        let w = clamp(1.0 - d * d * d * d, 0.0, 1.0);
        let window = w * w;
        if window <= 0.0 {
            continue;
        }
        var lit = 1.0;
        if rl.shadow_layer >= 0.0 {
            lit = shadow_layer_lookup(i32(rl.shadow_layer), world_pos, 0u);
        }
        let solid_angle = rect_solid_angle(rl, world_pos);
        let rl_phase = henyey_greenstein(dot(view_dir, normalize(rl.position - world_pos)), fog.anisotropy);
        in_scatter += rl.radiance * (solid_angle * window * lit) * scattering * rl_phase;
    }

    // Temporal accumulation. The dither above bought its way out of banding
    // with per-frame noise, and this is the half of the bargain that pays for
    // it: each froxel's value is an exponential average over the frames that
    // reached the same world position, so the noise averages back out while the
    // dither keeps the bands from re-forming.
    //
    // Reprojection, not a plain read of the same froxel: the grid is anchored to
    // the camera, so between frames a world position slides across it. Blending
    // one froxel with whatever the same *index* held last frame would average
    // two different places in the world and smear the shafts along the view
    // direction as soon as the camera moved.
    //
    // Three ways this frame stands alone rather than blending, all of them the
    // same rule TAA uses: no history exists yet (`history_valid`), the position
    // was outside the previous volume (`h.w`), or -- the case those two do not
    // cover -- the fog was switched off in between, which `apply` reports by
    // clearing `history_valid` because the volumes then hold a stale frame.
    var scatter = vec4<f32>(in_scatter, extinction);
    if fog.history_valid != 0u {
        // The froxel's centre, not the dithered position the lighting above was
        // sampled at -- see `froxel_centre_world_pos` for why that distinction
        // is the difference between an accumulation that settles and one that
        // shimmers.
        let h = froxel_history_uvw(froxel_centre_world_pos(gid));
        if h.w > 0.0 {
            // `textureSampleLevel`, not `textureSample`: this sits under a
            // per-froxel branch, and an implicit-derivative sample does not
            // exist in compute at all, let alone in non-uniform control flow.
            // The trilinear filter it does keep is what stops the reprojection
            // from snapping to whole froxels.
            let prev = textureSampleLevel(history_vol, history_sampler, h.xyz, 0.0);
            scatter = mix(scatter, prev, FOG_HISTORY_WEIGHT);
        }
    }
    textureStore(injection_vol, vec3<i32>(gid), scatter);
}
"#;

/// Integration: one thread per froxel *column*, marching front to back and
/// writing the running (in-scattered light, transmittance) at every slice.
const FOG_INTEGRATE_WGSL: &str = r#"
@group(1) @binding(0) var injection_vol: texture_3d<f32>;
@group(2) @binding(0) var integrated_vol: texture_storage_3d<rgba16float, write>;

@compute @workgroup_size(8, 8, 1)
fn cs_integrate(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_X || gid.y >= FROXEL_Y {
        return;
    }
    var accum = vec3<f32>(0.0, 0.0, 0.0);
    var transmittance = 1.0;
    // Slice 0 starts at the near plane; every later slice starts where the
    // previous one ended.
    var prev_depth = fog.near;
    for (var z: u32 = 0u; z < FROXEL_Z; z = z + 1u) {
        let s = textureLoad(injection_vol, vec3<i32>(i32(gid.x), i32(gid.y), i32(z)), 0);
        let depth = froxel_slice_depth(z);
        let slice_thickness = depth - prev_depth;
        prev_depth = depth;
        let slice_t = exp(-s.a * slice_thickness);
        // Analytic integration across the slice, not a point sample at its
        // centre: a point sample biases thick slices, and the exponential
        // depth distribution guarantees thick slices at distance.
        accum += transmittance * s.rgb * (1.0 - slice_t) / max(s.a, 1e-5);
        transmittance *= slice_t;
        textureStore(
            integrated_vol,
            vec3<i32>(i32(gid.x), i32(gid.y), i32(z)),
            vec4<f32>(accum, transmittance),
        );
    }
}
"#;

/// Full source of the injection shader: the generated grid constants, the fog
/// uniform at group 0, the shared declarations, the group-2 shadow resources,
/// then the pass itself.
/// How much of a froxel's sky a rect light fills: what the injection pass
/// multiplies a rect light's radiance by.
///
/// Fog in-scatters `radiance * phase` over every direction the panel covers.
/// For a panel of uniform radiance that integral is the radiance times the
/// panel's solid angle, exactly when the phase is isotropic and very nearly
/// otherwise (the phase is taken toward the panel's centre). The scene
/// shader's LTC is the same integral weighted by a surface's lobe, which fog
/// has no normal for; with the phase function standing in for the BRDF, the
/// solid angle is what is left of it. Unreal's and HDRP's rect lights both
/// reach their volumetric fog.
///
/// Van Oosterom and Strackee's formula for a triangle's solid angle, over
/// the rectangle's two halves. One-sided, as the panel lights surfaces: zero
/// behind its plane.
const FOG_RECT_WGSL: &str = r#"
fn triangle_solid_angle(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> f32 {
    let la = length(a);
    let lb = length(b);
    let lc = length(c);
    let num = abs(dot(a, cross(b, c)));
    let den = la * lb * lc + dot(a, b) * lc + dot(a, c) * lb + dot(b, c) * la;
    return 2.0 * atan2(num, den);
}

fn rect_solid_angle(rl: RectLightEntry, p: vec3<f32>) -> f32 {
    // The panel lights out of its local -z, which is `half_width x
    // half_height` pointing away.
    if dot(p - rl.position, cross(rl.half_width, rl.half_height)) >= 0.0 {
        return 0.0;
    }
    let c0 = rl.position + rl.half_width - rl.half_height - p;
    let c1 = rl.position - rl.half_width - rl.half_height - p;
    let c2 = rl.position - rl.half_width + rl.half_height - p;
    let c3 = rl.position + rl.half_width + rl.half_height - p;
    return triangle_solid_angle(c0, c1, c2) + triangle_solid_angle(c0, c2, c3);
}
"#;

fn fog_inject_wgsl() -> String {
    froxel_wgsl_preamble()
        + &fog_uniform_wgsl(0)
        + FROXEL_COMMON_WGSL
        + FROXEL_SHADOW_WGSL
        + crate::cookie::cookie_wgsl!()
        + crate::spot_shadow::spot_shadow_wgsl!()
        + crate::area_light::rect_light_decl_wgsl!()
        + FOG_RECT_WGSL
        + FOG_INJECT_WGSL
}

/// Full source of the integration shader. See [`fog_inject_wgsl`].
fn fog_integrate_wgsl() -> String {
    froxel_wgsl_preamble() + &fog_uniform_wgsl(0) + FROXEL_COMMON_WGSL + FOG_INTEGRATE_WGSL
}

/// Full source of the apply shader. Same shared declarations as the compute
/// passes -- which is what makes its `depth_to_froxel_w` the genuine inverse of
/// their `froxel_slice_depth` rather than a re-derivation -- with the uniform
/// at group 3 instead of group 0.
fn fog_apply_wgsl() -> String {
    froxel_wgsl_preamble() + &fog_uniform_wgsl(3) + FROXEL_COMMON_WGSL + FOG_WGSL
}

const BLOOM_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
    grade_enabled: u32, grade_contrast: f32, grade_saturation: f32, grade_lut_contribution: f32,
    grade_filter_r: f32, grade_filter_g: f32, grade_filter_b: f32, grade_lut_size: u32,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var tex_sampler: sampler;
@group(1) @binding(0) var<uniform> config: PostProcessConfig;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

@fragment
fn fs_bloom(in: FullscreenOut) -> @location(0) vec4<f32> {
    if config.bloom_enabled == 0u {
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    let radius_px = config.bloom_radius;
    var accum = vec3<f32>(0.0);
    var weight_sum = 0.0;
    let steps = 4;
    for (var dy = -steps; dy <= steps; dy++) {
        for (var dx = -steps; dx <= steps; dx++) {
            let offset = vec2<f32>(f32(dx), f32(dy)) * texel * (radius_px / f32(steps));
            let sample_uv = clamp(in.uv + offset, vec2<f32>(0.0), vec2<f32>(1.0));
            let color = textureSample(hdr_tex, tex_sampler, sample_uv).rgb;
            let lum = dot(color, vec3<f32>(0.2126, 0.7152, 0.0722));
            let t = config.bloom_threshold;
            let soft = config.bloom_softness * t;
            let knee_low = t - soft;
            let knee_high = t + soft;
            let blend = clamp((lum - knee_low) / max(knee_high - knee_low, 0.0001), 0.0, 1.0);
            let bright = color * blend;
            let w = exp(-0.5 * f32(dx * dx + dy * dy) / f32(steps * steps));
            accum += bright * w;
            weight_sum += w;
        }
    }
    return vec4<f32>(accum / weight_sum * config.bloom_intensity, 1.0);
}
"#;

/// Screen-space reflections.
///
/// # Why screen space, and what that costs
///
/// Unity (HDRP), Unreal and Godot all march the depth buffer and sample the
/// scene colour at the hit. The construction is the same in all three because
/// the trade is the same: it reflects what is already on screen, for the cost
/// of a march, and it cannot reflect what is not. A reflection that leaves the
/// frame, or lands behind something, has no answer -- so every one of them
/// fades those rays out rather than inventing a colour, and so does this.
///
/// The fade is what keeps the failure honest. A ray that walks off the edge of
/// the screen returns nothing and the surface keeps its existing shading;
/// without the fade it would return whatever happened to be at the edge, and a
/// mirror would smear the frame's border across itself.
const SSR_WGSL: &str = r#"
struct SsrCamera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xyz camera position, w the maximum roughness that still reflects.
    cam_pos: vec4<f32>,
    // x steps, y stride in view-space units, z thickness, w intensity.
    params: vec4<f32>,
    // Last frame's unjittered view-projection: where a hit was then.
    prev_view_proj: mat4x4<f32>,
    // x 1 when `scene_tex` holds last frame's lit scene, 0 before there is one.
    misc: vec4<f32>,
};

// Last frame's lit scene (`lit_history`): what a hit found this frame looked
// like then. The trace runs before this frame's opaque pass -- which reads
// its result -- so this frame's scene does not exist yet.
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var scene_samp: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(0) var normal_tex: texture_2d<f32>;
@group(2) @binding(1) var normal_samp: sampler;
@group(3) @binding(0) var<uniform> cam: SsrCamera;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

/// World position of whatever was drawn at this pixel.
fn world_at(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth);
    let p = cam.inv_view_proj * vec4<f32>(ndc, 1.0);
    return p.xyz / p.w;
}

/// Where a world position lands on screen, and how deep it is.
fn project(p: vec3<f32>) -> vec3<f32> {
    let clip = cam.proj * cam.view * vec4<f32>(p, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z);
}

@fragment
fn fs_ssr(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let uv = frag.xy / dims;
    let coord = vec2<i32>(frag.xy);
    let depth = textureLoad(depth_tex, coord, 0);

    let nr = textureSampleLevel(normal_tex, normal_samp, uv, 0.0);
    let n = nr.xyz;
    let roughness = nr.w;
    // The buffer clears to zero, which is not a unit vector: nothing opaque was
    // drawn here, so there is no surface to reflect from.
    let has_surface = dot(n, n) > 0.25 && depth < 1.0;
    // Rough surfaces scatter; a single mirror ray is the wrong answer for them,
    // and the three reference engines all fade SSR out with roughness rather
    // than pretending otherwise.
    let rough_fade = clamp(1.0 - roughness / max(cam.cam_pos.w, 0.0001), 0.0, 1.0);

    let world = world_at(uv, depth);
    let view_dir = normalize(world - cam.cam_pos.xyz);
    let refl = reflect(view_dir, normalize(n));

    // A ray heading back towards the camera has nothing on screen to hit: what
    // it would reflect is behind the viewer.
    //
    // `view_dir` points *from* the camera, so a ray continuing away from it
    // agrees with `view_dir`. ⚠️ Against `-view_dir` this reads backwards, and
    // reads zero at exactly the grazing angles where a floor reflects most --
    // which is a reflection that never appears rather than one that looks
    // wrong.
    let facing = clamp(dot(refl, view_dir), 0.0, 1.0);

    let steps = i32(cam.params.x);
    let stride = cam.params.y;
    let thickness = cam.params.z;

    var hit_uv = vec2<f32>(0.0);
    var found = 0.0;
    // ⚠️ The march starts just *off* the surface, not on it. A ray leaving
    // exactly the point it reflects from lands, at a grazing angle, back on the
    // same pixel -- and a mirror then reflects itself, brightly and everywhere.
    //
    // Half a step, not a distance of its own: an offset larger than the stride
    // would reach further than the march it is supposed to be nudging, and the
    // ray would find things a trace of that length could never have reached.
    var p = world + normalize(n) * (stride * 0.5);
    for (var i = 0; i < steps; i = i + 1) {
        p = p + refl * stride;
        let s = project(p);
        // Off screen: no answer, and the loop stops rather than walking on.
        //
        // ⚠️ Deleting this is not observable from the tests, and the reason is
        // worth knowing rather than guessing at: a `textureLoad` outside the
        // texture reads zero, so an off-screen sample reports a depth of 0 and
        // no ray ever hits anything there. What the break actually buys is the
        // steps it stops spending -- which is real, and which no pixel shows.
        if (s.x < 0.0 || s.x > 1.0 || s.y < 0.0 || s.y > 1.0 || s.z > 1.0) {
            break;
        }
        let px = vec2<i32>(s.xy * dims);
        // The pixel the ray left is the surface it left. Skipping it says that
        // directly, rather than relying on an offset big enough to escape it --
        // which is how the offset above grew past the stride the first time.
        if (px.x == coord.x && px.y == coord.y) {
            continue;
        }
        let scene_depth = textureLoad(depth_tex, px, 0);
        // The ray has gone behind what is drawn there. `thickness` is how deep
        // a surface is assumed to be: without it every ray eventually passes
        // behind something and reports a hit on the far side of the world.
        let behind = s.z - scene_depth;
        if (behind > 0.0 && behind < thickness) {
            hit_uv = s.xy;
            found = 1.0;
            break;
        }
    }

    // Fade towards the frame's edge, where a hit is about to run out of screen.
    let edge = min(
        min(hit_uv.x, 1.0 - hit_uv.x),
        min(hit_uv.y, 1.0 - hit_uv.y)
    );
    let edge_fade = clamp(edge / 0.1, 0.0, 1.0);

    // Where the hit was last frame, through last frame's camera, and its
    // light then -- HDRP's construction: the trace is this frame's, the
    // colour the previous frame's. A hit last frame's camera could not see
    // has no answer, and fades like one that left the screen.
    let hit_depth = textureLoad(depth_tex, vec2<i32>(hit_uv * dims), 0);
    let hit_world = world_at(hit_uv, hit_depth);
    let prev_clip = cam.prev_view_proj * vec4<f32>(hit_world, 1.0);
    var prev_uv = vec2<f32>(-1.0);
    if (prev_clip.w > 0.0) {
        let p = prev_clip.xy / prev_clip.w;
        prev_uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    }
    let seen_last_frame = f32(
        all(prev_uv >= vec2<f32>(0.0)) && all(prev_uv <= vec2<f32>(1.0))
    );

    let weight = found
        * f32(has_surface)
        * rough_fade
        * facing
        * edge_fade
        * seen_last_frame
        * cam.misc.x
        * cam.params.w;
    let reflected = textureSampleLevel(
        scene_tex, scene_samp, clamp(prev_uv, vec2<f32>(0.0), vec2<f32>(1.0)), 0.0
    ).rgb;
    // Premultiplied, with the weight beside it: the opaque pass takes `w` of
    // its specular environment from here and keeps `1 - w` of its own, so a
    // pixel with no hit changes nothing.
    return vec4<f32>(reflected * weight, weight);
}
"#;

/// What the reflection pass needs to know about the camera and the trace.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug, PartialEq)]
pub struct SsrCameraGpu {
    /// View matrix.
    pub view: [[f32; 4]; 4],
    /// Projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Inverse of `proj * view`, for turning a depth sample back into a world
    /// position.
    pub inv_view_proj: [[f32; 4]; 4],
    /// `xyz` camera position; `w` the roughness at which reflections have
    /// faded out entirely.
    pub cam_pos: [f32; 4],
    /// `x` march steps, `y` stride in world units, `z` assumed surface
    /// thickness in NDC depth, `w` overall intensity. Zero intensity is how
    /// the feature is switched off.
    pub params: [f32; 4],
    /// Last frame's unjittered view-projection, which a hit is found in last
    /// frame's lit scene through.
    pub prev_view_proj: [[f32; 4]; 4],
    /// `x` whether that scene exists: set by the post pass, not the caller.
    pub misc: [f32; 4],
}

/// Screen-space GI's trace: per pixel, rays in a cosine-weighted hemisphere
/// about its normal, marched through the depth buffer as the reflection's ray
/// is; a ray that goes behind what is drawn there, within the thickness, has
/// hit it, and brings back the light that surface sends towards the camera
/// in this frame's lit HDR scene (Lambertian surfaces send it every way
/// alike). Writes the light of the rays that hit, `sum(L) / N`, and the share
/// that hit, both times the intensity: the composite replaces that share of
/// the surface's indirect light with this.
///
/// `sum(L) / N`, not `PI * sum(L) / N`: the cosine-weighted estimate of the
/// irradiance is the latter, but the shading here keeps irradiance as
/// `E / PI` -- the flat ambient lights a surface `ambient * albedo`, the sky
/// and the probes store `E / PI` -- so the reflectance multiplies the mean
/// radiance as it is. With the PI, a floor whose rays met only floor under a
/// uniform ambient brightened by PI where it should not have changed at all.
const SSGI_WGSL: &str = r#"
struct SsgiCamera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    // xyz camera position, w the frame index (the rays' rotation).
    cam_pos: vec4<f32>,
    // x rays, y steps, z stride in world units, w thickness in NDC depth.
    params: vec4<f32>,
    // x intensity, y the history's blend, z whether the history holds a
    // previous frame (the temporal pass's), w the trace's scale: 1 at full
    // resolution, 2 at half.
    params2: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    // x 1 when the trace's `history_tex` holds last frame's lit scene.
    misc: vec4<f32>,
};

// This frame's lit scene, before screen-space GI -- one bounce -- and last
// frame's, after it (`lit_history`): where a hit is read from when there is
// one, so the light it brings back has bounced already, and every frame adds
// a bounce. Each surface takes its albedo's share of what reaches it, so the
// bounces converge.
@group(0) @binding(0) var scene_tex: texture_2d<f32>;
@group(0) @binding(1) var history_tex: texture_2d<f32>;
@group(0) @binding(2) var scene_samp: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(0) var normal_tex: texture_2d<f32>;
@group(2) @binding(1) var normal_samp: sampler;
@group(3) @binding(0) var<uniform> cam: SsgiCamera;

const PI: f32 = 3.14159265;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

fn world_at(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec3<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth);
    let p = cam.inv_view_proj * vec4<f32>(ndc, 1.0);
    return p.xyz / p.w;
}

fn project(p: vec3<f32>) -> vec3<f32> {
    let clip = cam.proj * cam.view * vec4<f32>(p, 1.0);
    let ndc = clip.xyz / clip.w;
    return vec3<f32>(ndc.x * 0.5 + 0.5, 0.5 - ndc.y * 0.5, ndc.z);
}

// The full-resolution pixel a trace pixel stands for: itself at full
// resolution; at half, the first of its 2 x 2 block. The same one every
// frame: a sample turning through the block would, on an edge, land on one
// surface and then the other, and the history -- thrown away whenever the
// depth it recorded is another surface's -- would start over every frame
// there. The composite brings the light back up to the other three by depth.
fn sample_coord(h: vec2<i32>, full: vec2<i32>) -> vec2<i32> {
    if cam.params2.w < 1.5 {
        return h;
    }
    return min(h * 2, full - vec2<i32>(1));
}

fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}
fn unit(h: u32) -> f32 {
    return f32(h >> 8u) / 16777216.0;
}

// A direction in the cosine-weighted hemisphere about `n`.
fn cosine_direction(n: vec3<f32>, u1: f32, u2: f32) -> vec3<f32> {
    let r = sqrt(u1);
    let phi = 2.0 * PI * u2;
    let up = select(vec3<f32>(0.0, 0.0, 1.0), vec3<f32>(1.0, 0.0, 0.0), abs(n.z) > 0.9);
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    return normalize(t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt(max(1.0 - u1, 0.0)));
}

@fragment
fn fs_ssgi(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let full = vec2<i32>(textureDimensions(depth_tex, 0));
    let dims = vec2<f32>(full);
    let coord = sample_coord(vec2<i32>(frag.xy), full);
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / dims;
    let depth = textureLoad(depth_tex, coord, 0);
    let nr = textureSampleLevel(normal_tex, normal_samp, uv, 0.0);
    // Nothing opaque here (the normal buffer clears to zero): no surface to
    // light, and nothing replaced.
    if dot(nr.xyz, nr.xyz) < 0.25 || depth >= 1.0 {
        return vec4<f32>(0.0);
    }
    let n = normalize(nr.xyz);
    let world = world_at(uv, depth);
    let rays = u32(cam.params.x);
    let steps = i32(cam.params.y);
    let stride = cam.params.z;
    let thickness = cam.params.w;
    let seed = pcg(u32(coord.x) ^ pcg(u32(coord.y) ^ pcg(u32(cam.cam_pos.w))));

    var light = vec3<f32>(0.0);
    var hits = 0.0;
    for (var r = 0u; r < rays; r++) {
        let h = pcg(seed ^ pcg(r));
        let d = cosine_direction(n, unit(h), unit(pcg(h)));
        // Off the surface by half a step, as the reflection's ray starts, so
        // it does not find the surface it leaves.
        var p = world + n * (stride * 0.5);
        for (var i = 0; i < steps; i++) {
            p = p + d * stride;
            let s = project(p);
            if s.x < 0.0 || s.x > 1.0 || s.y < 0.0 || s.y > 1.0 || s.z > 1.0 {
                break;
            }
            let px = vec2<i32>(s.xy * dims);
            if px.x == coord.x && px.y == coord.y {
                continue;
            }
            let scene_depth = textureLoad(depth_tex, px, 0);
            let behind = s.z - scene_depth;
            if behind > 0.0 && behind < thickness {
                // ⚠️ Not the surface the ray left. Near its start, a grazing
                // ray is a fraction of a pixel above that surface, and the
                // depth it is compared with is the pixel centre's -- on a
                // surface seen at a slant, nearer than the ray by more than
                // its height -- so it reads as behind its own surface, and
                // a flat floor lit itself with its own colour. A surface
                // in the plane the ray left cannot be what it meets: it
                // leaves that plane at once (cosine-weighted, into the
                // hemisphere above it). So a "hit" within half a step of
                // that plane is no hit, and the ray marches on.
                let hit = world_at((vec2<f32>(px) + vec2<f32>(0.5)) / dims, scene_depth);
                if dot(hit - world, n) < 0.5 * stride {
                    continue;
                }
                // Where the hit was last frame, through last frame's camera,
                // and its light then -- lit by the GI as well, which is the
                // next bounce; or, with no last frame or the hit outside what
                // it saw, this frame's, lit directly: one bounce, never none.
                var hit_light = textureSampleLevel(scene_tex, scene_samp, s.xy, 0.0).rgb;
                if (cam.misc.x > 0.5) {
                    let prev_clip = cam.prev_view_proj * vec4<f32>(hit, 1.0);
                    if (prev_clip.w > 0.0) {
                        let p = prev_clip.xy / prev_clip.w;
                        let prev_uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
                        if (all(prev_uv >= vec2<f32>(0.0)) && all(prev_uv <= vec2<f32>(1.0))) {
                            hit_light = textureSampleLevel(history_tex, scene_samp, prev_uv, 0.0).rgb;
                        }
                    }
                }
                light += hit_light;
                hits += 1.0;
                break;
            }
        }
    }
    let k = cam.params2.x / f32(max(rays, 1u));
    return vec4<f32>(light * k, hits * k);
}
"#;

/// Replaces the share of each pixel's indirect diffuse light that the trace
/// found something for: `reflectance * E_traced - share * indirect`, added
/// to the scene. Signed, which the HDR target holds: where the trace saw a
/// darker world than the probes did, the surface darkens.
///
/// At half resolution the traced light is brought up to each pixel from the
/// four trace pixels around it, bilinearly, each weighted as well by how
/// close its view depth (the history's, this frame's sample) is to this
/// pixel's -- a joint bilateral upsample, as Unity HDRP's and Unreal's
/// half-resolution screen traces are brought back -- so the light of a
/// surface does not spread onto the one in front of or behind it. Where
/// none of the four is near -- a thing thinner than a trace pixel, which
/// every trace sample went past -- nothing is replaced: it keeps the light
/// it had, rather than the light of what stands behind it.
const SSGI_COMPOSITE_WGSL: &str = r#"
struct SsgiCamera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    params: vec4<f32>,
    params2: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    // x 1 when the trace's `history_tex` holds last frame's lit scene.
    misc: vec4<f32>,
};
@group(0) @binding(0) var ssgi_tex: texture_2d<f32>;
@group(0) @binding(1) var ssgi_samp: sampler;
@group(1) @binding(0) var indirect_tex: texture_2d<f32>;
@group(1) @binding(1) var albedo_tex: texture_2d<f32>;
@group(1) @binding(2) var trace_depth_tex: texture_2d<f32>;
@group(1) @binding(3) var inputs_samp: sampler;
@group(2) @binding(0) var depth_tex: texture_depth_2d;
@group(3) @binding(0) var<uniform> cam: SsgiCamera;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

fn view_depth(coord: vec2<i32>, depth: f32, dims: vec2<f32>) -> f32 {
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / dims;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let w = cam.inv_view_proj * ndc;
    return (cam.proj * cam.view * vec4<f32>(w.xyz / w.w, 1.0)).w;
}

fn traced_at(coord: vec2<i32>) -> vec4<f32> {
    if cam.params2.w < 1.5 {
        return textureLoad(ssgi_tex, coord, 0);
    }
    let depth = textureLoad(depth_tex, coord, 0);
    if depth >= 1.0 {
        return vec4<f32>(0.0);
    }
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let w0 = view_depth(coord, depth, dims);
    let size = vec2<i32>(textureDimensions(ssgi_tex, 0));
    // This pixel among the trace pixels, which stand at the first pixel of
    // each block: a traced pixel is its own trace exactly (t = 0), the
    // others between the traces around them.
    let f = vec2<f32>(coord) / cam.params2.w;
    let base = vec2<i32>(floor(f));
    let t = f - floor(f);
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    for (var dy = 0; dy <= 1; dy++) {
        for (var dx = 0; dx <= 1; dx++) {
            let q = clamp(base + vec2<i32>(dx, dy), vec2<i32>(0), size - vec2<i32>(1));
            let dq = textureLoad(trace_depth_tex, q, 0).r;
            // Background in the trace: nothing there to bring up.
            if dq <= 0.0 {
                continue;
            }
            let l = textureLoad(ssgi_tex, q, 0);
            let bx = select(1.0 - t.x, t.x, dx == 1);
            let by = select(1.0 - t.y, t.y, dy == 1);
            let diff = abs(dq - w0);
            let w = bx * by * exp(-diff / (0.02 * w0));
            sum += l * w;
            weight += w;
        }
    }
    if weight > 1e-4 {
        return sum / weight;
    }
    return vec4<f32>(0.0);
}

@fragment
fn fs_composite(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    let coord = vec2<i32>(frag.xy);
    let traced = traced_at(coord);
    let indirect = textureLoad(indirect_tex, coord, 0);
    let gb = textureLoad(albedo_tex, coord, 0).xy;
    // The reflectance's red rides in the indirect target's alpha
    // (`GI_INDIRECT_FORMAT`).
    let reflectance = vec3<f32>(indirect.a, gb);
    return vec4<f32>(reflectance * traced.rgb - traced.a * indirect.rgb, 0.0);
}
"#;

/// Screen-space GI's temporal accumulation: this frame's trace blended into
/// the history of where the same surface was last frame -- found by its
/// velocity, or by the depth and the two cameras where it has none, as TAA
/// finds it -- and the history thrown away where the surface there is not
/// the same one (its view depth then is not what last frame recorded, by
/// more than 5%): a surface just uncovered, or a new frame's first. Writes
/// the blend and this pixel's view depth, next frame's history.
const SSGI_TEMPORAL_WGSL: &str = r#"
struct SsgiCamera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    params: vec4<f32>,
    // x intensity, y the history's blend (the share of this frame), z 1 when
    // the history holds a previous frame, w the trace's scale.
    params2: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    // x 1 when the trace's `history_tex` holds last frame's lit scene.
    misc: vec4<f32>,
};
@group(0) @binding(0) var current_tex: texture_2d<f32>;
@group(0) @binding(1) var current_samp: sampler;
@group(1) @binding(0) var history_tex: texture_2d<f32>;
@group(1) @binding(1) var history_depth_tex: texture_2d<f32>;
@group(1) @binding(2) var history_samp: sampler;
@group(2) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(1) var velocity_tex: texture_2d<f32>;
@group(3) @binding(0) var<uniform> cam: SsgiCamera;

struct TemporalOut {
    @location(0) light: vec4<f32>,
    @location(1) depth: vec4<f32>,
};

// The full-resolution pixel a trace pixel stands for: itself at full
// resolution; at half, the first of its 2 x 2 block. The same one every
// frame: a sample turning through the block would, on an edge, land on one
// surface and then the other, and the history -- thrown away whenever the
// depth it recorded is another surface's -- would start over every frame
// there. The composite brings the light back up to the other three by depth.
fn sample_coord(h: vec2<i32>, full: vec2<i32>) -> vec2<i32> {
    if cam.params2.w < 1.5 {
        return h;
    }
    return min(h * 2, full - vec2<i32>(1));
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

@fragment
fn fs_temporal(@builtin(position) frag: vec4<f32>) -> TemporalOut {
    let full = vec2<i32>(textureDimensions(depth_tex, 0));
    let dims = vec2<f32>(full);
    // This trace pixel's own value, and the full-resolution pixel it was
    // traced from this frame: its depth, its velocity, its place.
    let current = textureLoad(current_tex, vec2<i32>(frag.xy), 0);
    let coord = sample_coord(vec2<i32>(frag.xy), full);
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / dims;
    let depth = textureLoad(depth_tex, coord, 0);
    var out: TemporalOut;
    out.light = current;
    out.depth = vec4<f32>(0.0);
    if depth >= 1.0 {
        return out;
    }
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let world_h = cam.inv_view_proj * ndc;
    let world = world_h.xyz / world_h.w;
    // The view depth, as clip w: what the history records of this pixel.
    let w_now = (cam.proj * cam.view * vec4<f32>(world, 1.0)).w;
    out.depth = vec4<f32>(w_now, 0.0, 0.0, 0.0);
    if cam.params2.z < 0.5 {
        return out;
    }
    let prev_clip = cam.prev_view_proj * vec4<f32>(world, 1.0);
    if prev_clip.w <= 0.0 {
        return out;
    }
    let velocity = textureLoad(velocity_tex, coord, 0).xy;
    var prev_uv = uv - velocity;
    if velocity.x > 1000.0 {
        let prev_ndc = prev_clip.xyz / prev_clip.w;
        prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);
    }
    if any(prev_uv < vec2<f32>(0.0)) || any(prev_uv > vec2<f32>(1.0)) {
        return out;
    }
    // Where that is among the history's own pixels, whose samples stand at
    // the first screen pixel of each block (`sample_coord`): at half
    // resolution, screen position `p` is trace position `(p - 0.5) / 2 + 0.5`.
    // ⚠️ Not `prev_uv` itself: that puts each trace pixel's own sample a
    // quarter of a pixel off its centre, and the filtered read then mixes in
    // a quarter of the neighbour's -- across an edge, the other surface's
    // light, blended into this one's history frame after frame.
    let scale = max(cam.params2.w, 1.0);
    let history_size = vec2<f32>(textureDimensions(history_tex, 0));
    let history_uv = ((prev_uv * dims - vec2<f32>(0.5)) / scale + vec2<f32>(0.5)) / history_size;
    let recorded = textureSampleLevel(history_depth_tex, history_samp, history_uv, 0.0).r;
    // The same surface last frame stood where the previous camera saw it at
    // `prev_clip.w`; a history that recorded something else there is
    // another surface's -- the one this pixel's surface was hidden behind.
    if recorded <= 0.0 || abs(recorded - prev_clip.w) > 0.05 * prev_clip.w {
        return out;
    }
    let history = textureSampleLevel(history_tex, history_samp, history_uv, 0.0);
    out.light = mix(history, current, cam.params2.y);
    return out;
}
"#;

/// Screen-space GI's spatial filter: each pixel the weighted mean of its
/// 5 x 5 neighbourhood, weighted by distance and by how alike the
/// neighbour's surface is -- its view depth within a few percent, its normal
/// close -- so the light of one surface does not spill onto another across
/// an edge, as Unity's and Unreal's SSGI denoisers do.
const SSGI_SPATIAL_WGSL: &str = r#"
struct SsgiCamera {
    view: mat4x4<f32>,
    proj: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    cam_pos: vec4<f32>,
    params: vec4<f32>,
    params2: vec4<f32>,
    prev_view_proj: mat4x4<f32>,
    // x 1 when the trace's `history_tex` holds last frame's lit scene.
    misc: vec4<f32>,
};
@group(0) @binding(0) var light_tex: texture_2d<f32>;
@group(0) @binding(1) var light_samp: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(0) var normal_tex: texture_2d<f32>;
@group(2) @binding(1) var normal_samp: sampler;
@group(3) @binding(0) var<uniform> cam: SsgiCamera;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    return vec4<f32>(p[vi], 0.0, 1.0);
}

// The full-resolution pixel a trace pixel stands for: itself at full
// resolution; at half, the first of its 2 x 2 block. The same one every
// frame: a sample turning through the block would, on an edge, land on one
// surface and then the other, and the history -- thrown away whenever the
// depth it recorded is another surface's -- would start over every frame
// there. The composite brings the light back up to the other three by depth.
fn sample_coord(h: vec2<i32>, full: vec2<i32>) -> vec2<i32> {
    if cam.params2.w < 1.5 {
        return h;
    }
    return min(h * 2, full - vec2<i32>(1));
}

fn view_depth(coord: vec2<i32>, dims: vec2<f32>) -> f32 {
    let depth = textureLoad(depth_tex, coord, 0);
    let uv = (vec2<f32>(coord) + vec2<f32>(0.5)) / dims;
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, depth, 1.0);
    let w = cam.inv_view_proj * ndc;
    return (cam.proj * cam.view * vec4<f32>(w.xyz / w.w, 1.0)).w;
}

@fragment
fn fs_spatial(@builtin(position) frag: vec4<f32>) -> @location(0) vec4<f32> {
    // Neighbours among the trace's pixels; their depths and normals at the
    // full-resolution pixels they were traced from.
    let coord = vec2<i32>(frag.xy);
    let size = vec2<i32>(textureDimensions(light_tex, 0));
    let full = vec2<i32>(textureDimensions(depth_tex, 0));
    let dims = vec2<f32>(full);
    let c0 = sample_coord(coord, full);
    if textureLoad(depth_tex, c0, 0) >= 1.0 {
        return vec4<f32>(0.0);
    }
    let d0 = view_depth(c0, dims);
    let n0 = textureLoad(normal_tex, c0, 0).xyz;
    var sum = vec4<f32>(0.0);
    var weight = 0.0;
    for (var dy = -2; dy <= 2; dy++) {
        for (var dx = -2; dx <= 2; dx++) {
            let q = clamp(coord + vec2<i32>(dx, dy), vec2<i32>(0), size - vec2<i32>(1));
            let cq = sample_coord(q, full);
            if textureLoad(depth_tex, cq, 0) >= 1.0 {
                continue;
            }
            let dq = view_depth(cq, dims);
            let nq = textureLoad(normal_tex, cq, 0).xyz;
            let spatial = exp(-f32(dx * dx + dy * dy) / 4.5);
            let depth_w = exp(-abs(dq - d0) / (0.02 * d0));
            let normal_w = pow(max(dot(n0, nq), 0.0), 16.0);
            let w = spatial * depth_w * normal_w;
            sum += textureLoad(light_tex, q, 0) * w;
            weight += w;
        }
    }
    return sum / max(weight, 1e-6);
}
"#;

/// What screen-space GI's trace needs: the camera, unjittered, and the trace.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable, Debug, PartialEq)]
pub struct SsgiCameraGpu {
    /// View matrix.
    pub view: [[f32; 4]; 4],
    /// Projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Inverse of `proj * view`.
    pub inv_view_proj: [[f32; 4]; 4],
    /// `xyz` camera position; `w` the frame index, which turns the rays.
    pub cam_pos: [f32; 4],
    /// `x` rays, `y` steps, `z` stride in world units, `w` thickness in NDC
    /// depth.
    pub params: [f32; 4],
    /// `x` intensity; zero is off. `y` the share of each frame in the
    /// history, `z` whether the history holds a previous frame (set by
    /// [`PostProcessState::update_ssgi_camera`]).
    pub params2: [f32; 4],
    /// Last frame's unjittered view-projection, for the temporal pass and
    /// for finding a hit in last frame's lit scene.
    pub prev_view_proj: [[f32; 4]; 4],
    /// `x` whether that scene exists: set by the post pass, not the caller.
    pub misc: [f32; 4],
}

/// The share of each frame in screen-space GI's history: an exponential
/// mean over about the last ten frames, as Unity's and Unreal's temporal
/// GI filters keep, which settles 4 rays a pixel into the noise of 40.
pub const SSGI_HISTORY_BLEND: f32 = 0.1;

/// Format of the history's view depth: a half float is within the 5% the
/// rejection compares at, to well past any distance a screen trace sees.
const SSGI_DEPTH_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rg16Float;

/// Screen-space GI's trace scale until a camera asks for full resolution:
/// half, as `ScreenSpaceGlobalIllumination::full_resolution` defaults.
const SSGI_DEFAULT_SCALE: u32 = 2;

/// One half of screen-space GI's history.
struct SsgiHistory {
    _light: crate::profiler::TrackedTexture,
    light_view: wgpu::TextureView,
    /// The light alone, as the spatial filter reads it.
    light_bg: wgpu::BindGroup,
    _depth: crate::profiler::TrackedTexture,
    depth_view: wgpu::TextureView,
    /// Light and depth together, as the temporal pass reads last frame's.
    pair_bg: wgpu::BindGroup,
    /// The composite's inputs with this history's depth: the GI targets'
    /// indirect light and reflectance, and the trace's depths to upsample by.
    composite_bg: wgpu::BindGroup,
}

/// Screen-space GI's own targets, at the trace's resolution: made with the
/// rest, and again by themselves when the resolution is switched.
struct SsgiTargets {
    texture: crate::profiler::TrackedTexture,
    view: wgpu::TextureView,
    bg: wgpu::BindGroup,
    history: [SsgiHistory; 2],
}

const SSAO_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
    grade_enabled: u32, grade_contrast: f32, grade_saturation: f32, grade_lut_contribution: f32,
    grade_filter_r: f32, grade_filter_g: f32, grade_filter_b: f32, grade_lut_size: u32,
}
struct SsaoCamera {
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(0) var<uniform> config: PostProcessConfig;
@group(2) @binding(0) var<uniform> cam: SsaoCamera;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

fn load_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    return textureLoad(depth_tex, coord, 0);
}

fn reconstruct_view_pos(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, -(uv.y * 2.0 - 1.0), depth, 1.0);
    let view_pos = cam.inv_proj * ndc;
    return view_pos.xyz / view_pos.w;
}

fn view_pos_at(uv: vec2<f32>) -> vec3<f32> {
    return reconstruct_view_pos(uv, load_depth(uv));
}

// The view-space normal, from depth. On each axis the neighbour nearer in
// depth is the one differenced, so a pixel on a silhouette takes its own
// surface's slope rather than the jump to whatever is behind it; and the
// normal is turned to face the camera (the eye is the view-space origin).
// The first version crossed "right" with "down" -- the uv's y runs down the
// screen -- and so pointed every normal away from the camera: each surface hid
// half its own samples, and every flat surface in view went dark.
fn view_normal(uv: vec2<f32>, pos: vec3<f32>, texel: vec2<f32>) -> vec3<f32> {
    let l = view_pos_at(uv - vec2<f32>(texel.x, 0.0));
    let r = view_pos_at(uv + vec2<f32>(texel.x, 0.0));
    let u = view_pos_at(uv - vec2<f32>(0.0, texel.y));
    let d = view_pos_at(uv + vec2<f32>(0.0, texel.y));
    let dx = select(pos - l, r - pos, abs(r.z - pos.z) < abs(pos.z - l.z));
    let dy = select(pos - u, d - pos, abs(d.z - pos.z) < abs(pos.z - u.z));
    var n = normalize(cross(dx, dy));
    if dot(n, -pos) < 0.0 {
        n = -n;
    }
    return n;
}

// A 4x4 ordered set of rotations, one per pixel of each 4x4 tile: the blur
// pass that follows averages exactly one tile, so the rotations' pattern
// cancels and every pixel ends up sampled at all sixteen rotations. Without
// a per-pixel rotation every pixel sampled the same eight directions, and an
// occluder showed up as eight offset copies of itself -- the translucent discs
// around the spheres in games/gi-demo, and the bands along its walls.
fn rotation_at(frag: vec2<f32>) -> f32 {
    var bayer = array<f32, 16>(0.0, 8.0, 2.0, 10.0, 12.0, 4.0, 14.0, 6.0,
                               3.0, 11.0, 1.0, 9.0, 15.0, 7.0, 13.0, 5.0);
    let p = vec2<u32>(frag) % vec2<u32>(4u);
    return (bayer[p.y * 4u + p.x] + 0.5) / 16.0 * 6.2831853;
}

@fragment
fn fs_ssao(in: FullscreenOut) -> @location(0) vec4<f32> {
    if config.ssao_enabled == 0u {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let depth = load_depth(in.uv);
    if depth >= 1.0 {
        return vec4<f32>(1.0, 1.0, 1.0, 1.0);
    }
    let pos = reconstruct_view_pos(in.uv, depth);
    let dims = vec2<f32>(textureDimensions(depth_tex, 0));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);
    let normal = view_normal(in.uv, pos, texel);
    // The kernel's frame: the normal, and a tangent turned by this pixel's
    // rotation about it.
    let a = rotation_at(in.pos.xy);
    let spin = vec3<f32>(cos(a), sin(a), 0.0);
    var tangent = spin - normal * dot(spin, normal);
    if dot(tangent, tangent) < 1e-6 {
        tangent = vec3<f32>(0.0, cos(a), sin(a)) - normal * dot(vec3<f32>(0.0, cos(a), sin(a)), normal);
    }
    tangent = normalize(tangent);
    let bitangent = cross(normal, tangent);

    let n = clamp(config.ssao_sample_count, 4u, 32u);
    var occlusion = 0.0;
    for (var i: u32 = 0u; i < n; i++) {
        // A hemisphere about the normal, spread by the golden angle, never
        // flatter than 10 degrees off the surface (a sample in the plane
        // compares depth against the surface it lies on), and denser near
        // the centre, where occlusion matters most -- the usual kernel.
        let t = (f32(i) + 0.5) / f32(n);
        let up = mix(1.0, 0.17, t);
        let around = sqrt(max(1.0 - up * up, 0.0));
        let phi = f32(i) * 2.39996323;
        let scale = mix(0.1, 1.0, t * t);
        let k = vec3<f32>(around * cos(phi), around * sin(phi), up) * scale;
        let s = tangent * k.x + bitangent * k.y + normal * k.z;
        let sample_pos = pos + s * config.ssao_radius;
        let clip = cam.proj * vec4<f32>(sample_pos, 1.0);
        if clip.w <= 0.0 { continue; }
        let ndc = clip.xyz / clip.w;
        let sample_uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }
        let actual_pos = view_pos_at(sample_uv);
        // Something in front of the sample point hides it -- unless that
        // something is far in front of this pixel: counted fully within the
        // radius, fading out by twice it, so an object across the room does
        // not occlude the wall. (The usual `smoothstep(0, 1, r / |dz|)` never
        // reaches zero: an object four radii in front still counted a sixth,
        // and printed a faint copy of itself on the floor below.)
        let range_check = 1.0 - smoothstep(config.ssao_radius, 2.0 * config.ssao_radius, abs(pos.z - actual_pos.z));
        if actual_pos.z >= sample_pos.z + config.ssao_bias {
            occlusion += range_check;
        }
    }
    let ao = clamp(1.0 - (occlusion / f32(n)) * config.ssao_intensity, 0.0, 1.0);
    return vec4<f32>(ao, ao, ao, 1.0);
}
"#;

/// The occlusion averaged over the 4x4 tile its per-pixel rotations repeat on
/// (see `rotation_at` in [`SSAO_WGSL`]), skipping pixels at another depth so an
/// object's occlusion does not bleed onto what lies behind it. Without this
/// pass every pixel would show only its own one rotation of the kernel.
const SSAO_BLUR_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct SsaoCamera {
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}
@group(0) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(0) var ao_raw: texture_2d<f32>;
@group(1) @binding(1) var ao_raw_sampler: sampler;
@group(2) @binding(0) var<uniform> cam: SsaoCamera;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

// Distance from the camera, from a depth sample.
fn view_depth(depth: f32) -> f32 {
    let v = cam.inv_proj * vec4<f32>(0.0, 0.0, depth, 1.0);
    return -v.z / v.w;
}

@fragment
fn fs_ssao_blur(in: FullscreenOut) -> @location(0) vec4<f32> {
    let dims = vec2<i32>(textureDimensions(ao_raw, 0));
    let centre = vec2<i32>(in.pos.xy);
    let z0 = view_depth(textureLoad(depth_tex, centre, 0));
    var sum = 0.0;
    var weight = 0.0;
    for (var y: i32 = -2; y < 2; y++) {
        for (var x: i32 = -2; x < 2; x++) {
            let p = clamp(centre + vec2<i32>(x, y), vec2<i32>(0), dims - vec2<i32>(1));
            let z = view_depth(textureLoad(depth_tex, p, 0));
            // Within a tenth of the distance: the same surface, or one
            // touching it; anything further is something else.
            let w = select(0.0, 1.0, abs(z - z0) <= 0.1 * z0);
            sum += textureLoad(ao_raw, p, 0).r * w;
            weight += w;
        }
    }
    let ao = select(1.0, sum / weight, weight > 0.0);
    return vec4<f32>(ao, ao, ao, 1.0);
}
"#;

const COMPOSITE_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
    grade_enabled: u32, grade_contrast: f32, grade_saturation: f32, grade_lut_contribution: f32,
    grade_filter_r: f32, grade_filter_g: f32, grade_filter_b: f32, grade_lut_size: u32,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(1) @binding(0) var bloom_tex: texture_2d<f32>;
@group(1) @binding(1) var bloom_sampler: sampler;
@group(2) @binding(0) var ao_tex: texture_2d<f32>;
@group(2) @binding(1) var ao_sampler: sampler;
@group(3) @binding(0) var<uniform> config: PostProcessConfig;
// The colour LUT strip, and the sampler it is read through (linear, clamped).
// Only this pass declares them: its group 3 has its own layout, the config
// buffer plus these two, so no other pass's bindings move.
@group(3) @binding(1) var lut_tex: texture_2d<f32>;
@group(3) @binding(2) var lut_sampler: sampler;
// HDR display output (`HdrOutputSettings`): `enabled` when the target is a
// float scRGB swapchain; `scale` turns SDR white into scRGB (paper white
// nits / 80); `peak` is the display's brightest as a multiple of SDR white
// (max nits / paper white nits).
struct HdrOutput {
    enabled: u32,
    scale: f32,
    peak: f32,
    _pad: f32,
}
@group(3) @binding(3) var<uniform> hdr_out: HdrOutput;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

// The LUT's output for display-space colour `c`: the strip's `n` slices are
// `n` x `n` each, laid side by side -- red across a slice, green down it,
// blue from slice to slice (Unreal's layout). Texel centres are at
// `(i + 0.5) / n`, so each coordinate is scaled by `n - 1` and offset by half
// a texel; bilinear filtering then interpolates red and green within a
// slice, and the two nearest slices are blended on blue by hand, since
// hardware filtering must not bleed across a slice boundary.
fn sample_lut(c0: vec3<f32>) -> vec3<f32> {
    let c = clamp(c0, vec3<f32>(0.0), vec3<f32>(1.0));
    let n = f32(config.grade_lut_size);
    let b = c.b * (n - 1.0);
    let s0 = floor(b);
    let s1 = min(s0 + 1.0, n - 1.0);
    let x = c.r * (n - 1.0) + 0.5;
    let y = (c.g * (n - 1.0) + 0.5) / n;
    let w = n * n;
    let lo = textureSampleLevel(lut_tex, lut_sampler, vec2<f32>((s0 * n + x) / w, y), 0.0).rgb;
    let hi = textureSampleLevel(lut_tex, lut_sampler, vec2<f32>((s1 * n + x) / w, y), 0.0).rgb;
    return mix(lo, hi, b - s0);
}

fn aces(x: vec3<f32>) -> vec3<f32> {
    let a = 2.51; let b = 0.03; let c = 2.43; let d = 0.59; let e = 0.14;
    return clamp((x * (a * x + b)) / (x * (c * x + d) + e), vec3<f32>(0.0), vec3<f32>(1.0));
}
fn reinhard(x: vec3<f32>) -> vec3<f32> {
    return x / (x + vec3<f32>(1.0));
}
fn reinhard_lum(x: vec3<f32>) -> vec3<f32> {
    let lum = dot(x, vec3<f32>(0.2126, 0.7152, 0.0722));
    let mapped = lum / (lum + 1.0);
    return x * (mapped / max(lum, 0.0001));
}
fn filmic(x: vec3<f32>) -> vec3<f32> {
    let tmp = max(vec3<f32>(0.0), x - vec3<f32>(0.004));
    return (tmp * (6.2 * tmp + vec3<f32>(0.5))) / (tmp * (6.2 * tmp + vec3<f32>(1.7)) + vec3<f32>(0.06));
}

// The exact sRGB transfer functions (IEC 61966-2-1), not a 2.2 power: the
// grade's pivot (display 0.5) and luma are defined on the values the screen
// shows, and a 2.2 curve puts those values somewhere else -- visibly so in
// the darks, where the two curves differ most.
fn linear_to_srgb(c: vec3<f32>) -> vec3<f32> {
    let lo = c * 12.92;
    let hi = 1.055 * pow(max(c, vec3<f32>(0.0)), vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}
fn srgb_to_linear(c: vec3<f32>) -> vec3<f32> {
    let lo = c / 12.92;
    let hi = pow((max(c, vec3<f32>(0.0)) + 0.055) / 1.055, vec3<f32>(2.4));
    return select(hi, lo, c <= vec3<f32>(0.04045));
}

// `ColorGrading`, in display space after the tonemap: contrast about
// mid-grey, then the colour filter, then saturation about Rec. 709 luma --
// Unity's order, Godot's formulas.
//
// "Display space" is sRGB-encoded, the space Godot grades in (after its
// `linear_to_srgb`) and the one a LUT is authored in. This pass's output is
// still *linear* -- the sRGB render target encodes it on write -- so the
// colour is encoded, graded and decoded again here. Grading the linear value
// directly would pivot contrast on linear 0.5, which displays as 188/255:
// "mid-grey" would be nowhere near the middle of the screen's range.
// Clamped, because contrast above 1 pushes past both ends.
fn apply_grading(linear: vec3<f32>) -> vec3<f32> {
    var c = linear_to_srgb(clamp(linear, vec3<f32>(0.0), vec3<f32>(1.0)));
    c = mix(vec3<f32>(0.5), c, config.grade_contrast);
    c = c * vec3<f32>(config.grade_filter_r, config.grade_filter_g, config.grade_filter_b);
    let luma = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
    c = mix(vec3<f32>(luma), c, config.grade_saturation);
    // The LUT last, as Unity's Color Lookup comes after its adjustments, and
    // in the same display space: its texels are display colours.
    if config.grade_lut_size > 1u {
        c = mix(c, sample_lut(c), config.grade_lut_contribution);
    }
    return srgb_to_linear(clamp(c, vec3<f32>(0.0), vec3<f32>(1.0)));
}

fn apply_tonemap(color: vec3<f32>) -> vec3<f32> {
    let scaled = color * pow(2.0, config.tonemap_exposure);
    if config.tonemap_enabled == 0u { return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0)); }
    if config.tonemap_mode == 0u { return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0)); }
    if config.tonemap_mode == 1u { return reinhard(scaled); }
    if config.tonemap_mode == 2u { return reinhard_lum(scaled); }
    if config.tonemap_mode == 4u { return filmic(scaled); }
    return aces(scaled);
}

// The SDR curve for one channel (or, for Reinhard-on-luminance, for the
// luminance), without the final clamp: identity where tonemapping is off.
fn sdr_curve(x: f32) -> f32 {
    let v = vec3<f32>(x);
    if config.tonemap_enabled == 0u || config.tonemap_mode == 0u { return x; }
    if config.tonemap_mode == 1u || config.tonemap_mode == 2u { return reinhard(v).x; }
    if config.tonemap_mode == 4u { return filmic(v).x; }
    return aces(v).x;
}

// HDR output's tonemap for one value: the SDR curve exactly, up to
// scene-linear 1.0 -- so mid-tones look as they do in SDR -- and above it an
// exponential shoulder with the curve's value and slope at 1.0, rolling off to
// the display's peak instead of to white. Scaling the SDR curve itself to the
// peak (k f(x / k)) is the obvious alternative and is wrong: ACES's toe is not
// linear, and mid-grey comes out at half its SDR brightness.
fn hdr_curve(x: f32) -> f32 {
    if x <= 1.0 {
        return sdr_curve(x);
    }
    let s = sdr_curve(1.0);
    let slope = (sdr_curve(1.01) - sdr_curve(0.99)) / 0.02;
    let room = max(hdr_out.peak - s, 1e-4);
    return s + room * (1.0 - exp(-slope * (x - 1.0) / room));
}

// `apply_tonemap` for HDR output: the same curve per channel (or on the
// luminance, for Reinhard-on-luminance), extended by `hdr_curve`. In units of
// SDR white, 0 to `hdr_out.peak`.
fn apply_tonemap_hdr(color: vec3<f32>) -> vec3<f32> {
    let x = max(color * pow(2.0, config.tonemap_exposure), vec3<f32>(0.0));
    if config.tonemap_enabled != 0u && config.tonemap_mode == 2u {
        let lum = dot(x, vec3<f32>(0.2126, 0.7152, 0.0722));
        return x * (hdr_curve(lum) / max(lum, 0.0001));
    }
    return vec3<f32>(hdr_curve(x.r), hdr_curve(x.g), hdr_curve(x.b));
}

@fragment
fn fs_composite(in: FullscreenOut) -> @location(0) vec4<f32> {
    let hdr   = textureSample(hdr_tex,   hdr_sampler,   in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let ao    = textureSample(ao_tex,    ao_sampler,    in.uv).r;
    let combined = hdr * ao + bloom;
    // Graded only when a camera asked for it: with no `ColorGrading` the
    // branch is skipped on a uniform, and the frame is the tonemapped colour
    // exactly as before grading existed.
    if hdr_out.enabled != 0u {
        // Linear scRGB out: graded where SDR is (its 0..1, the range a grade
        // and a LUT are authored for), the highlights above it kept as they
        // are, and SDR white put at paper white.
        var c = apply_tonemap_hdr(combined);
        if config.grade_enabled != 0u {
            c = apply_grading(min(c, vec3<f32>(1.0))) + max(c - vec3<f32>(1.0), vec3<f32>(0.0));
        }
        return vec4<f32>(c * hdr_out.scale, 1.0);
    }
    var ldr = apply_tonemap(combined);
    if config.grade_enabled != 0u {
        ldr = apply_grading(ldr);
    }
    return vec4<f32>(ldr, 1.0);
}
"#;

/// Depth of field: a disk gather over the fogged HDR image, radius from each
/// pixel's distance to the camera (see `DepthOfField`).
///
/// The golden-angle disk spreads `DOF_TAPS` samples evenly over the circle at
/// any radius, so a small blur and a large one use the same count. Every
/// sample is weighted by whether its *own* blur circle reaches this pixel --
/// scatter-as-gather: a pixel receives what the pixels around it would have
/// spread onto it. That is what keeps a sharp subject in front of a blurred
/// background from leaving a halo of its colour on that background (a sharp
/// pixel spreads nowhere), while a blurred foreground still spreads over what
/// is behind it -- in focus or not. A neighbour *behind* a pixel reaches it
/// only as far as that pixel's own blur, so a blurred background cannot
/// spill over a sharp subject in front of it.
const DOF_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct SsaoCamera {
    proj: mat4x4<f32>,
    inv_proj: mat4x4<f32>,
}
struct Dof {
    near_distance: f32, near_transition: f32, far_distance: f32, far_transition: f32,
    max_radius: f32, near_enabled: u32, far_enabled: u32, _pad: u32,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(0) var<uniform> cam: SsaoCamera;
@group(3) @binding(0) var<uniform> dof: Dof;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

// Distance from the camera plane (linear view depth) at `uv`, through the
// inverse projection -- the value `DepthOfField`'s distances are measured in.
fn view_depth(uv: vec2<f32>) -> f32 {
    let dims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(vec2<i32>(uv * vec2<f32>(dims)), vec2<i32>(0), dims - vec2<i32>(1));
    let d = textureLoad(depth_tex, coord, 0);
    let ndc = vec4<f32>(uv.x * 2.0 - 1.0, (1.0 - uv.y) * 2.0 - 1.0, d, 1.0);
    let v = cam.inv_proj * ndc;
    return -v.z / v.w;
}

// Blur radius, in pixels, for a surface `z` from the camera.
fn blur_radius(z: f32) -> f32 {
    var amount = 0.0;
    if dof.far_enabled != 0u {
        amount = max(amount, smoothstep(
            dof.far_distance, dof.far_distance + max(dof.far_transition, 1e-4), z));
    }
    if dof.near_enabled != 0u {
        amount = max(amount, 1.0 - smoothstep(
            dof.near_distance - max(dof.near_transition, 1e-4), dof.near_distance, z));
    }
    return amount * dof.max_radius;
}

const DOF_TAPS: u32 = 32u;
const GOLDEN_ANGLE: f32 = 2.39996323;

@fragment
fn fs_dof(in: FullscreenOut) -> @location(0) vec4<f32> {
    let centre = textureSampleLevel(hdr_tex, hdr_sampler, in.uv, 0.0);
    let z = view_depth(in.uv);
    let radius = blur_radius(z);
    // How far to look. A pixel's own blur, normally -- but with the near band
    // on, an in-focus pixel may still sit under a blurred foreground spreading
    // over it, so every pixel searches the whole disk. (The far band needs no
    // such search: what is blurred there is behind, and hidden behind a sharp
    // pixel, not spread over it.)
    var search = radius;
    if dof.near_enabled != 0u {
        search = dof.max_radius;
    }
    if search < 0.5 {
        return centre;
    }
    let texel = 1.0 / vec2<f32>(textureDimensions(hdr_tex, 0));
    var sum = centre.rgb;
    var weight = 1.0;
    for (var i = 0u; i < DOF_TAPS; i = i + 1u) {
        let r = sqrt((f32(i) + 0.5) / f32(DOF_TAPS)) * search;
        let theta = f32(i) * GOLDEN_ANGLE;
        let uv = in.uv + vec2<f32>(cos(theta), sin(theta)) * r * texel;
        // A neighbour spreads as far as its own blur. One *behind* this pixel
        // spreads no farther than this pixel's blur: whatever is behind a
        // sharp surface is hidden by it, not smeared over it.
        let zs = view_depth(uv);
        let reach = select(blur_radius(zs), min(blur_radius(zs), radius), zs > z);
        let w = clamp(reach - r + 1.0, 0.0, 1.0);
        sum = sum + textureSampleLevel(hdr_tex, hdr_sampler, uv, 0.0).rgb * w;
        weight = weight + w;
    }
    return vec4<f32>(sum / weight, centre.a);
}
"#;

/// Motion blur: each pixel averaged along the screen path its surface took
/// since the previous frame (see `MotionBlur`).
///
/// The path is the velocity the opaque pass wrote -- the surface's own
/// motion and the camera's together, so a moving object streaks while the
/// camera holds still. Where nothing wrote one (the sky, terrain, custom
/// shaders) it is found by reprojection, the way the TAA resolve finds its
/// history: the pixel's depth, through this frame's inverse view-projection,
/// gives a world position, and last frame's view-projection says where that
/// position was on screen then. Both matrices are the unjittered pair TAA
/// already uploads.
///
/// The streak is centred on the pixel, scaled by `intensity`, clamped to
/// `max_blur` of the screen's width, and averaged over `samples` equally
/// weighted taps. `prev_valid == 0` -- the first frame, or the first after
/// a camera cut -- returns the pixel untouched: last frame's matrix is then
/// not the camera this frame moved from.
const MOTION_BLUR_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct TaaCamera {
    inv_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
}
struct MotionBlur {
    intensity: f32, max_blur: f32, samples: u32, prev_valid: u32,
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(1) @binding(0) var depth_tex: texture_depth_2d;
@group(1) @binding(1) var velocity_tex: texture_2d<f32>;
// Binding 1 of the TAA resolve's uniform group; binding 0 (the shared config)
// is in the layout but this shader has no use for it.
@group(2) @binding(1) var<uniform> cam: TaaCamera;
@group(3) @binding(0) var<uniform> mb: MotionBlur;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

@fragment
fn fs_motion_blur(in: FullscreenOut) -> @location(0) vec4<f32> {
    let centre = textureSampleLevel(hdr_tex, hdr_sampler, in.uv, 0.0);
    if mb.prev_valid == 0u {
        return centre;
    }
    let dims = vec2<f32>(textureDimensions(hdr_tex, 0));
    let ddims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(vec2<i32>(in.uv * vec2<f32>(ddims)), vec2<i32>(0), ddims - vec2<i32>(1));
    // The surface's own screen motion where the opaque pass wrote one;
    // elsewhere the camera's, from the depth.
    var motion = textureLoad(velocity_tex, coord, 0).xy;
    if motion.x > 1000.0 {
        let d = textureLoad(depth_tex, coord, 0);
        let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, (1.0 - in.uv.y) * 2.0 - 1.0, d, 1.0);
        let world_h = cam.inv_view_proj * ndc;
        let world = world_h.xyz / world_h.w;
        let prev = cam.prev_view_proj * vec4<f32>(world, 1.0);
        // Behind last frame's camera: there is no screen position to streak from.
        if prev.w <= 1e-5 {
            return centre;
        }
        let prev_uv = vec2<f32>(prev.x / prev.w * 0.5 + 0.5, 0.5 - prev.y / prev.w * 0.5);
        motion = in.uv - prev_uv;
    }
    var blur = motion * mb.intensity;
    // The clamp is in pixels, against the width, so a streak is limited to
    // the same share of the screen whatever its direction.
    let len_px = length(blur * dims);
    let max_px = mb.max_blur * dims.x;
    if len_px > max_px {
        blur = blur * (max_px / len_px);
    }
    if min(len_px, max_px) < 0.5 {
        return centre;
    }
    var sum = vec3<f32>(0.0);
    let n = max(mb.samples, 1u);
    for (var i = 0u; i < n; i = i + 1u) {
        let t = (f32(i) + 0.5) / f32(n) - 0.5;
        sum = sum + textureSampleLevel(hdr_tex, hdr_sampler, in.uv + blur * t, 0.0).rgb;
    }
    return vec4<f32>(sum / f32(n), centre.a);
}
"#;

/// Fast approximate antialiasing (see `Fxaa`), after the composite and before
/// the TAA resolve, on the tonemapped LDR image.
///
/// FXAA 3.11's quality algorithm in outline: a pixel whose four neighbours
/// span enough luma contrast is on an edge; the 3x3 neighbourhood says whether
/// that edge runs horizontally or vertically and which side of it the pixel
/// is on; a walk along the edge in both directions finds where it ends, and
/// the nearer end sets how far across the edge to sample -- half a pixel at a
/// stair-step's corner, nothing in the middle of a long straight run. A
/// separate low-pass term softens features narrower than a pixel. The final
/// colour is one bilinear sample offset across the edge by the larger of the
/// two.
///
/// Luma is taken in perceptual (gamma) space as FXAA expects: when the target
/// is sRGB the sampler hands back linear light, and `srgb_input` says to take
/// its square root first. Under HDR output the target is linear scRGB with SDR
/// white at `hdr_scale`: luma is divided by it and clamped to 1 first, so the
/// thresholds see the same 0..1 they do in SDR (highlights above white are
/// all "white" to the edge test, as they would be on an SDR screen).
const FXAA_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct Fxaa {
    edge_threshold: f32, edge_threshold_min: f32, subpixel: f32, srgb_input: u32,
    hdr_scale: f32, _p0: f32, _p1: f32, _p2: f32,
}
@group(0) @binding(0) var ldr_tex: texture_2d<f32>;
@group(0) @binding(1) var ldr_sampler: sampler;
@group(1) @binding(0) var<uniform> fxaa: Fxaa;

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

fn luma_of(c: vec3<f32>) -> f32 {
    let l = dot(c, vec3<f32>(0.299, 0.587, 0.114));
    if fxaa.hdr_scale > 0.0 {
        return sqrt(clamp(l / fxaa.hdr_scale, 0.0, 1.0));
    }
    if fxaa.srgb_input != 0u {
        return sqrt(l);
    }
    return l;
}

fn luma_at(uv: vec2<f32>) -> f32 {
    return luma_of(textureSampleLevel(ldr_tex, ldr_sampler, uv, 0.0).rgb);
}

// How far each successive step of the edge walk moves, in pixels -- FXAA
// 3.11's quality-12 preset: fine near the pixel, coarser farther out.
const EDGE_STEPS: u32 = 10u;
fn edge_step(i: u32) -> f32 {
    switch i {
        case 0u, 1u, 2u, 3u, 4u: { return 1.0; }
        case 5u: { return 1.5; }
        case 6u, 7u, 8u: { return 2.0; }
        default: { return 4.0; }
    }
}

@fragment
fn fs_fxaa(in: FullscreenOut) -> @location(0) vec4<f32> {
    let texel = 1.0 / vec2<f32>(textureDimensions(ldr_tex, 0));
    let centre = textureSampleLevel(ldr_tex, ldr_sampler, in.uv, 0.0);
    let m = luma_of(centre.rgb);
    let n = luma_at(in.uv + vec2<f32>(0.0, -texel.y));
    let s = luma_at(in.uv + vec2<f32>(0.0, texel.y));
    let e = luma_at(in.uv + vec2<f32>(texel.x, 0.0));
    let w = luma_at(in.uv + vec2<f32>(-texel.x, 0.0));
    let hi = max(max(max(n, s), max(e, w)), m);
    let lo = min(min(min(n, s), min(e, w)), m);
    let range = hi - lo;
    if range < max(fxaa.edge_threshold_min, hi * fxaa.edge_threshold) {
        return centre;
    }

    let nw = luma_at(in.uv + vec2<f32>(-texel.x, -texel.y));
    let ne = luma_at(in.uv + vec2<f32>(texel.x, -texel.y));
    let sw = luma_at(in.uv + vec2<f32>(-texel.x, texel.y));
    let se = luma_at(in.uv + vec2<f32>(texel.x, texel.y));

    // Sub-pixel term: how far the pixel stands out from its neighbourhood's
    // average, relative to the neighbourhood's contrast.
    let average = (2.0 * (n + s + e + w) + (nw + ne + sw + se)) / 12.0;
    let sub = smoothstep(0.0, 1.0, clamp(abs(average - m) / range, 0.0, 1.0));
    let sub_blend = sub * sub * fxaa.subpixel;

    // Which way the edge runs: a horizontal edge changes down the columns.
    let horizontal_change = abs(n + s - 2.0 * m) * 2.0 + abs(ne + se - 2.0 * e)
        + abs(nw + sw - 2.0 * w);
    let vertical_change = abs(e + w - 2.0 * m) * 2.0 + abs(ne + nw - 2.0 * n)
        + abs(se + sw - 2.0 * s);
    let horizontal = horizontal_change >= vertical_change;

    // Which side of the pixel the edge is on: the neighbour across it that
    // differs the most.
    var across = select(texel.x, texel.y, horizontal);
    let positive = select(e, s, horizontal);
    let negative = select(w, n, horizontal);
    let g_pos = abs(positive - m);
    let g_neg = abs(negative - m);
    var opposite = positive;
    var gradient = g_pos;
    if g_neg > g_pos {
        across = -across;
        opposite = negative;
        gradient = g_neg;
    }

    // Walk along the edge, on the line half-way between the pixel and its
    // opposite neighbour, until the luma there leaves the edge's.
    var edge_uv = in.uv;
    var along = vec2<f32>(texel.x, 0.0);
    if horizontal {
        edge_uv.y = edge_uv.y + across * 0.5;
    } else {
        edge_uv.x = edge_uv.x + across * 0.5;
        along = vec2<f32>(0.0, texel.y);
    }
    let edge_luma = (m + opposite) * 0.5;
    let threshold = gradient * 0.25;
    var dist_pos = 0.0;
    var dist_neg = 0.0;
    var end_pos = edge_luma;
    var end_neg = edge_luma;
    var done_pos = false;
    var done_neg = false;
    for (var i = 0u; i < EDGE_STEPS; i = i + 1u) {
        let step = edge_step(i);
        if !done_pos {
            dist_pos = dist_pos + step;
            end_pos = luma_at(edge_uv + along * dist_pos);
            done_pos = abs(end_pos - edge_luma) >= threshold;
        }
        if !done_neg {
            dist_neg = dist_neg + step;
            end_neg = luma_at(edge_uv - along * dist_neg);
            done_neg = abs(end_neg - edge_luma) >= threshold;
        }
        if done_pos && done_neg {
            break;
        }
    }

    // The nearer end decides. If the edge there turns the same way the pixel
    // differs from the edge, the pixel is on the far side of the step and
    // gets nothing; otherwise the closer to that end, the more it blends.
    let nearer_pos = dist_pos < dist_neg;
    let end_delta = select(end_neg, end_pos, nearer_pos) - edge_luma;
    let shortest = min(dist_pos, dist_neg);
    var edge_blend = 0.0;
    if (end_delta < 0.0) != (m - edge_luma < 0.0) {
        edge_blend = 0.5 - shortest / (dist_pos + dist_neg);
    }

    let offset = max(edge_blend, sub_blend) * across;
    var uv = in.uv;
    if horizontal {
        uv.y = uv.y + offset;
    } else {
        uv.x = uv.x + offset;
    }
    return vec4<f32>(textureSampleLevel(ldr_tex, ldr_sampler, uv, 0.0).rgb, centre.a);
}
"#;

/// The temporal-antialiasing resolve pass.
///
/// It reads the composite pass's LDR result, reprojects the previous frame's
/// resolved image through the camera's motion, clamps that history to the
/// local 3x3 neighbourhood of the current frame, and blends the two. The
/// result is written twice -- to the swapchain and to the history target the
/// next frame will reproject from -- which is why the pass exists as a
/// separate blit at all: a pass cannot sample the texture it renders into.
///
/// Reprojection here is **camera-only**: there are no per-object motion
/// vectors, so a moving object's history lands at the wrong pixel by design.
/// The neighbourhood clamp is what keeps that error from smearing across the
/// frame -- it is not an optional quality knob.
///
/// With `config.taa_enabled == 0u` the shader returns the current frame
/// untouched, so the pass stays the exact passthrough it replaced.
const TAA_WGSL: &str = r#"
struct FullscreenOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
}
struct PostProcessConfig {
    bloom_threshold: f32, bloom_softness: f32, bloom_intensity: f32, bloom_radius: f32,
    bloom_enabled: u32, tonemap_mode: u32, tonemap_exposure: f32, tonemap_enabled: u32,
    ssao_radius: f32, ssao_bias: f32, ssao_intensity: f32, ssao_sample_count: u32,
    ssao_enabled: u32, taa_enabled: u32, taa_history_blend: f32, taa_clamp_strength: f32,
    grade_enabled: u32, grade_contrast: f32, grade_saturation: f32, grade_lut_contribution: f32,
    grade_filter_r: f32, grade_filter_g: f32, grade_filter_b: f32, grade_lut_size: u32,
}
struct TaaCamera {
    inv_view_proj: mat4x4<f32>,
    prev_view_proj: mat4x4<f32>,
}
@group(0) @binding(0) var ldr_tex: texture_2d<f32>;
@group(0) @binding(1) var ldr_sampler: sampler;
@group(1) @binding(0) var history_tex: texture_2d<f32>;
@group(1) @binding(1) var history_sampler: sampler;
@group(2) @binding(0) var depth_tex: texture_depth_2d;
@group(2) @binding(1) var velocity_tex: texture_2d<f32>;
// Both uniforms share group 3: four groups is the WebGPU baseline
// `max_bind_groups`, and LDR/history/depth already take three.
@group(3) @binding(0) var<uniform> config: PostProcessConfig;
@group(3) @binding(1) var<uniform> cam: TaaCamera;

// The resolved colour goes to two attachments: @location(0) is the swapchain,
// @location(1) is next frame's history. They are always the same colour.
struct TaaOut {
    @location(0) color: vec4<f32>,
    @location(1) history: vec4<f32>,
}

fn taa_out(c: vec4<f32>) -> TaaOut {
    var out: TaaOut;
    out.color = c;
    out.history = c;
    return out;
}

@vertex
fn vs_fullscreen(@builtin(vertex_index) vi: u32) -> FullscreenOut {
    var positions = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -3.0), vec2<f32>(-1.0, 1.0), vec2<f32>(3.0, 1.0),
    );
    let p = positions[vi];
    var out: FullscreenOut;
    out.pos = vec4<f32>(p.x, p.y, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, -p.y * 0.5 + 0.5);
    return out;
}

@fragment
fn fs_taa(in: FullscreenOut) -> TaaOut {
    let current = textureSample(ldr_tex, ldr_sampler, in.uv);
    if config.taa_enabled == 0u {
        return taa_out(current);
    }

    let dims = vec2<f32>(textureDimensions(ldr_tex, 0));
    let texel = vec2<f32>(1.0 / dims.x, 1.0 / dims.y);

    // Neighbourhood bounds, gathered from THIS frame before any history is
    // consulted: the clamp below is only meaningful against the range the
    // current frame actually contains. These `textureSample` calls also have
    // to happen here, above the per-pixel early-outs, because implicit
    // derivatives are only legal in uniform control flow.
    var lo = current.rgb;
    var hi = current.rgb;
    for (var dy: i32 = -1; dy <= 1; dy = dy + 1) {
        for (var dx: i32 = -1; dx <= 1; dx = dx + 1) {
            let s = textureSample(
                ldr_tex, ldr_sampler,
                in.uv + vec2<f32>(f32(dx), f32(dy)) * texel
            ).rgb;
            lo = min(lo, s);
            hi = max(hi, s);
        }
    }

    let depth_dims = vec2<i32>(textureDimensions(depth_tex, 0));
    let coord = clamp(
        vec2<i32>(in.uv * vec2<f32>(depth_dims)),
        vec2<i32>(0),
        depth_dims - vec2<i32>(1)
    );
    let depth = textureLoad(depth_tex, coord, 0);
    // Nothing was drawn here: sky/background has no reliable surface to
    // reproject, so trust the current frame.
    if depth >= 1.0 {
        return taa_out(current);
    }

    // Where this surface was last frame. Wherever an opaque mesh covers the
    // pixel, it wrote its velocity -- its own motion and the camera's --
    // and a moving object finds its own history. Elsewhere (no velocity:
    // see `VELOCITY_NONE`) the depth and the two cameras say, which is exact
    // for anything that did not move.
    let velocity = textureLoad(velocity_tex, coord, 0).xy;
    var prev_uv = in.uv - velocity;
    if velocity.x > 1000.0 {
        // Pixel + depth -> world position -> where it was last frame.
        let ndc = vec4<f32>(in.uv.x * 2.0 - 1.0, 1.0 - in.uv.y * 2.0, depth, 1.0);
        let world_h = cam.inv_view_proj * ndc;
        let world = world_h.xyz / world_h.w;
        let prev_clip = cam.prev_view_proj * vec4<f32>(world, 1.0);
        // Behind the eye last frame: there is no previous-frame pixel to find.
        if prev_clip.w <= 0.0 {
            return taa_out(current);
        }
        let prev_ndc = prev_clip.xyz / prev_clip.w;
        prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);
    }

    // History from outside the previous frame simply does not exist.
    if prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0 {
        return taa_out(current);
    }

    // `textureSampleLevel`, not `textureSample`: this read sits under the
    // per-pixel branches above, and an implicit-derivative sample there is
    // non-uniform control flow. The history texture has a single mip, so
    // level 0 is the only level either call could have read.
    let history = textureSampleLevel(history_tex, history_sampler, prev_uv, 0.0).rgb;

    // Clamp history into the local range. This is what stops whatever the
    // reprojection still gets wrong -- a surface just uncovered, which has
    // no history of its own, or an object with no velocity (a custom
    // shader) that moved -- from smearing across the frame.
    let centre = (lo + hi) * 0.5;
    let extent = (hi - lo) * 0.5 * config.taa_clamp_strength;
    let clamped = clamp(history, centre - extent, centre + extent);

    return taa_out(vec4<f32>(
        mix(current.rgb, clamped, config.taa_history_blend),
        current.a
    ));
}
"#;

/// GPU-uniform-buffer layout for bloom/tonemap/SSAO settings, matching the
/// `PostProcessConfig` struct declared in the WGSL shaders above.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PostProcessConfigGpu {
    /// Luminance level above which pixels start contributing to bloom.
    pub bloom_threshold: f32,
    /// Width of the soft knee around `bloom_threshold`.
    pub bloom_softness: f32,
    /// Multiplier applied to the bloom contribution during composite.
    pub bloom_intensity: f32,
    /// Blur sample radius in pixels for the bloom pass.
    pub bloom_radius: f32,
    /// Nonzero to enable the bloom pass; zero disables it.
    pub bloom_enabled: u32,
    /// Selects the tonemap curve (0=clamp, 1=Reinhard, 2=Reinhard-luminance, 4=filmic, else ACES).
    pub tonemap_mode: u32,
    /// Exposure adjustment (stops, applied as `2^exposure`) before tonemapping.
    pub tonemap_exposure: f32,
    /// Nonzero to enable tonemapping; zero passes color through clamped but linear.
    pub tonemap_enabled: u32,
    /// World/view-space sample radius for SSAO occlusion checks.
    pub ssao_radius: f32,
    /// Depth bias added to avoid SSAO self-occlusion artifacts.
    pub ssao_bias: f32,
    /// Multiplier applied to the computed occlusion amount.
    pub ssao_intensity: f32,
    /// Number of hemisphere samples to take per pixel (capped at 8 in the shader).
    pub ssao_sample_count: u32,
    /// Nonzero to enable the SSAO pass; zero always returns full visibility.
    pub ssao_enabled: u32,
    /// Nonzero to enable the TAA pass; zero passes the composite result
    /// through unchanged.
    pub taa_enabled: u32,
    /// See `Taa::history_blend`.
    pub taa_history_blend: f32,
    /// See `Taa::clamp_strength`.
    pub taa_clamp_strength: f32,
    /// Nonzero to apply `ColorGrading` after the tonemap; zero leaves the
    /// tonemapped colour exactly as it was.
    pub grade_enabled: u32,
    /// See `ColorGrading::contrast`.
    pub grade_contrast: f32,
    /// See `ColorGrading::saturation`.
    pub grade_saturation: f32,
    /// See `ColorGrading::lut_contribution`.
    pub grade_lut_contribution: f32,
    /// `ColorGrading::color_filter`, red.
    pub grade_filter_r: f32,
    /// Green.
    pub grade_filter_g: f32,
    /// Blue.
    pub grade_filter_b: f32,
    /// Slices in the bound LUT strip (its height), or 0 when none is bound
    /// -- which is what skips the lookup. Set from what the post pass
    /// actually holds, never from the component, so a LUT that failed to
    /// load cannot be sampled as if it had.
    pub grade_lut_size: u32,
}

/// Mirrors the WGSL `Dof` uniform.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DofGpu {
    near_distance: f32,
    near_transition: f32,
    far_distance: f32,
    far_transition: f32,
    max_radius: f32,
    near_enabled: u32,
    far_enabled: u32,
    _pad: u32,
}

const DOF_SIZE: u64 = std::mem::size_of::<DofGpu>() as u64;

/// Mirrors the WGSL `MotionBlur` uniform.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct MotionBlurGpu {
    intensity: f32,
    max_blur: f32,
    samples: u32,
    prev_valid: u32,
}

const MOTION_BLUR_SIZE: u64 = std::mem::size_of::<MotionBlurGpu>() as u64;

/// Mirrors the WGSL `Fxaa` uniform.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct FxaaGpu {
    edge_threshold: f32,
    edge_threshold_min: f32,
    subpixel: f32,
    srgb_input: u32,
    /// SDR white in the target under HDR output; 0 otherwise.
    hdr_scale: f32,
    _pad: [f32; 3],
}

const FXAA_SIZE: u64 = std::mem::size_of::<FxaaGpu>() as u64;

/// Format of the post pass's own LUT copy: the linear (non-sRGB) twin of
/// the `Rgba8UnormSrgb` images are stored as. See `PostProcessState::set_lut`.
const LUT_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

/// The composite pass's group 3: config buffer, LUT, LUT sampler, HDR output.
fn make_composite_config_bg(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    config_buffer: &wgpu::Buffer,
    lut_view: &wgpu::TextureView,
    sampler: &wgpu::Sampler,
    hdr_buffer: &wgpu::Buffer,
) -> wgpu::BindGroup {
    device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("pp composite config bg"),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: config_buffer.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(sampler),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: hdr_buffer.as_entire_binding(),
            },
        ],
    })
}

/// The composite pass's HDR output uniform (`hdr_out` in [`COMPOSITE_WGSL`]).
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HdrOutputGpu {
    /// Nonzero for an scRGB float target.
    pub enabled: u32,
    /// SDR white in scRGB: paper white nits / 80.
    pub scale: f32,
    /// The display's peak in units of SDR white: max nits / paper white nits.
    pub peak: f32,
    _pad: f32,
}

impl HdrOutputGpu {
    /// The uniform for `settings`, or off.
    pub fn from_settings(settings: Option<bsengine_core::HdrOutputSettings>) -> Self {
        match settings {
            Some(s) => Self {
                enabled: 1,
                scale: s.paper_white_nits / 80.0,
                peak: (s.max_nits / s.paper_white_nits).max(1.0),
                _pad: 0.0,
            },
            None => Self::default(),
        }
    }
}

/// GPU-uniform-buffer layout for the camera matrices the SSAO pass needs to
/// reconstruct view-space position from depth.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SsaoCameraGpu {
    /// Camera projection matrix.
    pub proj: [[f32; 4]; 4],
    /// Inverse of `proj`, used to unproject depth back to view space.
    pub inv_proj: [[f32; 4]; 4],
}

/// GPU-uniform-buffer layout for the matrices the TAA pass needs to
/// reproject a pixel into the previous frame.
///
/// Both matrices are deliberately **unjittered**. Jitter belongs to
/// rasterization only: reprojecting with jittered matrices would chase the
/// jitter instead of the camera, and the accumulated image would never
/// converge.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TaaCameraGpu {
    /// Inverse of this frame's UNJITTERED view-projection, used to turn a
    /// pixel plus its depth back into a world-space position.
    pub inv_view_proj: [[f32; 4]; 4],
    /// The PREVIOUS frame's unjittered view-projection, used to find where
    /// that world position appeared last frame.
    pub prev_view_proj: [[f32; 4]; 4],
}

/// GPU-uniform layout for the froxel passes.
///
/// A buffer of its own rather than more fields on [`PostProcessConfigGpu`]:
/// that struct is exactly 64 bytes and full, IBL and TAA having taken the last
/// of its padding.
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct FogUniform {
    /// Inverse view-projection, to turn a froxel into a world position.
    pub inv_view_proj: [[f32; 4]; 4],
    /// The directional shadow cascades -- the same matrices the scene
    /// shader's camera uniform carries.
    ///
    /// They live here rather than being read out of the shared light uniform
    /// because they are not in there: on the scene side they belong to the
    /// *camera* uniform, next to `view_proj`, and this pass already carries its
    /// own camera matrices for the same reason (they are the jittered ones).
    pub cascade_view_proj: [[[f32; 4]; 4]; crate::shadow::MAX_CASCADES],
    /// The PREVIOUS frame's view-projection, used to find where a froxel's world
    /// position sat in the previous frame's volume.
    ///
    /// Unjittered, like [`TaaCameraGpu`]'s pair and for the same reason:
    /// reprojection has to track the camera, and chasing a sub-pixel offset
    /// would keep the accumulation from ever settling.
    pub prev_view_proj: [[f32; 4]; 4],
    /// Inverse of [`FogUniform::prev_view_proj`], used to rebuild the previous
    /// frame's far-plane ray and so recover a view depth from a world position.
    pub prev_inv_view_proj: [[f32; 4]; 4],
    /// Camera position in world space.
    pub camera_pos: [f32; 3],
    /// Camera near plane; the front edge of the first depth slice.
    pub near: f32,
    /// Direction *toward* the light, matching the scene shader's convention.
    pub light_dir: [f32; 3],
    /// Camera far plane; the back edge of the last depth slice.
    pub far: f32,
    /// Radiance of the directional light the fog scatters.
    pub light_color: [f32; 3],
    /// Uniform participating-media density (both scattering and extinction).
    pub density: f32,
    /// Albedo of the medium, multiplied into the scattering coefficient.
    pub fog_color: [f32; 3],
    /// Henyey-Greenstein anisotropy `g`; see [`crate::froxel::henyey_greenstein`].
    pub anisotropy: f32,
    /// The PREVIOUS frame's camera position, the origin the reprojection ray is
    /// measured from.
    pub prev_camera_pos: [f32; 3],
    /// Cross-fade width at each cascade boundary, matching the scene
    /// shader's. Took a padding slot, so the struct grew only by the cascade
    /// data itself.
    pub cascade_blend: f32,
    /// Nonzero to run the fog passes at all.
    pub enabled: u32,
    /// The frame counter driving the injection pass's depth dither; see
    /// [`crate::froxel::froxel_jitter`].
    ///
    /// The same counter the TAA jitter cycle runs on, and it has to advance
    /// whether or not TAA is enabled: a frozen index would make every frame pick
    /// the same offset, which is a fixed pattern rather than dithering and
    /// averages to nothing. It takes the slot that used to be padding, so the
    /// uniform is the same size it was.
    pub frame_index: u32,
    /// Nonzero once the scattering ping-pong holds a frame worth blending
    /// against.
    ///
    /// **Written by [`PostProcessState::update_fog`], not by its caller.** The
    /// flag belongs to the ping-pong, which only `apply` advances, so whatever a
    /// caller puts here is overwritten. It is a field of this struct rather than
    /// a separate uniform because the injection shader reads it per froxel.
    pub history_valid: u32,
    /// How many of [`FogUniform::cascade_view_proj`] are live. Took the other
    /// padding slot.
    pub cascade_count: u32,
    /// Far distance of each cascade; entries past `cascade_count` sit beyond
    /// any reachable depth.
    pub cascade_splits: [f32; 4],
    /// The camera's forward axis, so a froxel measures depth along the axis
    /// the cascades were sliced along -- the same one the scene shader uses.
    pub cam_forward: [f32; 3],
    /// Padding to the 16-byte uniform stride. See `fog_uniform_size`.
    pub _pad0: f32,
}

/// The scene's shadow resources, borrowed for the froxel injection pass's
/// group 2.
///
/// Gathered into a struct for the same reason `surface.rs`'s `LightBindings`
/// is: four loose references in a constructor argument list are four chances to
/// hand over the point shadow view where the directional one belongs, and both
/// are `&wgpu::TextureView`.
///
/// Every field is owned by [`crate::surface::WgpuSurface`] for its whole life
/// and none of them is recreated on resize -- the shadow maps are fixed-size,
/// and so is the froxel grid -- so the bind group built from these is built
/// once, in [`PostProcessState::new`].
pub struct FroxelShadowBindings<'a> {
    /// The directional shadow map, as the scene shader's group 2 binding 2
    /// binds it.
    pub shadow_map_view: &'a wgpu::TextureView,
    /// The `LessEqual` comparison sampler the directional lookup uses.
    pub shadow_sampler: &'a wgpu::Sampler,
    /// The full `D2Array` view of the point-light linear-distance cube array.
    pub point_shadow_view: &'a wgpu::TextureView,
    /// The scene's light uniform buffer, shared rather than copied. See
    /// [`FROXEL_SHADOW_WGSL`].
    pub light_buffer: &'a wgpu::Buffer,
    /// The spot lights' cookie atlas and its sampler (`crate::cookie`), at
    /// the bindings the scene shader gives them, 18 and 19.
    pub cookie_view: &'a wgpu::TextureView,
    /// The cookie atlas's bilinear sampler.
    pub cookie_sampler: &'a wgpu::Sampler,
    /// The spot and rect lights' shadow maps and their matrices
    /// (`crate::spot_shadow`), at 20 and 21, as in the scene shader.
    pub spot_shadow_view: &'a wgpu::TextureView,
    /// Every spot and rect shadow's view-projection.
    pub spot_shadow_buffer: &'a wgpu::Buffer,
    /// The rect lights' uniform (`crate::area_light`), at 14 as in the scene
    /// shader.
    pub rect_light_buffer: &'a wgpu::Buffer,
}

struct PostProcessTargets {
    hdr_texture: crate::profiler::TrackedTexture,
    hdr_view: wgpu::TextureView,
    hdr_bg: wgpu::BindGroup,
    /// Second HDR target. The scene renders into `hdr_texture`; the fog pass
    /// reads that and writes here, and bloom/composite read this one. Two
    /// targets because a pass may not sample the texture it renders to.
    fog_hdr_texture: crate::profiler::TrackedTexture,
    fog_hdr_view: wgpu::TextureView,
    fog_hdr_bg: wgpu::BindGroup,
    bloom_texture: crate::profiler::TrackedTexture,
    bloom_view: wgpu::TextureView,
    bloom_bg: wgpu::BindGroup,
    ao_texture: crate::profiler::TrackedTexture,
    ao_view: wgpu::TextureView,
    ao_bg: wgpu::BindGroup,
    /// The occlusion as the SSAO pass computes it, one kernel rotation per
    /// pixel, before the blur pass averages it into `ao_view`.
    ao_raw_texture: crate::profiler::TrackedTexture,
    ao_raw_view: wgpu::TextureView,
    ao_raw_bg: wgpu::BindGroup,
    /// World-space normal in rgb and roughness in alpha, written by the opaque
    /// pass alongside its colour.
    ///
    /// Written rather than reconstructed from depth: the opaque pass has
    /// already computed the normal it shaded with, decals included, and
    /// reconstructing loses exactly the detail that makes a reflection look
    /// like it belongs to the surface.
    normal_texture: crate::profiler::TrackedTexture,
    normal_view: wgpu::TextureView,
    normal_bg: wgpu::BindGroup,
    /// Each pixel's screen motion since last frame (see [`VELOCITY_FORMAT`]),
    /// written by the opaque pass, read through `depth_bg`.
    velocity_texture: crate::profiler::TrackedTexture,
    velocity_view: wgpu::TextureView,
    /// The opaque pass's GI albedo and indirect light (see
    /// [`GI_ALBEDO_FORMAT`], [`GI_INDIRECT_FORMAT`]).
    gi_albedo_texture: crate::profiler::TrackedTexture,
    gi_albedo_view: wgpu::TextureView,
    gi_indirect_texture: crate::profiler::TrackedTexture,
    gi_indirect_view: wgpu::TextureView,
    gi_indirect_bg: wgpu::BindGroup,
    gi_albedo_bg: wgpu::BindGroup,
    /// Where screen-space GI's trace writes, before the composite.
    ssgi_texture: crate::profiler::TrackedTexture,
    ssgi_view: wgpu::TextureView,
    ssgi_bg: wgpu::BindGroup,
    /// The trace's history, two of it: the light and share, and each
    /// pixel's view depth.
    ssgi_history: [SsgiHistory; 2],
    /// Where the reflection pass writes, for the opaque pass to read.
    ssr_texture: crate::profiler::TrackedTexture,
    ssr_view: wgpu::TextureView,
    /// The prepass's normals and roughness, which the reflection pass traces
    /// from.
    ssr_normal_texture: crate::profiler::TrackedTexture,
    ssr_normal_view: wgpu::TextureView,
    ssr_normal_bg: wgpu::BindGroup,
    /// Last frame's lit scene, which a hit's light is read from.
    lit_history_texture: crate::profiler::TrackedTexture,
    lit_history_bg: wgpu::BindGroup,
    /// This frame's lit scene and that one, as the GI trace reads them.
    ssgi_scene_bg: wgpu::BindGroup,
    ldr_texture: crate::profiler::TrackedTexture,
    ldr_view: wgpu::TextureView,
    ldr_bg: wgpu::BindGroup,
    fxaa_texture: crate::profiler::TrackedTexture,
    fxaa_view: wgpu::TextureView,
    fxaa_bg: wgpu::BindGroup,
    history_textures: [crate::profiler::TrackedTexture; 2],
    history_views: [wgpu::TextureView; 2],
    history_bgs: [wgpu::BindGroup; 2],
    depth_bg: wgpu::BindGroup,
}

/// Owns the render targets, bind groups, and pipelines for the post-process
/// chain (fog -> bloom -> SSAO -> tonemapped composite -> TAA resolve onto the
/// swapchain).
pub struct PostProcessState {
    /// View of the HDR scene-color render target the main pass writes into.
    pub hdr_view: wgpu::TextureView,
    _hdr_texture: crate::profiler::TrackedTexture,
    /// The fog pass's HDR output, and the image the rest of the chain reads.
    /// `hdr_view` above is what the scene rendered; this is that image with
    /// fog applied, and it exists as a second target because the fog pass
    /// cannot sample the texture it renders into.
    fog_hdr_view: wgpu::TextureView,
    _fog_hdr_texture: crate::profiler::TrackedTexture,
    bloom_view: wgpu::TextureView,
    _bloom_texture: crate::profiler::TrackedTexture,
    ao_view: wgpu::TextureView,
    _ao_texture: crate::profiler::TrackedTexture,
    /// The SSAO pass's unblurred output (see `PostProcessTargets::ao_raw_view`).
    ao_raw_view: wgpu::TextureView,
    _ao_raw_texture: crate::profiler::TrackedTexture,
    ao_raw_bg: wgpu::BindGroup,
    /// LDR output of the composite pass. Composite used to write straight to
    /// the swapchain; it writes here instead so the TAA pass has a readable
    /// copy of this frame's finished colour -- a pass cannot sample the
    /// texture it is rendering into.
    ldr_view: wgpu::TextureView,
    _ldr_texture: crate::profiler::TrackedTexture,
    /// FXAA's output: the LDR image antialiased, which the TAA resolve then
    /// reads in place of `ldr_view`. A target of its own for the same reason
    /// `ldr_view` exists -- a pass cannot sample what it renders into.
    fxaa_view: wgpu::TextureView,
    _fxaa_texture: crate::profiler::TrackedTexture,
    fxaa_bg: wgpu::BindGroup,
    /// The FXAA pass, its uniform and that uniform's group; `fxaa_on` is
    /// whether this frame runs it (see `update_fxaa`).
    fxaa_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    fxaa_buffer: wgpu::Buffer,
    fxaa_bg_uniform: wgpu::BindGroup,
    fxaa_on: bool,
    /// SMAA's three passes, tables and targets (see `crate::smaa`). When it
    /// runs this frame, FXAA does not: both reference engines that offer
    /// SMAA make the two one choice.
    smaa: crate::smaa::SmaaPass,
    /// Two history targets, swapped each frame: one holds the previous
    /// frame's TAA output while the other receives this frame's. A single
    /// texture cannot be sampled and rendered to in the same pass.
    history_views: [wgpu::TextureView; 2],
    _history_textures: [crate::profiler::TrackedTexture; 2],
    history_bgs: [wgpu::BindGroup; 2],
    /// Index of the history texture holding the PREVIOUS frame's result.
    /// Flipped at the end of every `apply`.
    history_read: usize,
    /// False until a frame has been written to history. The first frame has
    /// no history to reproject, so it must pass through unblended rather
    /// than blending against an uninitialised texture.
    history_valid: bool,
    hdr_bg: wgpu::BindGroup,
    fog_hdr_bg: wgpu::BindGroup,
    bloom_bg: wgpu::BindGroup,
    ao_bg: wgpu::BindGroup,
    /// World normal in rgb and roughness in alpha, written by the opaque pass.
    ///
    /// Public because the main pass attaches it: the scene is what fills it,
    /// exactly as the scene fills `hdr_view`.
    pub normal_view: wgpu::TextureView,
    _normal_texture: crate::profiler::TrackedTexture,
    normal_bg: wgpu::BindGroup,
    /// Each pixel's screen motion since last frame (see [`VELOCITY_FORMAT`]).
    /// Public for the same reason as `normal_view`: the main pass fills it.
    pub velocity_view: wgpu::TextureView,
    _velocity_texture: crate::profiler::TrackedTexture,
    /// The green and blue of the reflectance each pixel's indirect diffuse
    /// was multiplied by ([`GI_ALBEDO_FORMAT`]); the main pass fills it.
    pub gi_albedo_view: wgpu::TextureView,
    _gi_albedo_texture: crate::profiler::TrackedTexture,
    /// The indirect diffuse light each pixel was given, and its reflectance's
    /// red ([`GI_INDIRECT_FORMAT`]); the main pass fills it.
    pub gi_indirect_view: wgpu::TextureView,
    _gi_indirect_texture: crate::profiler::TrackedTexture,
    gi_indirect_bg: wgpu::BindGroup,
    gi_albedo_bg: wgpu::BindGroup,
    /// Screen-space GI's traced light and share, before the composite.
    ssgi_view: wgpu::TextureView,
    _ssgi_texture: crate::profiler::TrackedTexture,
    ssgi_bg: wgpu::BindGroup,
    /// The trace's history, two of it, written in turn: `ssgi_current` is
    /// the one last written, and whether it holds a frame of the current
    /// trace is `ssgi_history_valid`.
    ssgi_history: [SsgiHistory; 2],
    ssgi_current: usize,
    ssgi_history_valid: bool,
    /// Whether the spatial filter runs: always, but for the test that
    /// measures what it takes away.
    ssgi_spatial: bool,
    /// Whether the GI trace reads its hits from last frame's lit scene
    /// (every frame a bounce more): always, but for the tests about
    /// something else that a fresh renderer, with no last frame, is compared
    /// against.
    ssgi_multibounce: bool,
    ssgi_history_bgl: std::sync::Arc<wgpu::BindGroupLayout>,
    ssgi_composite_bgl: std::sync::Arc<wgpu::BindGroupLayout>,
    /// How many screen pixels each way one trace pixel stands for: 2, or 1
    /// at full resolution (`ScreenSpaceGlobalIllumination::full_resolution`).
    ssgi_scale: u32,
    /// The screen's size, which the trace's targets are made from again
    /// when the scale changes.
    ssgi_screen: (u32, u32),
    /// Screen-space GI's passes, its camera and whether it runs.
    ssgi_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    ssgi_temporal_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    ssgi_spatial_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    ssgi_composite_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    ssgi_cam_buffer: wgpu::Buffer,
    ssgi_cam_bg: wgpu::BindGroup,
    ssgi_enabled: bool,
    /// Screen-space reflections, traced before the opaque pass: premultiplied
    /// light in rgb, the share of the specular environment it replaces in
    /// alpha. Bound in the lighting group, which is how the mesh shader reads
    /// it; cleared to nothing when the camera has none.
    pub ssr_view: wgpu::TextureView,
    _ssr_texture: crate::profiler::TrackedTexture,
    /// The prepass's normals and roughness, as the opaque pass would write
    /// them but for the decals -- what the reflections are traced from,
    /// before the opaque pass has written its own.
    pub ssr_normal_view: wgpu::TextureView,
    _ssr_normal_texture: crate::profiler::TrackedTexture,
    ssr_normal_bg: wgpu::BindGroup,
    /// The lit scene of the last frame that traced reflections, copied out
    /// after screen-space GI: what a hit's light is read from.
    lit_history_texture: crate::profiler::TrackedTexture,
    lit_history_bg: wgpu::BindGroup,
    /// This frame's lit scene and last frame's, as the GI trace reads them.
    ssgi_scene_bg: wgpu::BindGroup,
    /// Whether [`Self::lit_history_texture`] holds the frame before this one:
    /// not on the first frame, after a resize, or after a frame without
    /// reflections.
    lit_history_valid: bool,
    ldr_bg: wgpu::BindGroup,
    depth_bg: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    config_buffer: wgpu::Buffer,
    config_bg: wgpu::BindGroup,
    /// The depth-of-field pass, its settings uniform and that uniform's
    /// group; `dof_on` is whether this frame runs it (see `update_dof`).
    dof_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    dof_buffer: wgpu::Buffer,
    dof_bg: wgpu::BindGroup,
    dof_on: bool,
    /// The motion blur pass, its uniform and that uniform's group;
    /// `motion_blur_on` is whether this frame runs it (see
    /// `update_motion_blur`).
    motion_blur_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    motion_blur_buffer: wgpu::Buffer,
    motion_blur_bg: wgpu::BindGroup,
    motion_blur_on: bool,
    /// Whether the previous frame's camera -- the one `TaaCameraGpu`'s
    /// `prev_view_proj` holds -- is a frame this one continues from. False
    /// before the first frame (the matrix is then the identity it was
    /// initialised to) and after `invalidate_history`, the camera cut; a
    /// streak measured across either would be the distance to an unrelated
    /// camera.
    motion_blur_prev_valid: bool,
    /// The composite pass's group 3 (see where it is built) and its layout,
    /// kept to rebuild the group when the LUT changes.
    composite_config_bgl: std::sync::Arc<wgpu::BindGroupLayout>,
    composite_config_bg: wgpu::BindGroup,
    /// [`HdrOutputGpu`], bound in `composite_config_bg`.
    hdr_buffer: wgpu::Buffer,
    /// SDR white in the output under HDR (`HdrOutputGpu::scale`), 0 in SDR:
    /// what FXAA and SMAA divide luma by.
    hdr_scale: f32,
    _neutral_lut: crate::profiler::TrackedTexture,
    neutral_lut_view: wgpu::TextureView,
    /// The bound LUT strip and its slice count, or `None`.
    lut: Option<(crate::profiler::TrackedTexture, u32)>,
    ssao_cam_buffer: wgpu::Buffer,
    ssao_cam_bg: wgpu::BindGroup,
    taa_cam_buffer: wgpu::Buffer,
    /// The TAA pass's single uniform group: the shared config buffer at
    /// binding 0 and `taa_cam_buffer` at binding 1. One group rather than the
    /// two the other passes use, because the resolve is already at the
    /// four-bind-group WebGPU baseline limit.
    taa_uniform_bg: wgpu::BindGroup,
    /// The scattering volumes: RGB is in-scattered light, A is extinction, one
    /// texel per froxel. Filled by the injection pass, read by the integration
    /// pass.
    ///
    /// A **pair**, swapped each frame, because the injection pass blends this
    /// frame's scattering into the accumulation the previous frame left behind
    /// and a pass cannot sample the volume it is writing. Which half is which is
    /// [`PostProcessState::fog_history_read`].
    ///
    /// **Deliberately not in `PostProcessTargets`, unlike every other texture
    /// in this file.** Everything there lives in that struct because a window
    /// resize has to reallocate it. The froxel grid is a fixed
    /// `FROXEL_X x FROXEL_Y x FROXEL_Z` resolution, chosen independently of the
    /// output size, so these volumes are the only render resources here that a
    /// resize must leave alone -- putting them in `PostProcessTargets` would
    /// silently rebuild them on every resize for no reason.
    _injection_volumes: [crate::profiler::TrackedTexture; 2],
    /// The integrated volume: RGB is accumulated in-scattered light up to each
    /// slice, A is the transmittance to it. Written by the integration pass and
    /// sampled by the apply pass.
    ///
    /// Not resized -- see [`PostProcessState::_injection_volumes`].
    ///
    /// Single, unlike those: nothing reads it across frames, because the
    /// accumulation happens before integration rather than after it.
    _integrated_volume: crate::profiler::TrackedTexture,
    fog_uniform_buffer: wgpu::Buffer,
    fog_uniform_bg: wgpu::BindGroup,
    /// The scene's shadow maps and light uniform, bound for the injection pass
    /// to test each froxel against. Built once from [`FroxelShadowBindings`];
    /// none of the resources behind it is ever recreated.
    froxel_shadow_bg: wgpu::BindGroup,
    /// Each scattering volume bound for writing (injection pass).
    injection_write_bgs: [wgpu::BindGroup; 2],
    /// The same volumes bound for `textureLoad` (integration pass). Separate
    /// bind groups rather than a read-write binding, which the WebGPU baseline
    /// does not offer for storage textures.
    injection_read_bgs: [wgpu::BindGroup; 2],
    /// The same volumes again, bound with a filtering sampler for the injection
    /// pass to reproject its history out of. A third set rather than a reuse of
    /// [`PostProcessState::injection_read_bgs`] because that layout carries no
    /// sampler, and the reprojected lookup lands between froxels.
    injection_history_bgs: [wgpu::BindGroup; 2],
    /// Index of the scattering volume holding the PREVIOUS frame's
    /// accumulation. The injection pass samples this one and writes the other;
    /// they swap after every dispatch.
    fog_history_read: usize,
    /// False until a fog frame has been written to the ping-pong. The first
    /// frame has nothing to reproject, so it must stand alone rather than blend
    /// against an uninitialised volume -- exactly what
    /// [`PostProcessState::history_valid`] does for the TAA resolve.
    ///
    /// Also cleared whenever fog is *off* for a frame: the volumes then keep
    /// whatever the last foggy frame left in them, which a later frame must not
    /// mistake for its own previous one.
    fog_history_valid: bool,
    integrated_write_bg: wgpu::BindGroup,
    /// The integrated volume bound for the apply pass to sample: a filtering
    /// sampler and a `texture_3d<f32>`, not the storage binding the
    /// integration pass writes it through.
    integrated_read_bg: wgpu::BindGroup,
    /// Whether the last [`PostProcessState::update_fog`] enabled the effect.
    ///
    /// Mirrored on the CPU because `apply` skips the dispatch entirely when
    /// fog is off, and it cannot read the flag back out of the uniform buffer.
    fog_enabled: bool,
    /// Compute pipeline that fills the scattering volume.
    fog_inject_pipeline: std::sync::Arc<wgpu::ComputePipeline>,
    /// Compute pipeline that marches each froxel column front to back.
    fog_integrate_pipeline: std::sync::Arc<wgpu::ComputePipeline>,
    /// Pipeline for the volumetric-fog apply shader, the first pass of the
    /// chain: it composites the integrated volume onto the scene's HDR image.
    /// An exact passthrough while `fog.enabled` is zero.
    pub fog_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    /// Pipeline for the bright-pass bloom extraction shader.
    pub bloom_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    /// Pipeline for the SSAO occlusion shader.
    pub ssao_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    /// Averages the SSAO pass's output over the 4x4 tile of its rotations.
    ssao_blur_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    /// Traces the reflections into [`Self::ssr_view`].
    ssr_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    ssr_cam_buffer: wgpu::Buffer,
    ssr_cam_bg: wgpu::BindGroup,
    /// Whether the reflection passes run this frame.
    ssr_enabled: bool,
    /// Pipeline for the final composite (HDR + bloom + AO, tonemapped) shader.
    pub composite_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    /// Pipeline for the temporal-antialiasing resolve. Writes both the
    /// swapchain image and next frame's history target.
    pub taa_pipeline: std::sync::Arc<wgpu::RenderPipeline>,
    tex2d_bgl: std::sync::Arc<wgpu::BindGroupLayout>,
    depth_bgl: std::sync::Arc<wgpu::BindGroupLayout>,
    /// Format of the final swapchain image. The LDR intermediate target must
    /// match it exactly, or the round trip through it would requantize the
    /// frame; kept here so `resize_targets` can recreate that target.
    surface_format: wgpu::TextureFormat,
}

impl PostProcessState {
    /// Builds every pipeline, bind group layout, and sized render target needed
    /// for the post-process chain at the given resolution.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        surface_format: wgpu::TextureFormat,
        shadow: &FroxelShadowBindings,
    ) -> Self {
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("pp sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            // W matters only to the fog apply pass, the one user of a 3D
            // texture here: clamping is what makes a lookup past the far slice
            // hold at that slice instead of wrapping back to the near one.
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let tex2d_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp tex2d bgl"),
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

        let depth_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp depth bgl"),
            // The velocity rides with the depth: the two passes that read it
            // (TAA and motion blur) already use all four bind groups WebGPU
            // guarantees, and they read the depth anyway, as the fallback
            // for pixels with no velocity. Shaders that only want the depth
            // simply do not declare binding 1.
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });

        let config_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp config bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(CONFIG_SIZE),
                },
                count: None,
            }],
        });

        let ssao_cam_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp ssao cam bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(SSAO_CAM_SIZE),
                },
                count: None,
            }],
        });

        // The resolve pass already needs four groups (LDR, history, depth,
        // uniforms) and `max_bind_groups` is 4 on the WebGPU baseline, so the
        // config and camera uniforms share one group rather than taking two.
        let taa_uniform_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp taa uniform bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(CONFIG_SIZE),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(TAA_CAM_SIZE),
                    },
                    count: None,
                },
            ],
        });

        let config_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp config buffer"),
            size: CONFIG_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let config_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp config bg"),
            layout: &config_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: config_buffer.as_entire_binding(),
            }],
        });

        // The composite pass's group 3: the shared config buffer, plus the
        // colour LUT and its sampler, which only that pass reads. Its own
        // layout so that the LUT can be swapped without touching the bloom,
        // SSAO and TAA passes that bind the plain config group.
        let composite_config_bgl =
            device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pp composite config bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(CONFIG_SIZE),
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<HdrOutputGpu>() as u64,
                            ),
                        },
                        count: None,
                    },
                ],
            });
        // HDR output's parameters: off (all zero) until `set_hdr_output`.
        let hdr_buffer = wgpu::util::DeviceExt::create_buffer_init(
            device,
            &wgpu::util::BufferInitDescriptor {
                label: Some("pp hdr output"),
                contents: bytemuck::bytes_of(&HdrOutputGpu::default()),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            },
        );
        // Bound whenever no LUT is: a bind group cannot leave a slot empty.
        // `grade_lut_size` is 0 then, so the shader never reads it.
        let neutral_lut = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("pp neutral lut"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: LUT_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        let neutral_lut_view = neutral_lut.create_view(&wgpu::TextureViewDescriptor::default());
        let composite_config_bg = make_composite_config_bg(
            device,
            &composite_config_bgl,
            &config_buffer,
            &neutral_lut_view,
            &sampler,
            &hdr_buffer,
        );

        let dof_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp dof bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(DOF_SIZE),
                },
                count: None,
            }],
        });
        let dof_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp dof buffer"),
            size: DOF_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dof_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp dof bg"),
            layout: &dof_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: dof_buffer.as_entire_binding(),
            }],
        });

        let motion_blur_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp motion blur bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(MOTION_BLUR_SIZE),
                },
                count: None,
            }],
        });
        let motion_blur_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp motion blur buffer"),
            size: MOTION_BLUR_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let motion_blur_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp motion blur bg"),
            layout: &motion_blur_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: motion_blur_buffer.as_entire_binding(),
            }],
        });

        let fxaa_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp fxaa bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(FXAA_SIZE),
                },
                count: None,
            }],
        });
        let fxaa_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp fxaa buffer"),
            size: FXAA_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fxaa_bg_uniform = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp fxaa uniform bg"),
            layout: &fxaa_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: fxaa_buffer.as_entire_binding(),
            }],
        });

        let ssao_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp ssao cam buffer"),
            size: SSAO_CAM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ssao_cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp ssao cam bg"),
            layout: &ssao_cam_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ssao_cam_buffer.as_entire_binding(),
            }],
        });

        let taa_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp taa cam buffer"),
            size: TAA_CAM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let taa_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp taa uniform bg"),
            layout: &taa_uniform_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: config_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: taa_cam_buffer.as_entire_binding(),
                },
            ],
        });

        // --- froxel volumes and the two compute pipelines ---
        //
        // Created here rather than in `create_targets` on purpose: the froxel
        // grid has a fixed resolution, so unlike every other texture in this
        // file these two must survive a window resize untouched.
        // Visible to both stages: the two compute passes read these parameters
        // to fill the volumes, and the apply pass reads the same buffer for
        // `enabled`, the near/far mapping, and the camera matrix.
        let fog_uniform_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp fog uniform bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(FOG_UNIFORM_SIZE),
                },
                count: None,
            }],
        });
        // Group 2 of the injection pass. Mirrors the scene shader's group 2
        // entry types exactly -- a `Depth` sample type for the directional map,
        // a `Comparison` sampler, and a *non-filterable* float array for the
        // point shadows, which is what `R32Float` allows without the
        // FLOAT32_FILTERABLE feature this engine never requests.
        let froxel_shadow_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp froxel shadow bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        // The directional shadow map, now one layer per
                        // cascade. (The scene depth buffer's own layout, above,
                        // stays D2 -- it is a different texture.)
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(crate::surface::LIGHT_UNIFORM_SIZE),
                    },
                    count: None,
                },
                // 18-21: the spot lights' cookies and the spot and rect
                // lights' shadow maps, at the scene shader's own binding
                // numbers so `cookie_wgsl!` and `spot_shadow_wgsl!` are
                // included here unchanged.
                wgpu::BindGroupLayoutEntry {
                    binding: 18,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 19,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 20,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2Array,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 21,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 14,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let froxel_shadow_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp froxel shadow bg"),
            layout: &froxel_shadow_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(shadow.shadow_map_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(shadow.shadow_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(shadow.point_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: shadow.light_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 18,
                    resource: wgpu::BindingResource::TextureView(shadow.cookie_view),
                },
                wgpu::BindGroupEntry {
                    binding: 19,
                    resource: wgpu::BindingResource::Sampler(shadow.cookie_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 20,
                    resource: wgpu::BindingResource::TextureView(shadow.spot_shadow_view),
                },
                wgpu::BindGroupEntry {
                    binding: 21,
                    resource: shadow.spot_shadow_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 14,
                    resource: shadow.rect_light_buffer.as_entire_binding(),
                },
            ],
        });
        let froxel_write_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp froxel write bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::StorageTexture {
                    access: wgpu::StorageTextureAccess::WriteOnly,
                    format: FROXEL_FORMAT,
                    view_dimension: wgpu::TextureViewDimension::D3,
                },
                count: None,
            }],
        });
        let froxel_read_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp froxel read bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D3,
                    multisampled: false,
                },
                count: None,
            }],
        });
        // Group 3 of the injection pass: the other half of the scattering
        // ping-pong, bound for sampling. A filtering sampler beside the texture,
        // unlike `froxel_read_bgl` above, because the reprojected history lookup
        // lands between froxels and a nearest read would snap it to whole ones.
        let froxel_history_bgl =
            device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pp froxel history bgl"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D3,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::COMPUTE,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        // The apply pass reads the integrated volume as a *sampled* texture --
        // it wants the trilinear filter between froxels, which a storage
        // binding cannot give it -- so its layout is a `Texture` entry with a
        // `D3` view dimension plus a filtering sampler, not the storage entry
        // the integration pass writes through.
        let froxel_apply_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp froxel apply bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D3,
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

        let fog_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp fog uniform buffer"),
            size: FOG_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let fog_uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp fog uniform bg"),
            layout: &fog_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: fog_uniform_buffer.as_entire_binding(),
            }],
        });

        let make_volume = |label: &str| {
            crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width: crate::froxel::FROXEL_X,
                        height: crate::froxel::FROXEL_Y,
                        depth_or_array_layers: crate::froxel::FROXEL_Z,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D3,
                    format: FROXEL_FORMAT,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::STORAGE_BINDING,
                    view_formats: &[],
                },
            )
        };
        let volume_view = |tex: &crate::profiler::TrackedTexture| {
            tex.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D3),
                ..Default::default()
            })
        };
        // Two scattering volumes, not one: the injection pass blends this
        // frame's light into the accumulation the previous frame left, and a
        // pass cannot sample the volume it is writing.
        let injection_volumes = [
            make_volume("pp froxel injection 0"),
            make_volume("pp froxel injection 1"),
        ];
        let integrated_volume = make_volume("pp froxel integrated");
        let injection_views = [
            volume_view(&injection_volumes[0]),
            volume_view(&injection_volumes[1]),
        ];
        let integrated_view = volume_view(&integrated_volume);

        let make_volume_bg =
            |label: &str, bgl: &wgpu::BindGroupLayout, view: &wgpu::TextureView| {
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some(label),
                    layout: bgl,
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    }],
                })
            };
        let injection_write_bgs = [
            make_volume_bg(
                "pp froxel injection write bg 0",
                &froxel_write_bgl,
                &injection_views[0],
            ),
            make_volume_bg(
                "pp froxel injection write bg 1",
                &froxel_write_bgl,
                &injection_views[1],
            ),
        ];
        let injection_read_bgs = [
            make_volume_bg(
                "pp froxel injection read bg 0",
                &froxel_read_bgl,
                &injection_views[0],
            ),
            make_volume_bg(
                "pp froxel injection read bg 1",
                &froxel_read_bgl,
                &injection_views[1],
            ),
        ];
        // The same pair once more, this time with `sampler` beside them. It
        // clamps on every axis, W included, so a reprojection that lands just
        // past the volume's near or far end reads that end's slice rather than
        // wrapping around to the other one.
        let make_history_bg = |label: &str, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &froxel_history_bgl,
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
            })
        };
        let injection_history_bgs = [
            make_history_bg("pp froxel injection history bg 0", &injection_views[0]),
            make_history_bg("pp froxel injection history bg 1", &injection_views[1]),
        ];
        let integrated_write_bg = make_volume_bg(
            "pp froxel integrated write bg",
            &froxel_write_bgl,
            &integrated_view,
        );
        // The same volume the integration pass writes, bound for the apply
        // pass to sample. `sampler` clamps on every axis, W included, so the
        // near and far ends of the volume hold at their edge slices instead of
        // wrapping around to each other.
        let integrated_read_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp froxel integrated read bg"),
            layout: &froxel_apply_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&integrated_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let fog_inject_pipeline = Self::make_fog_inject_pipeline(
            device,
            &fog_uniform_bgl,
            &froxel_write_bgl,
            &froxel_shadow_bgl,
            &froxel_history_bgl,
        );
        let fog_integrate_pipeline = Self::make_fog_integrate_pipeline(
            device,
            &fog_uniform_bgl,
            &froxel_read_bgl,
            &froxel_write_bgl,
        );

        let fog_pipeline = Self::make_fog_pipeline(
            device,
            &tex2d_bgl,
            &depth_bgl,
            &froxel_apply_bgl,
            &fog_uniform_bgl,
        );
        let bloom_pipeline = Self::make_bloom_pipeline(device, &tex2d_bgl, &config_bgl);
        let ssao_pipeline =
            Self::make_ssao_pipeline(device, &depth_bgl, &config_bgl, &ssao_cam_bgl);
        let ssao_blur_pipeline =
            Self::make_ssao_blur_pipeline(device, &depth_bgl, &tex2d_bgl, &ssao_cam_bgl);

        let ssr_cam_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp ssr cam bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let ssr_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp ssr cam buffer"),
            size: std::mem::size_of::<SsrCameraGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ssr_cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp ssr cam bg"),
            layout: &ssr_cam_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ssr_cam_buffer.as_entire_binding(),
            }],
        });
        let ssr_pipeline = Self::make_ssr_pipeline(device, &tex2d_bgl, &depth_bgl, &ssr_cam_bgl);
        let ssgi_cam_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("pp ssgi cam buffer"),
            size: std::mem::size_of::<SsgiCameraGpu>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let ssgi_cam_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp ssgi cam bg"),
            layout: &ssr_cam_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ssgi_cam_buffer.as_entire_binding(),
            }],
        });
        // The history's two textures and their sampler, in one group: the
        // temporal pass also reads this frame's trace, the depth with the
        // velocity, and the camera -- four groups, WebGPU's baseline.
        let filterable = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let ssgi_history_bgl = device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp ssgi history bgl"),
            entries: &[
                filterable(0),
                filterable(1),
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        // The composite's inputs: the GI targets' indirect light and
        // reflectance and the trace's depths, in one group -- with this
        // frame's trace, the screen's depth and the camera, four.
        let ssgi_composite_bgl =
            device.cached_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("pp ssgi composite bgl"),
                entries: &[
                    filterable(0),
                    filterable(1),
                    filterable(2),
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });
        let ssgi_temporal_pipeline = Self::make_targets_pipeline(
            device,
            "ssgi temporal",
            SSGI_TEMPORAL_WGSL,
            "fs_temporal",
            &[&tex2d_bgl, &ssgi_history_bgl, &depth_bgl, &ssr_cam_bgl],
            &[HDR_FORMAT, SSGI_DEPTH_FORMAT],
        );
        let ssgi_spatial_pipeline = Self::make_targets_pipeline(
            device,
            "ssgi spatial",
            SSGI_SPATIAL_WGSL,
            "fs_spatial",
            &[&tex2d_bgl, &depth_bgl, &tex2d_bgl, &ssr_cam_bgl],
            &[HDR_FORMAT],
        );
        let ssgi_pipeline = Self::make_fullscreen_pipeline(
            device,
            "ssgi",
            SSGI_WGSL,
            "fs_ssgi",
            &[&ssgi_history_bgl, &depth_bgl, &tex2d_bgl, &ssr_cam_bgl],
            Some(wgpu::BlendState::REPLACE),
            wgpu::ColorWrites::ALL,
        );
        // A plain add of a signed value: the composite's output is the
        // change to the scene, the light it replaces taken out.
        let ssgi_composite_pipeline = Self::make_fullscreen_pipeline(
            device,
            "ssgi composite",
            SSGI_COMPOSITE_WGSL,
            "fs_composite",
            &[&tex2d_bgl, &ssgi_composite_bgl, &depth_bgl, &ssr_cam_bgl],
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent::REPLACE,
            }),
            wgpu::ColorWrites::COLOR,
        );
        let dof_pipeline =
            Self::make_dof_pipeline(device, &tex2d_bgl, &depth_bgl, &ssao_cam_bgl, &dof_bgl);
        // The TAA resolve's uniform group for the camera pair: four groups is
        // the WebGPU baseline's limit, so the matrices ride in the group that
        // already carries them rather than a fifth.
        let motion_blur_pipeline = Self::make_motion_blur_pipeline(
            device,
            &tex2d_bgl,
            &depth_bgl,
            &taa_uniform_bgl,
            &motion_blur_bgl,
        );
        let composite_pipeline = Self::make_composite_pipeline(
            device,
            &tex2d_bgl,
            &composite_config_bgl,
            surface_format,
        );
        let taa_pipeline = Self::make_taa_pipeline(
            device,
            &tex2d_bgl,
            &depth_bgl,
            &taa_uniform_bgl,
            surface_format,
        );
        let fxaa_pipeline = Self::make_fxaa_pipeline(device, &tex2d_bgl, &fxaa_bgl, surface_format);

        let targets = Self::create_targets(
            device,
            width,
            height,
            depth_view,
            &sampler,
            &tex2d_bgl,
            &depth_bgl,
            &ssgi_history_bgl,
            &ssgi_composite_bgl,
            SSGI_DEFAULT_SCALE,
            surface_format,
        );

        Self {
            hdr_view: targets.hdr_view,
            _hdr_texture: targets.hdr_texture,
            fog_hdr_view: targets.fog_hdr_view,
            _fog_hdr_texture: targets.fog_hdr_texture,
            bloom_view: targets.bloom_view,
            _bloom_texture: targets.bloom_texture,
            ao_view: targets.ao_view,
            _ao_texture: targets.ao_texture,
            ao_raw_view: targets.ao_raw_view,
            _ao_raw_texture: targets.ao_raw_texture,
            ao_raw_bg: targets.ao_raw_bg,
            ldr_view: targets.ldr_view,
            fxaa_view: targets.fxaa_view,
            _fxaa_texture: targets.fxaa_texture,
            fxaa_bg: targets.fxaa_bg,
            fxaa_pipeline,
            fxaa_buffer,
            fxaa_bg_uniform,
            fxaa_on: false,
            smaa: crate::smaa::SmaaPass::new(device, surface_format),
            _ldr_texture: targets.ldr_texture,
            history_views: targets.history_views,
            _history_textures: targets.history_textures,
            history_bgs: targets.history_bgs,
            history_read: 0,
            history_valid: false,
            hdr_bg: targets.hdr_bg,
            fog_hdr_bg: targets.fog_hdr_bg,
            bloom_bg: targets.bloom_bg,
            ao_bg: targets.ao_bg,
            normal_view: targets.normal_view,
            _normal_texture: targets.normal_texture,
            normal_bg: targets.normal_bg,
            velocity_view: targets.velocity_view,
            _velocity_texture: targets.velocity_texture,
            gi_albedo_view: targets.gi_albedo_view,
            _gi_albedo_texture: targets.gi_albedo_texture,
            gi_indirect_view: targets.gi_indirect_view,
            _gi_indirect_texture: targets.gi_indirect_texture,
            gi_indirect_bg: targets.gi_indirect_bg,
            gi_albedo_bg: targets.gi_albedo_bg,
            ssgi_view: targets.ssgi_view,
            _ssgi_texture: targets.ssgi_texture,
            ssgi_bg: targets.ssgi_bg,
            ssgi_history: targets.ssgi_history,
            ssgi_current: 0,
            ssgi_history_valid: false,
            ssgi_spatial: true,
            ssgi_multibounce: true,
            ssgi_history_bgl,
            ssgi_composite_bgl,
            ssgi_scale: SSGI_DEFAULT_SCALE,
            ssgi_screen: (width, height),
            ssgi_pipeline,
            ssgi_temporal_pipeline,
            ssgi_spatial_pipeline,
            ssgi_composite_pipeline,
            ssgi_cam_buffer,
            ssgi_cam_bg,
            ssgi_enabled: false,
            ssr_view: targets.ssr_view,
            _ssr_texture: targets.ssr_texture,
            ssr_normal_view: targets.ssr_normal_view,
            _ssr_normal_texture: targets.ssr_normal_texture,
            ssr_normal_bg: targets.ssr_normal_bg,
            lit_history_texture: targets.lit_history_texture,
            lit_history_bg: targets.lit_history_bg,
            ssgi_scene_bg: targets.ssgi_scene_bg,
            lit_history_valid: false,
            ldr_bg: targets.ldr_bg,
            depth_bg: targets.depth_bg,
            sampler,
            config_buffer,
            config_bg,
            composite_config_bgl,
            composite_config_bg,
            hdr_buffer,
            hdr_scale: 0.0,
            dof_pipeline,
            dof_buffer,
            dof_bg,
            dof_on: false,
            motion_blur_pipeline,
            motion_blur_buffer,
            motion_blur_bg,
            motion_blur_on: false,
            motion_blur_prev_valid: false,
            _neutral_lut: neutral_lut,
            neutral_lut_view,
            lut: None,
            ssao_cam_buffer,
            ssao_cam_bg,
            taa_cam_buffer,
            taa_uniform_bg,
            _injection_volumes: injection_volumes,
            _integrated_volume: integrated_volume,
            fog_uniform_buffer,
            fog_uniform_bg,
            froxel_shadow_bg,
            injection_write_bgs,
            injection_read_bgs,
            injection_history_bgs,
            fog_history_read: 0,
            fog_history_valid: false,
            integrated_write_bg,
            integrated_read_bg,
            fog_enabled: false,
            fog_inject_pipeline,
            fog_integrate_pipeline,
            fog_pipeline,
            bloom_pipeline,
            ssao_pipeline,
            ssao_blur_pipeline,
            ssr_pipeline,
            ssr_cam_buffer,
            ssr_cam_bg,
            ssr_enabled: false,
            composite_pipeline,
            taa_pipeline,
            tex2d_bgl,
            depth_bgl,
            surface_format,
        }
    }

    /// Screen-space GI's targets for a screen of `screen` at `scale`: the
    /// trace's, and the history pair's, each a `scale`-th of the screen each
    /// way (rounded up, so the last trace pixel covers the last screen one).
    #[allow(clippy::too_many_arguments)]
    fn create_ssgi_targets(
        device: &wgpu::Device,
        screen: (u32, u32),
        scale: u32,
        sampler: &wgpu::Sampler,
        tex2d_bgl: &wgpu::BindGroupLayout,
        ssgi_history_bgl: &wgpu::BindGroupLayout,
        ssgi_composite_bgl: &wgpu::BindGroupLayout,
        gi: (&wgpu::TextureView, &wgpu::TextureView),
    ) -> SsgiTargets {
        let (width, height) = (screen.0.div_ceil(scale), screen.1.div_ceil(scale));
        let make_tex = |label: &str, fmt: wgpu::TextureFormat| {
            crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: fmt,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };
        let make_tex2d_bg = |label: &str, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: tex2d_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };
        let ssgi_texture = make_tex("pp ssgi", HDR_FORMAT);
        let ssgi_view = ssgi_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ssgi_bg = make_tex2d_bg("pp ssgi bg", &ssgi_view);
        let ssgi_history = [0, 1].map(|i| {
            let light = make_tex(&format!("pp ssgi history {i}"), HDR_FORMAT);
            let light_view = light.create_view(&wgpu::TextureViewDescriptor::default());
            let depth = make_tex(&format!("pp ssgi history depth {i}"), SSGI_DEPTH_FORMAT);
            let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
            let light_bg = make_tex2d_bg("pp ssgi history light bg", &light_view);
            let pair_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pp ssgi history bg"),
                layout: ssgi_history_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&light_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            let composite_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("pp ssgi composite bg"),
                layout: ssgi_composite_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(gi.0),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(gi.1),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&depth_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            });
            SsgiHistory {
                composite_bg,
                _light: light,
                light_view,
                light_bg,
                _depth: depth,
                depth_view,
                pair_bg,
            }
        });
        SsgiTargets {
            texture: ssgi_texture,
            view: ssgi_view,
            bg: ssgi_bg,
            history: ssgi_history,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn create_targets(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        depth_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        ssgi_history_bgl: &wgpu::BindGroupLayout,
        ssgi_composite_bgl: &wgpu::BindGroupLayout,
        ssgi_scale: u32,
        surface_format: wgpu::TextureFormat,
    ) -> PostProcessTargets {
        let make_tex = |label: &str, fmt: wgpu::TextureFormat| {
            crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: fmt,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            )
        };

        // Copyable: the reflections read last frame's lit scene from a copy
        // of it (`lit_history`).
        let hdr_texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("pp hdr"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );
        let hdr_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Same format and size as `hdr_texture`: the fog pass copies the scene
        // image through it, so anything narrower would requantize the frame.
        let fog_hdr_texture = make_tex("pp fog hdr", HDR_FORMAT);
        let fog_hdr_view = fog_hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_texture = make_tex("pp bloom", BLOOM_FORMAT);
        let bloom_view = bloom_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ao_texture = make_tex("pp ao", AO_FORMAT);
        let ao_view = ao_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ao_raw_texture = make_tex("pp ao raw", AO_FORMAT);
        let ao_raw_view = ao_raw_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let normal_texture = make_tex("pp normal", NORMAL_FORMAT);
        let normal_view = normal_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Copyable as well, for `WgpuSurface::read_velocity`: the post passes
        // are its only readers, so a test has no other way to see it.
        let velocity_texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("pp velocity"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: VELOCITY_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING
                    | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            },
        );
        let velocity_view = velocity_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Copyable for the same reason as the velocity: the tests' only view
        // of them until screen-space GI reads them.
        let gi_target = |label: &str, format: wgpu::TextureFormat| {
            crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some(label),
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                },
            )
        };
        let gi_albedo_texture = gi_target("pp gi albedo", GI_ALBEDO_FORMAT);
        let gi_albedo_view = gi_albedo_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let gi_indirect_texture = gi_target("pp gi indirect", GI_INDIRECT_FORMAT);
        let gi_indirect_view =
            gi_indirect_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ssr_texture = make_tex("pp ssr", HDR_FORMAT);
        let ssr_view = ssr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ssr_normal_texture = make_tex("pp ssr normal", NORMAL_FORMAT);
        let ssr_normal_view =
            ssr_normal_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let lit_history_texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("pp lit history"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: HDR_FORMAT,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
        );
        let lit_history_view =
            lit_history_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // `surface_format`, not HDR_FORMAT: this holds the already-tonemapped
        // LDR image on its way to the swapchain, so matching the swapchain's
        // format keeps the round trip through it exact.
        let ldr_texture = make_tex("pp ldr", surface_format);
        let ldr_view = ldr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // FXAA's output, in the same format for the same reason.
        let fxaa_texture = make_tex("pp fxaa", surface_format);
        let fxaa_view = fxaa_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // The TAA history ping-pong pair. Both hold a finished LDR frame, so
        // like the LDR target they use `surface_format` at the full surface
        // size; a pair rather than one texture because the resolve pass has
        // to sample last frame's history while rendering this frame's.
        let history_textures = [
            make_tex("pp history 0", surface_format),
            make_tex("pp history 1", surface_format),
        ];
        let history_views = [
            history_textures[0].create_view(&wgpu::TextureViewDescriptor::default()),
            history_textures[1].create_view(&wgpu::TextureViewDescriptor::default()),
        ];

        let make_tex2d_bg = |label: &str, view: &wgpu::TextureView| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: tex2d_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };

        let hdr_bg = make_tex2d_bg("pp hdr bg", &hdr_view);
        let fog_hdr_bg = make_tex2d_bg("pp fog hdr bg", &fog_hdr_view);
        let bloom_bg = make_tex2d_bg("pp bloom bg", &bloom_view);
        let ao_bg = make_tex2d_bg("pp ao bg", &ao_view);
        let ao_raw_bg = make_tex2d_bg("pp ao raw bg", &ao_raw_view);
        let normal_bg = make_tex2d_bg("pp normal bg", &normal_view);
        let ssr_normal_bg = make_tex2d_bg("pp ssr normal bg", &ssr_normal_view);
        let lit_history_bg = make_tex2d_bg("pp lit history bg", &lit_history_view);
        // This frame's lit scene and last frame's, for the GI trace.
        let ssgi_scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp ssgi scene bg"),
            layout: ssgi_history_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&hdr_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&lit_history_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });
        let ssgi = Self::create_ssgi_targets(
            device,
            (width, height),
            ssgi_scale,
            sampler,
            tex2d_bgl,
            ssgi_history_bgl,
            ssgi_composite_bgl,
            (&gi_indirect_view, &gi_albedo_view),
        );
        let gi_indirect_bg = make_tex2d_bg("pp gi indirect bg", &gi_indirect_view);
        let gi_albedo_bg = make_tex2d_bg("pp gi albedo bg", &gi_albedo_view);
        let ldr_bg = make_tex2d_bg("pp ldr bg", &ldr_view);
        let fxaa_bg = make_tex2d_bg("pp fxaa bg", &fxaa_view);
        let history_bgs = [
            make_tex2d_bg("pp history bg 0", &history_views[0]),
            make_tex2d_bg("pp history bg 1", &history_views[1]),
        ];

        let depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp depth bg"),
            layout: depth_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(depth_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&velocity_view),
                },
            ],
        });

        PostProcessTargets {
            hdr_texture,
            hdr_view,
            hdr_bg,
            fog_hdr_texture,
            fog_hdr_view,
            fog_hdr_bg,
            bloom_texture,
            bloom_view,
            bloom_bg,
            ao_texture,
            ao_view,
            ao_bg,
            ao_raw_texture,
            ao_raw_view,
            ao_raw_bg,
            normal_texture,
            normal_view,
            normal_bg,
            velocity_texture,
            velocity_view,
            gi_albedo_texture,
            gi_albedo_view,
            gi_indirect_texture,
            gi_indirect_view,
            gi_indirect_bg,
            gi_albedo_bg,
            ssgi_texture: ssgi.texture,
            ssgi_view: ssgi.view,
            ssgi_bg: ssgi.bg,
            ssgi_history: ssgi.history,
            ssr_texture,
            ssr_view,
            ssr_normal_texture,
            ssr_normal_view,
            ssr_normal_bg,
            lit_history_texture,
            lit_history_bg,
            ssgi_scene_bg,
            ldr_texture,
            ldr_view,
            ldr_bg,
            fxaa_texture,
            fxaa_view,
            fxaa_bg,
            history_textures,
            history_views,
            history_bgs,
            depth_bg,
        }
    }

    /// The froxel injection pass: the engine's first compute pipeline.
    ///
    /// Four groups: the fog parameters, the volume it writes, the scene's shadow
    /// resources it tests each froxel against, and the previous frame's volume
    /// it accumulates into. Four is the WebGPU baseline `max_bind_groups`, so
    /// this pass has no room for a fifth.
    fn make_fog_inject_pipeline(
        device: &wgpu::Device,
        fog_uniform_bgl: &wgpu::BindGroupLayout,
        froxel_write_bgl: &wgpu::BindGroupLayout,
        froxel_shadow_bgl: &wgpu::BindGroupLayout,
        froxel_history_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::ComputePipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fog inject shader"),
            source: wgpu::ShaderSource::Wgsl(fog_inject_wgsl().into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fog inject pll"),
            bind_group_layouts: &[
                fog_uniform_bgl,
                froxel_write_bgl,
                froxel_shadow_bgl,
                froxel_history_bgl,
            ],
            push_constant_ranges: &[],
        });
        device.cached_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fog inject pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: "cs_inject",
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// The froxel integration pass. Reads the injection volume as a sampled
    /// texture and writes the integrated one as storage; the same texture
    /// cannot be both in one binding on the WebGPU baseline.
    fn make_fog_integrate_pipeline(
        device: &wgpu::Device,
        fog_uniform_bgl: &wgpu::BindGroupLayout,
        froxel_read_bgl: &wgpu::BindGroupLayout,
        froxel_write_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::ComputePipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fog integrate shader"),
            source: wgpu::ShaderSource::Wgsl(fog_integrate_wgsl().into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fog integrate pll"),
            bind_group_layouts: &[fog_uniform_bgl, froxel_read_bgl, froxel_write_bgl],
            push_constant_ranges: &[],
        });
        device.cached_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("fog integrate pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: "cs_integrate",
            compilation_options: Default::default(),
            cache: None,
        })
    }

    /// The fog apply pass renders HDR into HDR, so its colour target is
    /// `HDR_FORMAT` rather than the swapchain format the later passes use.
    ///
    /// Four bind groups -- scene colour, depth, the integrated volume, the fog
    /// parameters -- which is the WebGPU baseline `max_bind_groups`, the same
    /// ceiling the TAA resolve sits at.
    fn make_fog_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        froxel_apply_bgl: &wgpu::BindGroupLayout,
        fog_uniform_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fog shader"),
            source: wgpu::ShaderSource::Wgsl(fog_apply_wgsl().into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fog pll"),
            bind_group_layouts: &[tex2d_bgl, depth_bgl, froxel_apply_bgl, fog_uniform_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fog pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_fog",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_bloom_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom shader"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom pll"),
            bind_group_layouts: &[tex2d_bgl, config_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("bloom pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_bloom",
                targets: &[Some(wgpu::ColorTargetState {
                    format: BLOOM_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// The reflection trace. Groups: scene colour, depth, normal+roughness,
    /// camera -- the normal buffer reuses `tex2d_bgl` because it is a plain
    /// sampled 2D texture like every other post-process input.
    /// A full-screen pass, unblended, into `formats`: the SSGI denoiser's.
    fn make_targets_pipeline(
        device: &wgpu::Device,
        label: &str,
        source: &str,
        entry_point: &str,
        groups: &[&wgpu::BindGroupLayout],
        formats: &[wgpu::TextureFormat],
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: groups,
            push_constant_ranges: &[],
        });
        let targets: Vec<Option<wgpu::ColorTargetState>> = formats
            .iter()
            .map(|&format| {
                Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })
            })
            .collect();
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point,
                targets: &targets,
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// A full-screen pass into one HDR target: screen-space GI's two.
    #[allow(clippy::too_many_arguments)] // one pass's whole description
    fn make_fullscreen_pipeline(
        device: &wgpu::Device,
        label: &str,
        source: &str,
        entry_point: &str,
        groups: &[&wgpu::BindGroupLayout],
        blend: Option<wgpu::BlendState>,
        write_mask: wgpu::ColorWrites,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(label),
            bind_group_layouts: groups,
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some(label),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point,
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend,
                    write_mask,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_ssr_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        ssr_cam_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssr shader"),
            source: wgpu::ShaderSource::Wgsl(SSR_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssr pll"),
            bind_group_layouts: &[tex2d_bgl, depth_bgl, tex2d_bgl, ssr_cam_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ssr pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_ssr",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_ssao_pipeline(
        device: &wgpu::Device,
        depth_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
        ssao_cam_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssao shader"),
            source: wgpu::ShaderSource::Wgsl(SSAO_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssao pll"),
            bind_group_layouts: &[depth_bgl, config_bgl, ssao_cam_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ssao pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_ssao",
                targets: &[Some(wgpu::ColorTargetState {
                    format: AO_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_ssao_blur_pipeline(
        device: &wgpu::Device,
        depth_bgl: &wgpu::BindGroupLayout,
        tex2d_bgl: &wgpu::BindGroupLayout,
        ssao_cam_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssao blur shader"),
            source: wgpu::ShaderSource::Wgsl(SSAO_BLUR_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssao blur pll"),
            bind_group_layouts: &[depth_bgl, tex2d_bgl, ssao_cam_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ssao blur pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_ssao_blur",
                targets: &[Some(wgpu::ColorTargetState {
                    format: AO_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_dof_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        ssao_cam_bgl: &wgpu::BindGroupLayout,
        dof_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("dof shader"),
            source: wgpu::ShaderSource::Wgsl(DOF_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("dof pll"),
            bind_group_layouts: &[tex2d_bgl, depth_bgl, ssao_cam_bgl, dof_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("dof pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_dof",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_fxaa_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        fxaa_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fxaa shader"),
            source: wgpu::ShaderSource::Wgsl(FXAA_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fxaa pll"),
            bind_group_layouts: &[tex2d_bgl, fxaa_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("fxaa pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_fxaa",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_motion_blur_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        taa_uniform_bgl: &wgpu::BindGroupLayout,
        motion_blur_bgl: &wgpu::BindGroupLayout,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("motion blur shader"),
            source: wgpu::ShaderSource::Wgsl(MOTION_BLUR_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("motion blur pll"),
            bind_group_layouts: &[tex2d_bgl, depth_bgl, taa_uniform_bgl, motion_blur_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("motion blur pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_motion_blur",
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR_FORMAT,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    fn make_composite_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite pll"),
            bind_group_layouts: &[tex2d_bgl, tex2d_bgl, tex2d_bgl, config_bgl],
            push_constant_ranges: &[],
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("composite pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_composite",
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// Two color targets, both `surface_format`: `@location(0)` is the
    /// swapchain and `@location(1)` is next frame's history. Writing both in
    /// one pass is why the history pair can hold exactly the image that was
    /// presented, with no extra copy.
    fn make_taa_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        depth_bgl: &wgpu::BindGroupLayout,
        taa_uniform_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> std::sync::Arc<wgpu::RenderPipeline> {
        let shader = device.cached_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taa shader"),
            source: wgpu::ShaderSource::Wgsl(TAA_WGSL.into()),
        });
        let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taa pll"),
            bind_group_layouts: &[tex2d_bgl, tex2d_bgl, depth_bgl, taa_uniform_bgl],
            push_constant_ranges: &[],
        });
        let color_target = Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        });
        device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("taa pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_fullscreen",
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_taa",
                targets: &[color_target.clone(), color_target],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        })
    }

    /// Recreates the sized render targets (HDR/fog HDR/bloom/AO/LDR/history and
    /// the depth bind group) for a new surface resolution, leaving pipelines and
    /// samplers untouched.
    pub fn resize_targets(
        &mut self,
        device: &wgpu::Device,
        depth_view: &wgpu::TextureView,
        width: u32,
        height: u32,
    ) {
        let t = Self::create_targets(
            device,
            width,
            height,
            depth_view,
            &self.sampler,
            &self.tex2d_bgl,
            &self.depth_bgl,
            &self.ssgi_history_bgl,
            &self.ssgi_composite_bgl,
            self.ssgi_scale,
            self.surface_format,
        );
        self.ssgi_screen = (width, height);
        self.hdr_view = t.hdr_view;
        self._hdr_texture = t.hdr_texture;
        self.fog_hdr_view = t.fog_hdr_view;
        self._fog_hdr_texture = t.fog_hdr_texture;
        self.bloom_view = t.bloom_view;
        self._bloom_texture = t.bloom_texture;
        self.ao_view = t.ao_view;
        self._ao_texture = t.ao_texture;
        self.ao_raw_view = t.ao_raw_view;
        self._ao_raw_texture = t.ao_raw_texture;
        self.ao_raw_bg = t.ao_raw_bg;
        self.normal_view = t.normal_view;
        self._normal_texture = t.normal_texture;
        self.normal_bg = t.normal_bg;
        self.velocity_view = t.velocity_view;
        self._velocity_texture = t.velocity_texture;
        self.gi_albedo_view = t.gi_albedo_view;
        self._gi_albedo_texture = t.gi_albedo_texture;
        self.gi_indirect_view = t.gi_indirect_view;
        self._gi_indirect_texture = t.gi_indirect_texture;
        self.gi_indirect_bg = t.gi_indirect_bg;
        self.gi_albedo_bg = t.gi_albedo_bg;
        self.ssgi_view = t.ssgi_view;
        self._ssgi_texture = t.ssgi_texture;
        self.ssgi_bg = t.ssgi_bg;
        self.ssgi_history = t.ssgi_history;
        // New textures hold nothing: the next frame starts the history over.
        self.ssgi_history_valid = false;
        self.ssr_view = t.ssr_view;
        self._ssr_texture = t.ssr_texture;
        self.ssr_normal_view = t.ssr_normal_view;
        self._ssr_normal_texture = t.ssr_normal_texture;
        self.ssr_normal_bg = t.ssr_normal_bg;
        self.lit_history_texture = t.lit_history_texture;
        self.lit_history_bg = t.lit_history_bg;
        self.ssgi_scene_bg = t.ssgi_scene_bg;
        // A new texture holds no frame: no reflection until one is copied.
        self.lit_history_valid = false;
        self.ldr_view = t.ldr_view;
        self._ldr_texture = t.ldr_texture;
        self.fxaa_view = t.fxaa_view;
        self._fxaa_texture = t.fxaa_texture;
        self.fxaa_bg = t.fxaa_bg;
        self.history_views = t.history_views;
        self._history_textures = t.history_textures;
        self.history_bgs = t.history_bgs;
        // The freshly created history pair is uninitialised at the new
        // resolution, so nothing in it may be blended against.
        self.invalidate_history();
        self.hdr_bg = t.hdr_bg;
        self.fog_hdr_bg = t.fog_hdr_bg;
        self.bloom_bg = t.bloom_bg;
        self.ao_bg = t.ao_bg;
        self.ldr_bg = t.ldr_bg;
        self.depth_bg = t.depth_bg;
    }

    /// Discards both accumulated histories -- the TAA resolve's and the froxel
    /// fog's -- so the next frame stands alone instead of blending against a
    /// stale or uninitialised one.
    ///
    /// Anything that makes those textures stop describing the frame that comes
    /// next has to call this -- a resize above being the standing example, since
    /// the reallocated history pair holds garbage at the new resolution. The fog
    /// volumes survive a resize untouched, being fixed-resolution, but their
    /// contents were reprojected through the old camera and a resize changes the
    /// aspect ratio, so they are equally stale.
    pub fn invalidate_history(&mut self) {
        self.history_valid = false;
        self.fog_history_valid = false;
        // A cut for motion blur too: the next frame's previous camera is not
        // one it moved from.
        self.motion_blur_prev_valid = false;
    }

    /// Uploads new bloom/tonemap/SSAO settings to the config uniform buffer.
    pub fn update_config(&self, queue: &wgpu::Queue, config: PostProcessConfigGpu) {
        queue.write_buffer(&self.config_buffer, 0, bytemuck::cast_slice(&[config]));
    }

    /// Sets this frame's depth of field; `None`, or one that is disabled or
    /// blurs neither band, skips the pass entirely -- the frame is then the
    /// fogged image exactly as before depth of field existed.
    pub fn update_dof(&mut self, queue: &wgpu::Queue, dof: Option<bsengine_core::DepthOfField>) {
        let active =
            dof.filter(|d| d.enabled && (d.far_enabled || d.near_enabled) && d.max_radius >= 0.5);
        self.dof_on = active.is_some();
        if let Some(d) = active {
            let data = DofGpu {
                near_distance: d.near_distance,
                near_transition: d.near_transition,
                far_distance: d.far_distance,
                far_transition: d.far_transition,
                max_radius: d.max_radius,
                near_enabled: d.near_enabled as u32,
                far_enabled: d.far_enabled as u32,
                _pad: 0,
            };
            queue.write_buffer(&self.dof_buffer, 0, bytemuck::bytes_of(&data));
        }
    }

    /// Whether this frame's depth-of-field pass will run (before
    /// `fast_render`, which skips it regardless).
    pub fn dof_active(&self) -> bool {
        self.dof_on
    }

    /// Sets this frame's motion blur; `None`, or one that is disabled or has
    /// no intensity, skips the pass -- the frame is then exactly what it was
    /// before motion blur existed.
    ///
    /// Called once per frame, blur or not, because it is also what moves the
    /// "is last frame's camera one we continued from" flag forward: the
    /// camera pair is uploaded every frame whatever the settings, so turning
    /// blur on mid-run measures against the frame that really preceded it.
    pub fn update_motion_blur(
        &mut self,
        queue: &wgpu::Queue,
        blur: Option<bsengine_core::MotionBlur>,
    ) {
        let active = blur.filter(|b| b.enabled && b.intensity > 0.0 && b.max_blur > 0.0);
        self.motion_blur_on = active.is_some();
        if let Some(b) = active {
            let data = MotionBlurGpu {
                intensity: b.intensity,
                max_blur: b.max_blur,
                samples: b.clamped_samples(),
                prev_valid: self.motion_blur_prev_valid as u32,
            };
            queue.write_buffer(&self.motion_blur_buffer, 0, bytemuck::bytes_of(&data));
        }
        // This frame becomes the next one's previous camera.
        self.motion_blur_prev_valid = true;
    }

    /// Whether this frame's motion blur pass will run (before `fast_render`,
    /// which skips it regardless).
    pub fn motion_blur_active(&self) -> bool {
        self.motion_blur_on
    }

    /// Sets this frame's FXAA; `None`, or a disabled one, skips the pass --
    /// the TAA resolve then reads the composite's output exactly as before
    /// FXAA existed.
    pub fn update_fxaa(&mut self, queue: &wgpu::Queue, fxaa: Option<bsengine_core::Fxaa>) {
        let active = fxaa.filter(|f| f.enabled);
        self.fxaa_on = active.is_some();
        if let Some(f) = active {
            let data = FxaaGpu {
                edge_threshold: f.edge_threshold,
                edge_threshold_min: f.edge_threshold_min,
                subpixel: f.subpixel.clamp(0.0, 1.0),
                // Sampling an sRGB target decodes to linear light; FXAA's
                // thresholds are for perceptual luma, so the shader encodes
                // it back (approximately, by a square root) first.
                srgb_input: self.surface_format.is_srgb() as u32,
                hdr_scale: self.hdr_scale,
                _pad: [0.0; 3],
            };
            queue.write_buffer(&self.fxaa_buffer, 0, bytemuck::bytes_of(&data));
        }
    }

    /// Whether this frame's FXAA pass will run (before `fast_render`, which
    /// skips it regardless). False while SMAA runs, which replaces it.
    pub fn fxaa_active(&self) -> bool {
        self.fxaa_on && !self.smaa.active()
    }

    /// Sets this frame's SMAA; `None`, or a disabled one, skips its passes.
    /// Takes the device because SMAA's tables and targets are made the first
    /// frame they are needed, and its bind groups over the LDR target every
    /// frame it runs (so a resize can never leave it reading the old one).
    pub fn update_smaa(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        smaa: Option<bsengine_core::Smaa>,
    ) {
        let size = (self._ldr_texture.width(), self._ldr_texture.height());
        self.smaa.prepare(
            device,
            queue,
            smaa,
            &self.ldr_view,
            size,
            self.surface_format,
            self.hdr_scale,
            &self.tex2d_bgl,
            &self.sampler,
        );
    }

    /// Switches the composite to HDR output -- linear scRGB into a float
    /// target -- or back off; FXAA and SMAA follow, normalising luma by SDR
    /// white. Set once, when the surface's output format is chosen.
    pub fn set_hdr_output(&mut self, queue: &wgpu::Queue, hdr: HdrOutputGpu) {
        queue.write_buffer(&self.hdr_buffer, 0, bytemuck::bytes_of(&hdr));
        self.hdr_scale = if hdr.enabled != 0 { hdr.scale } else { 0.0 };
    }

    /// The velocity buffer's texture, for `WgpuSurface::read_velocity`.
    pub(crate) fn velocity_texture(&self) -> &wgpu::Texture {
        &self._velocity_texture
    }

    /// The GI albedo and indirect textures, for `WgpuSurface::read_gi`.
    pub(crate) fn gi_textures(&self) -> (&wgpu::Texture, &wgpu::Texture) {
        (&self._gi_albedo_texture, &self._gi_indirect_texture)
    }

    /// Whether this frame's SMAA passes will run (before `fast_render`, which
    /// skips them regardless).
    pub fn smaa_active(&self) -> bool {
        self.smaa.active()
    }

    /// Slices in the bound colour LUT, or 0 for none.
    pub fn lut_size(&self) -> u32 {
        self.lut.as_ref().map(|(_, n)| *n).unwrap_or(0)
    }

    /// Binds `source` as the colour LUT, copying its first mip into a texture
    /// of the post pass's own; `None` unbinds it.
    ///
    /// The copy keeps the source's format and is *read* through a
    /// [`LUT_FORMAT`] view, the linear reading of the same bytes: a strip is
    /// stored as an sRGB image, and sampling it through an sRGB view would
    /// decode its texels to linear, while the grade reads them as the
    /// display values they are. Not a copy *into* the linear format: wgpu
    /// allows copies that differ only in sRGB-ness, but Metal's left the
    /// destination empty.
    ///
    /// Refused, leaving the previous LUT bound, for a source that is not a
    /// strip (width = height squared, at least 2 slices) or not 8-bit RGBA --
    /// a block-compressed texture cannot be copied into an uncompressed one,
    /// and would have lost the precision a LUT exists to hold anyway.
    pub fn set_lut(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        source: Option<&wgpu::Texture>,
    ) -> Result<(), String> {
        let Some(source) = source else {
            self.lut = None;
            self.composite_config_bg = make_composite_config_bg(
                device,
                &self.composite_config_bgl,
                &self.config_buffer,
                &self.neutral_lut_view,
                &self.sampler,
                &self.hdr_buffer,
            );
            return Ok(());
        };
        let (w, h) = (source.width(), source.height());
        if h < 2 || w != h * h {
            return Err(format!(
                "a LUT must be a strip of N slices of N x N (width = height squared), got {w} x {h}"
            ));
        }
        if !matches!(
            source.format(),
            wgpu::TextureFormat::Rgba8UnormSrgb | wgpu::TextureFormat::Rgba8Unorm
        ) {
            return Err(format!(
                "a LUT must be uncompressed 8-bit RGBA, got {:?}",
                source.format()
            ));
        }
        let texture = crate::profiler::create_tracked_texture(
            device,
            &wgpu::TextureDescriptor {
                label: Some("pp colour lut"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                // The source's own format, so the copy is between identical
                // formats: Metal's blit copy between an sRGB and a linear
                // format left the destination empty (the LUT read black on
                // macOS CI while Vulkan and D3D12 copied the bytes).
                format: source.format(),
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                // ... and the linear reading comes from a *view* instead:
                // WebGPU's sanctioned way to reinterpret sRGB-ness.
                view_formats: &[LUT_FORMAT],
            },
        );
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("pp lut copy"),
        });
        encoder.copy_texture_to_texture(
            source.as_image_copy(),
            texture.as_image_copy(),
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(LUT_FORMAT),
            ..Default::default()
        });
        self.composite_config_bg = make_composite_config_bg(
            device,
            &self.composite_config_bgl,
            &self.config_buffer,
            &view,
            &self.sampler,
            &self.hdr_buffer,
        );
        self.lut = Some((texture, h));
        Ok(())
    }

    /// Switches screen-space GI's spatial filter, which always runs, for the
    /// test that measures it. Off, the composite reads this frame's trace
    /// unfiltered (the history still accumulates it).
    #[doc(hidden)]
    pub fn set_ssgi_spatial_for_testing(&mut self, on: bool) {
        self.ssgi_spatial = on;
    }

    /// Traces screen-space GI at full resolution or at half: on a change,
    /// its targets are made anew at the other size, and the history starts
    /// over.
    pub fn set_ssgi_full_resolution(&mut self, device: &wgpu::Device, full: bool) {
        let scale = if full { 1 } else { 2 };
        if scale == self.ssgi_scale {
            return;
        }
        let t = Self::create_ssgi_targets(
            device,
            self.ssgi_screen,
            scale,
            &self.sampler,
            &self.tex2d_bgl,
            &self.ssgi_history_bgl,
            &self.ssgi_composite_bgl,
            (&self.gi_indirect_view, &self.gi_albedo_view),
        );
        self.ssgi_scale = scale;
        self._ssgi_texture = t.texture;
        self.ssgi_view = t.view;
        self.ssgi_bg = t.bg;
        self.ssgi_history = t.history;
        self.ssgi_history_valid = false;
    }

    /// The size screen-space GI traces at, for the tests that it is a
    /// quarter of the screen's pixels.
    #[doc(hidden)]
    pub fn ssgi_trace_size(&self) -> (u32, u32) {
        (
            self.ssgi_screen.0.div_ceil(self.ssgi_scale),
            self.ssgi_screen.1.div_ceil(self.ssgi_scale),
        )
    }

    /// Switches screen-space GI's further bounces, which are always on, for
    /// the tests that compare against a renderer with no last frame. Off,
    /// every hit's light is this frame's: one bounce.
    #[doc(hidden)]
    pub fn set_ssgi_multibounce_for_testing(&mut self, on: bool) {
        self.ssgi_multibounce = on;
    }

    /// Uploads screen-space GI's camera and trace; an intensity of zero
    /// switches both its passes off.
    pub fn update_ssgi_camera(&mut self, queue: &wgpu::Queue, mut cam: SsgiCameraGpu) {
        self.ssgi_enabled = cam.params2[0] > 0.0;
        // A frame without the trace leaves the history behind: switched back
        // on, it starts over rather than blending into a stale frame.
        if !self.ssgi_enabled {
            self.ssgi_history_valid = false;
        }
        cam.params2[1] = SSGI_HISTORY_BLEND;
        cam.params2[3] = self.ssgi_scale as f32;
        cam.misc[0] = if self.lit_history_valid && self.ssgi_multibounce {
            1.0
        } else {
            0.0
        };
        cam.params2[2] = if self.ssgi_history_valid { 1.0 } else { 0.0 };
        queue.write_buffer(&self.ssgi_cam_buffer, 0, bytemuck::bytes_of(&cam));
    }

    /// Uploads what the reflection pass needs. Intensity zero switches it off.
    pub fn update_ssr_camera(&mut self, queue: &wgpu::Queue, mut cam: SsrCameraGpu) {
        // Kept beside the buffer rather than read back from it: the surface
        // has to decide whether to run the prepass and the trace at all, and
        // a zero-intensity trace that still marches every pixel is the cost
        // without the effect.
        self.ssr_enabled = cam.params[3] > 0.0;
        cam.misc[0] = if self.lit_history_valid { 1.0 } else { 0.0 };
        queue.write_buffer(&self.ssr_cam_buffer, 0, bytemuck::bytes_of(&cam));
    }

    /// Whether this frame traces reflections: the surface runs the prepass
    /// they trace from only then.
    pub fn ssr_enabled(&self) -> bool {
        self.ssr_enabled
    }

    /// Traces this frame's reflections into [`Self::ssr_view`], before the
    /// opaque pass reads them -- from the prepass's depth and normals
    /// ([`Self::ssr_normal_view`]) and last frame's lit scene -- or clears it
    /// to "nothing reflected" when the camera has none: the mesh shader reads
    /// it every frame, and a stale trace would stay on screen.
    pub fn trace_ssr(&mut self, encoder: &mut wgpu::CommandEncoder, fast_render: bool) -> u32 {
        let trace = self.ssr_enabled && !fast_render;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(if trace { "ssr pass" } else { "ssr clear" }),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &self.ssr_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });
        if !trace {
            return 0;
        }
        pass.set_pipeline(&self.ssr_pipeline);
        pass.set_bind_group(0, &self.lit_history_bg, &[]);
        pass.set_bind_group(1, &self.depth_bg, &[]);
        pass.set_bind_group(2, &self.ssr_normal_bg, &[]);
        pass.set_bind_group(3, &self.ssr_cam_bg, &[]);
        pass.draw(0..3, 0..1);
        1
    }

    /// Uploads the current frame's camera projection matrices for SSAO depth reconstruction.
    pub fn update_ssao_camera(&self, queue: &wgpu::Queue, cam: SsaoCameraGpu) {
        queue.write_buffer(&self.ssao_cam_buffer, 0, bytemuck::cast_slice(&[cam]));
    }

    /// Uploads the froxel passes' parameters, and records whether those passes
    /// should run at all.
    ///
    /// Takes `&mut self`, unlike the uploads either side of it, because the
    /// enabled flag has to stay readable on the CPU: `apply` skips the whole
    /// dispatch when fog is off, and it cannot read the flag back out of the
    /// uniform buffer.
    ///
    /// [`FogUniform::history_valid`] is stamped here rather than taken from the
    /// caller. Only `apply` advances the scattering ping-pong, so only this
    /// struct knows whether there is anything in it worth blending against;
    /// letting a caller answer that question would let it ask the injection pass
    /// to average this frame with an uninitialised volume.
    pub fn update_fog(&mut self, queue: &wgpu::Queue, fog: FogUniform) {
        self.fog_enabled = fog.enabled != 0;
        let fog = FogUniform {
            history_valid: u32::from(self.fog_history_valid),
            ..fog
        };
        queue.write_buffer(&self.fog_uniform_buffer, 0, bytemuck::cast_slice(&[fog]));
    }

    /// Uploads the unjittered view-projection matrices the TAA pass reprojects
    /// with: this frame's inverse and the previous frame's forward matrix.
    pub fn update_taa_camera(&self, queue: &wgpu::Queue, cam: TaaCameraGpu) {
        queue.write_buffer(&self.taa_cam_buffer, 0, bytemuck::cast_slice(&[cam]));
    }

    /// Runs the fog, bloom, SSAO, composite, and TAA passes in sequence,
    /// writing the final tonemapped result into `surface_view`.
    ///
    /// When the last [`PostProcessState::update_fog`] enabled the effect, a
    /// compute pass runs first and fills the two froxel volumes; the apply
    /// pass then samples the integrated one.
    ///
    /// The fog pass comes first and writes the scene's HDR image into a second
    /// HDR target that every later pass samples, because a pass cannot sample
    /// the texture it renders into. With fog disabled it is an exact
    /// passthrough, so the image reaching `surface_view` is the one the chain
    /// produced when bloom and composite read the scene target directly.
    ///
    /// Composite writes into an intermediate LDR target rather than straight
    /// into `surface_view`; the TAA pass then reads that target and writes both
    /// the swapchain and next frame's history target. With `taa_enabled` zero
    /// the resolve returns the composite result untouched, so the image
    /// reaching `surface_view` is the same one composite used to write there
    /// directly.
    ///
    /// Takes `&mut self` because the resolve advances the history ping-pong:
    /// the target just written becomes next frame's read source.
    ///
    /// `fast_render` skips the bloom/SSAO shading work but still clears both
    /// targets to their neutral values (black = "no bloom contribution",
    /// white = "fully visible") -- see `WgpuSurface::is_fast_render`. Never
    /// skip the clear itself: the composite pass below always samples both
    /// textures, and an un-cleared AO texture reads as 0.0 ("fully
    /// occluded") instead of 1.0, which would render every pixel black.
    ///
    /// Returns the `(draw_calls, triangles)` issued by this call, so the
    /// caller can fold them into its own per-frame counters.
    pub fn apply(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        surface_view: &wgpu::TextureView,
        fast_render: bool,
    ) -> (u32, u64) {
        let mut draw_calls = 0u32;
        let mut triangles = 0u64;

        // Reflections first, before fog: what they change is the surface's own
        // shading, and fog is atmosphere in front of it. Folding them in after
        // the fog would put a reflection on top of the haze that is supposed to
        // be hiding it.
        //
        // Two passes because the trace samples the scene it is then added to,
        // and a pass may not sample the texture it renders to -- the same
        // reason the fog pass has a target of its own.
        // Screen-space GI first: the diffuse light it changes is part of the
        // surface the reflections then show. Trace, then composite -- the
        // trace samples the scene the composite writes to.
        if self.ssgi_enabled && !fast_render {
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssgi pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.ssgi_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                pass.set_pipeline(&self.ssgi_pipeline);
                pass.set_bind_group(0, &self.ssgi_scene_bg, &[]);
                pass.set_bind_group(1, &self.depth_bg, &[]);
                pass.set_bind_group(2, &self.normal_bg, &[]);
                pass.set_bind_group(3, &self.ssgi_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
            // Into this frame's half of the history, from last frame's.
            let (read, write) = (self.ssgi_current, 1 - self.ssgi_current);
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssgi temporal pass"),
                    color_attachments: &[
                        Some(wgpu::RenderPassColorAttachment {
                            view: &self.ssgi_history[write].light_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                        Some(wgpu::RenderPassColorAttachment {
                            view: &self.ssgi_history[write].depth_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                    ],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                pass.set_pipeline(&self.ssgi_temporal_pipeline);
                pass.set_bind_group(0, &self.ssgi_bg, &[]);
                pass.set_bind_group(1, &self.ssgi_history[read].pair_bg, &[]);
                pass.set_bind_group(2, &self.depth_bg, &[]);
                pass.set_bind_group(3, &self.ssgi_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
            // Back into the trace's own target, which the temporal pass has
            // finished reading: the composite reads it as before.
            if self.ssgi_spatial {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssgi spatial pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.ssgi_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                pass.set_pipeline(&self.ssgi_spatial_pipeline);
                pass.set_bind_group(0, &self.ssgi_history[write].light_bg, &[]);
                pass.set_bind_group(1, &self.depth_bg, &[]);
                pass.set_bind_group(2, &self.normal_bg, &[]);
                pass.set_bind_group(3, &self.ssgi_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
            self.ssgi_current = write;
            self.ssgi_history_valid = true;
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssgi composite pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &self.hdr_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                pass.set_pipeline(&self.ssgi_composite_pipeline);
                // The filtered light, or -- with the filter off -- the
                // temporal pass's own, in this frame's half of the history.
                let light = if self.ssgi_spatial {
                    &self.ssgi_bg
                } else {
                    &self.ssgi_history[self.ssgi_current].light_bg
                };
                pass.set_bind_group(0, light, &[]);
                pass.set_bind_group(1, &self.ssgi_history[self.ssgi_current].composite_bg, &[]);
                pass.set_bind_group(2, &self.depth_bg, &[]);
                pass.set_bind_group(3, &self.ssgi_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
        }

        // Last frame's lit scene, for next frame's reflections and GI: copied
        // here, after screen-space GI has replaced what it replaces and before
        // fog and the rest are laid over it -- the scene as surfaces see it,
        // as HDRP's colour pyramid is. A frame that copies nothing leaves it
        // a frame that is no longer the last one, and says so.
        if (self.ssr_enabled || self.ssgi_enabled) && !fast_render {
            encoder.copy_texture_to_texture(
                self._hdr_texture.as_image_copy(),
                self.lit_history_texture.as_image_copy(),
                self._hdr_texture.size(),
            );
            self.lit_history_valid = true;
        } else {
            self.lit_history_valid = false;
        }

        if self.fog_enabled {
            // The froxel volumes, filled before anything samples them. One
            // pass with two dispatches: the injection volume's transition from
            // storage-write to sampled between them is what orders the second
            // behind the first.
            //
            // Compute contributes no draws, so the counters below stay out of
            // this block.
            let groups_x = crate::froxel::FROXEL_X.div_ceil(FROXEL_WORKGROUP);
            let groups_y = crate::froxel::FROXEL_Y.div_ceil(FROXEL_WORKGROUP);
            // Write into the half NOT being sampled, exactly as the TAA resolve
            // does with its history pair.
            let fog_history_write = 1 - self.fog_history_read;
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("froxel fog pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.fog_inject_pipeline);
            pass.set_bind_group(0, &self.fog_uniform_bg, &[]);
            pass.set_bind_group(1, &self.injection_write_bgs[fog_history_write], &[]);
            // The shadow maps this frame's shadow passes just filled. The
            // integration pass below re-binds group 2 for its own output
            // volume, which is why this one has to be set per dispatch rather
            // than once for the pass.
            pass.set_bind_group(2, &self.froxel_shadow_bg, &[]);
            // The accumulation the previous fog frame left. Bound even when
            // `fog_history_valid` is false -- the pipeline layout demands the
            // group, and the shader's `history_valid` check is what keeps an
            // uninitialised volume from reaching the blend.
            pass.set_bind_group(3, &self.injection_history_bgs[self.fog_history_read], &[]);
            // One thread per froxel.
            pass.dispatch_workgroups(groups_x, groups_y, crate::froxel::FROXEL_Z);

            pass.set_pipeline(&self.fog_integrate_pipeline);
            pass.set_bind_group(0, &self.fog_uniform_bg, &[]);
            // The volume the injection dispatch above just wrote, not the one it
            // read: integration marches this frame's accumulated scattering.
            pass.set_bind_group(1, &self.injection_read_bgs[fog_history_write], &[]);
            pass.set_bind_group(2, &self.integrated_write_bg, &[]);
            // One thread per froxel *column*: each marches the whole Z axis.
            pass.dispatch_workgroups(groups_x, groups_y, 1);
            drop(pass);

            // The volume just written holds the accumulation the next foggy
            // frame reprojects out of.
            self.fog_history_read = fog_history_write;
            self.fog_history_valid = true;
        } else {
            // Nothing ran, so the pair still holds whatever the last foggy frame
            // left there -- a frame with a different camera, and possibly a
            // different scene. Switching fog back on has to start over rather
            // than average against it.
            self.fog_history_valid = false;
        }

        {
            // Always drawn, `fast_render` included: every pass below reads
            // `fog_hdr_bg`, so skipping this one would leave them sampling the
            // clear colour instead of the scene.
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fog apply pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.fog_hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.fog_pipeline);
            pass.set_bind_group(0, &self.hdr_bg, &[]);
            pass.set_bind_group(1, &self.depth_bg, &[]);
            // Bound even with fog off: the pipeline layout demands all four
            // groups, and the shader's `enabled == 0` early-out is what stops
            // the volume from being read.
            pass.set_bind_group(2, &self.integrated_read_bg, &[]);
            pass.set_bind_group(3, &self.fog_uniform_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }

        // Depth of field, between fog and everything that reads the scene:
        // bloom has to spread the *blurred* highlights, and the composite has
        // to tonemap the blurred image. It writes into the raw scene target,
        // which nothing reads again after the fog pass consumed it, so no
        // extra full-screen texture is needed; the passes below then read
        // that target instead of the fogged one.
        let dof_ran = self.dof_on && !fast_render;
        if dof_ran {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dof pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.hdr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.dof_pipeline);
            pass.set_bind_group(0, &self.fog_hdr_bg, &[]);
            pass.set_bind_group(1, &self.depth_bg, &[]);
            pass.set_bind_group(2, &self.ssao_cam_bg, &[]);
            pass.set_bind_group(3, &self.dof_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }

        // Motion blur, after depth of field as in Unreal (a defocused
        // highlight streaks as the disc it became) and before bloom and the
        // composite, for the same reason depth of field is. It ping-pongs
        // between the two full-screen HDR targets: it reads whichever one
        // holds the scene so far and writes the other -- after the fog pass,
        // and after depth of field if it ran, nothing reads the one it
        // overwrites.
        let motion_blur_ran = self.motion_blur_on && !fast_render;
        if motion_blur_ran {
            let (source, target) = if dof_ran {
                (&self.hdr_bg, &self.fog_hdr_view)
            } else {
                (&self.fog_hdr_bg, &self.hdr_view)
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("motion blur pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.motion_blur_pipeline);
            pass.set_bind_group(0, source, &[]);
            pass.set_bind_group(1, &self.depth_bg, &[]);
            pass.set_bind_group(2, &self.taa_uniform_bg, &[]);
            pass.set_bind_group(3, &self.motion_blur_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }
        // Where the scene now is: each of the two passes above that ran moved
        // it to the other target.
        let scene_bg = if dof_ran != motion_blur_ran {
            &self.hdr_bg
        } else {
            &self.fog_hdr_bg
        };

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("bloom pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.bloom_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if !fast_render {
                pass.set_pipeline(&self.bloom_pipeline);
                pass.set_bind_group(0, scene_bg, &[]);
                pass.set_bind_group(1, &self.config_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ssao pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ao_raw_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if !fast_render {
                pass.set_pipeline(&self.ssao_pipeline);
                pass.set_bind_group(0, &self.depth_bg, &[]);
                pass.set_bind_group(1, &self.config_bg, &[]);
                pass.set_bind_group(2, &self.ssao_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("ssao blur pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ao_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            if !fast_render {
                pass.set_pipeline(&self.ssao_blur_pipeline);
                pass.set_bind_group(0, &self.depth_bg, &[]);
                pass.set_bind_group(1, &self.ao_raw_bg, &[]);
                pass.set_bind_group(2, &self.ssao_cam_bg, &[]);
                pass.draw(0..3, 0..1);
                draw_calls += 1;
                triangles += 1;
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("composite pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.ldr_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.composite_pipeline);
            pass.set_bind_group(0, scene_bg, &[]);
            pass.set_bind_group(1, &self.bloom_bg, &[]);
            pass.set_bind_group(2, &self.ao_bg, &[]);
            pass.set_bind_group(3, &self.composite_config_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }

        // FXAA, on the finished LDR image: after the composite, since edges
        // are to be found by the contrast the player sees, and before the
        // TAA resolve, which then accumulates the smoothed frames (and, with
        // TAA off, is the plain copy to the swapchain).
        // SMAA in the same place, and instead of FXAA when both are asked
        // for.
        let smaa_bg = if fast_render {
            None
        } else {
            self.smaa.encode(encoder)
        };
        if smaa_bg.is_some() {
            draw_calls += 3;
            triangles += 3;
        }
        let fxaa_ran = self.fxaa_on && !fast_render && smaa_bg.is_none();
        if fxaa_ran {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("fxaa pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.fxaa_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.fxaa_pipeline);
            pass.set_bind_group(0, &self.ldr_bg, &[]);
            pass.set_bind_group(1, &self.fxaa_bg_uniform, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }
        // This frame's finished colour, as the resolve reads it.
        let current_bg = if let Some(bg) = smaa_bg {
            bg
        } else if fxaa_ran {
            &self.fxaa_bg
        } else {
            &self.ldr_bg
        };

        {
            // Without a valid history the source is this frame's own LDR, so
            // the blend degenerates to `mix(current, current, blend)` -- a
            // no-op -- instead of reading an uninitialised texture.
            let history_src = if self.history_valid {
                &self.history_bgs[self.history_read]
            } else {
                current_bg
            };
            // Write into the half NOT being sampled; the flip below makes it
            // next frame's read source.
            let history_dst = &self.history_views[1 - self.history_read];
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("taa pass"),
                color_attachments: &[
                    Some(wgpu::RenderPassColorAttachment {
                        view: surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                    Some(wgpu::RenderPassColorAttachment {
                        view: history_dst,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                ],
                depth_stencil_attachment: None,
                ..Default::default()
            });
            pass.set_pipeline(&self.taa_pipeline);
            pass.set_bind_group(0, current_bg, &[]);
            pass.set_bind_group(1, history_src, &[]);
            pass.set_bind_group(2, &self.depth_bg, &[]);
            pass.set_bind_group(3, &self.taa_uniform_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }

        // The target just written holds the frame the next resolve reprojects.
        self.history_read = 1 - self.history_read;
        self.history_valid = true;

        (draw_calls, triangles)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::surface::WgpuSurface;

    /// Four shaders declare `PostProcessConfig` by hand and all four read the
    /// one buffer, so they must declare it identically. A field added to
    /// some of them -- as the grade fields were, to all four at once -- would
    /// shift every later field in the others and misread them silently.
    #[test]
    fn every_shader_declares_the_same_post_process_config() {
        let decl = |src: &str| -> String {
            let start = src
                .find("struct PostProcessConfig {")
                .expect("declares PostProcessConfig");
            let end = start + src[start..].find('}').unwrap();
            src[start..=end].to_string()
        };
        let reference = decl(COMPOSITE_WGSL);
        assert!(
            reference.contains("grade_filter_b: f32"),
            "premise: the reference declaration carries the grade fields"
        );
        for (name, src) in [
            ("BLOOM_WGSL", BLOOM_WGSL),
            ("SSAO_WGSL", SSAO_WGSL),
            ("TAA_WGSL", TAA_WGSL),
        ] {
            assert_eq!(
                decl(src),
                reference,
                "{name} declares a different PostProcessConfig"
            );
        }
    }

    #[test]
    fn config_gpu_size() {
        assert_eq!(std::mem::size_of::<PostProcessConfigGpu>(), 96);
        assert_eq!(CONFIG_SIZE, 96);
    }

    #[test]
    fn ssao_cam_gpu_size() {
        assert_eq!(std::mem::size_of::<SsaoCameraGpu>(), 128);
    }

    #[test]
    fn taa_cam_gpu_size() {
        assert_eq!(std::mem::size_of::<TaaCameraGpu>(), 128);
    }

    #[test]
    fn fog_uniform_size() {
        // Must equal the `min_binding_size` the fog bind group layout declares
        // and the size WGSL computes for `FogUniform`; a mismatch is a
        // validation error at bind time, not a compile error.
        assert_eq!(
            std::mem::size_of::<FogUniform>() as u64,
            FOG_UNIFORM_SIZE,
            "FogUniform must stay {FOG_UNIFORM_SIZE} bytes"
        );
    }

    #[test]
    fn hdr_format_is_rgba16float() {
        assert_eq!(HDR_FORMAT, wgpu::TextureFormat::Rgba16Float);
    }

    #[test]
    fn fog_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(fog_apply_wgsl().into()),
        });
    }

    /// The two compute shaders are the first in the engine, so nothing else
    /// would catch a WGSL error in them until it surfaced as a panic deep
    /// inside a pixel test.
    #[test]
    fn fog_inject_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(fog_inject_wgsl().into()),
        });
    }

    /// The text of `point_shadow_factor` in `src`, from its signature to the
    /// closing brace in column 0. Every brace inside the function is indented,
    /// so the first unindented one is its end.
    fn point_shadow_factor_source(src: &str) -> &str {
        let start = src
            .find("fn point_shadow_factor(")
            .expect("both shaders define point_shadow_factor");
        let body = &src[start..];
        let end = body
            .find("\n}\n")
            .expect("the function ends at a closing brace in column 0")
            + "\n}\n".len();
        &body[..end]
    }

    /// The cube-face selection in `point_shadow_factor` matches
    /// `point_light_face_view_projs`'s Rust-side `look_at_rh`/`perspective`
    /// construction, and that correspondence was established by hand rather
    /// than by any test. So the froxel copy must be exactly the scene shader's,
    /// character for character -- a re-derivation that picked the wrong face,
    /// or the right face with a mirrored in-face UV, would still return
    /// plausible shadows.
    ///
    /// A pixel test cannot stand in for this, and it was worth checking rather
    /// than assuming: swapping the `+Y`/`-Y` face constants does move
    /// `pixels_shafts.rs`'s reading (which is why its point-light assertion is
    /// a fraction rather than a fixed margin), but flipping the sign of `v`
    /// within a face does not move it at all, because the test's occluder
    /// covers that whole face and every texel on it reads the same distance.
    /// Only comparing the source catches that one.
    #[test]
    fn the_injection_pass_ports_point_shadow_factor_verbatim() {
        let inject = fog_inject_wgsl();
        let scene = point_shadow_factor_source(crate::surface::MESH_WGSL);
        let froxel = point_shadow_factor_source(&inject);
        assert_eq!(
            scene, froxel,
            "the froxel injection shader's `point_shadow_factor` has drifted \
             from the scene shader's. Whichever side changed, make them equal \
             again rather than re-deriving either: a froxel and a surface at \
             one world position have to read the same cube texel, or the fog's \
             shafts will not line up with the shadows the geometry casts"
        );
    }

    /// See [`fog_inject_shader_compiles`].
    #[test]
    fn fog_integrate_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(fog_integrate_wgsl().into()),
        });
    }

    /// Runs a test-only compute entry point named `cs_probe`, appended to the
    /// real shared froxel WGSL, and returns the `out_floats` values it wrote
    /// to `out_values` at `@group(1) @binding(0)`.
    ///
    /// Only the entry point a caller supplies is test-only. The preamble, the
    /// `FogUniform`, and `FROXEL_COMMON_WGSL` around it are the exact source
    /// the two compute pipelines are built from, and `depth_to_froxel_w` in
    /// there is the exact function the apply shader calls -- so what these
    /// probes measure is the shipping mapping, not a copy of it.
    fn run_froxel_probe(near: f32, far: f32, probe_wgsl: &str, out_floats: usize) -> Vec<f32> {
        use bytemuck::Zeroable as _;
        run_froxel_probe_with(
            FogUniform {
                near,
                far,
                ..FogUniform::zeroed()
            },
            probe_wgsl,
            out_floats,
        )
    }

    /// [`run_froxel_probe`] with the whole uniform supplied, for probes that
    /// read more of it than the depth range -- the reprojection, which needs
    /// real camera matrices, being the case in point.
    fn run_froxel_probe_with(fog: FogUniform, probe_wgsl: &str, out_floats: usize) -> Vec<f32> {
        let bytes = (out_floats * std::mem::size_of::<f32>()) as u64;

        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("froxel probe"),
            source: wgpu::ShaderSource::Wgsl(
                (froxel_wgsl_preamble() + &fog_uniform_wgsl(0) + FROXEL_COMMON_WGSL + probe_wgsl)
                    .into(),
            ),
        });

        let fog_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(FOG_UNIFORM_SIZE),
                },
                count: None,
            }],
        });
        let out_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::COMPUTE,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(bytes),
                },
                count: None,
            }],
        });

        let fog_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: FOG_UNIFORM_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&fog_buffer, 0, bytemuck::cast_slice(&[fog]));
        let out_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let fog_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &fog_uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: fog_buffer.as_entire_binding(),
            }],
        });
        let out_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &out_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: out_buffer.as_entire_binding(),
            }],
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&fog_uniform_bgl, &out_bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&layout),
            module: &shader,
            entry_point: "cs_probe",
            compilation_options: Default::default(),
            cache: None,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: None,
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &fog_bg, &[]);
            pass.set_bind_group(1, &out_bg, &[]);
            pass.dispatch_workgroups(crate::froxel::FROXEL_Z.div_ceil(64), 1, 1);
        }
        encoder.copy_buffer_to_buffer(&out_buffer, 0, &readback, 0, bytes);
        queue.submit(Some(encoder.finish()));

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async never reported a result")
            .expect("mapping the readback buffer failed");
        let got: Vec<f32> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
        readback.unmap();
        got
    }

    /// Reads `froxel_slice_depth` out of the real shared WGSL and compares it
    /// against the Rust it was ported from.
    ///
    /// A mapping that disagrees with the Rust puts the fog at the wrong
    /// distance, which reads as a mistuned density rather than as a mapping
    /// bug. See [`run_froxel_probe`] for what is and is not test-only here.
    #[test]
    fn the_wgsl_depth_slicing_matches_froxel_slice_depth() {
        const PROBE_WGSL: &str = r#"
@group(1) @binding(0) var<storage, read_write> out_values: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn cs_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_Z {
        return;
    }
    out_values[gid.x] = froxel_slice_depth(gid.x);
}
"#;
        let (near, far) = (0.1f32, 100.0f32);
        let slices = crate::froxel::FROXEL_Z as usize;
        let got = run_froxel_probe(near, far, PROBE_WGSL, slices);

        for s in 0..crate::froxel::FROXEL_Z {
            let expected = crate::froxel::froxel_slice_depth(s, near, far);
            let actual = got[s as usize];
            assert!(
                (actual - expected).abs() <= expected * 1e-4,
                "slice {s}: the WGSL depth mapping gave {actual}, the Rust one \
                 {expected}. The two must agree exactly, or fog lands at the \
                 wrong distance and looks like a density problem"
            );
        }
    }

    /// `depth_to_froxel_w`, the function the apply pass turns a pixel's view
    /// depth into a volume coordinate with, must be the exact inverse of the
    /// slicing the injection pass placed that light at.
    ///
    /// Slice `s` holds what was integrated up to its far edge, at
    /// `froxel_slice_depth(s)`, which is `(s+1)/FROXEL_Z` of the way down the
    /// volume -- so feeding that depth back in has to return `(s+1)/FROXEL_Z`.
    /// An inverse that disagrees samples a neighbouring slice's light and
    /// again reads as a density problem, not a mapping one.
    ///
    /// The two entries past the grid check the clamp the background path
    /// depends on: a depth outside `near..far` must land on an edge slice.
    #[test]
    fn the_wgsl_depth_to_froxel_w_inverts_the_slice_mapping() {
        const PROBE_WGSL: &str = r#"
@group(1) @binding(0) var<storage, read_write> out_values: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn cs_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_Z {
        return;
    }
    out_values[gid.x] = depth_to_froxel_w(froxel_slice_depth(gid.x));
    if gid.x == 0u {
        out_values[FROXEL_Z] = depth_to_froxel_w(fog.near * 0.5);
        out_values[FROXEL_Z + 1u] = depth_to_froxel_w(fog.far * 2.0);
    }
}
"#;
        let (near, far) = (0.1f32, 100.0f32);
        let slices = crate::froxel::FROXEL_Z as usize;
        let got = run_froxel_probe(near, far, PROBE_WGSL, slices + 2);

        for s in 0..crate::froxel::FROXEL_Z {
            let expected = (s + 1) as f32 / crate::froxel::FROXEL_Z as f32;
            let actual = got[s as usize];
            assert!(
                (actual - expected).abs() <= 1e-4,
                "slice {s}: depth_to_froxel_w(froxel_slice_depth({s})) gave \
                 {actual}, not {expected}. The apply pass would sample the \
                 wrong slice, which looks like mistuned density"
            );
        }
        assert_eq!(
            got[slices], 0.0,
            "a depth nearer than the near plane must clamp onto the first slice"
        );
        assert_eq!(
            got[slices + 1],
            1.0,
            "a depth past the far plane must clamp onto the last slice -- this \
             is the path every background pixel takes"
        );
    }

    /// The WGSL `froxel_jitter` must be the exact mirror of the Rust one, and
    /// must be a real dither rather than a constant shift.
    ///
    /// Both halves matter and neither implies the other. Determinism is what
    /// keeps a headless replay reproducible -- the same froxel on the same frame
    /// has to give the same offset on every run, which is why this is a hash of
    /// the coordinate rather than any kind of running random state. Variation
    /// across froxels is what makes it dithering at all: a jitter that returned
    /// one value everywhere would move every slice boundary together and leave
    /// the bands exactly as visible as before.
    ///
    /// Comparing against the Rust rather than merely against itself is what
    /// catches a drifted port -- the two are separate texts, and `froxel.rs`'s
    /// own tests only pin the Rust side.
    #[test]
    fn the_wgsl_froxel_jitter_matches_the_rust_one() {
        // One froxel coordinate per thread, over four frames, so the frame term
        // is exercised too: a port that dropped it would still be deterministic
        // and still vary across the grid.
        const FRAMES: u32 = 4;
        const PROBE_WGSL: &str = r#"
@group(1) @binding(0) var<storage, read_write> out_values: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn cs_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_Z {
        return;
    }
    // A coordinate that varies on all three axes with the thread index, so no
    // axis of the hash can be dropped without moving a value here.
    let coord = vec3<u32>(gid.x * 3u + 1u, gid.x * 7u + 2u, gid.x);
    for (var f: u32 = 0u; f < 4u; f = f + 1u) {
        out_values[f * FROXEL_Z + gid.x] = froxel_jitter(coord, f);
    }
}
"#;
        let slices = crate::froxel::FROXEL_Z;
        let got = run_froxel_probe(0.1, 100.0, PROBE_WGSL, (slices * FRAMES) as usize);

        let mut distinct_within_a_frame = std::collections::HashSet::new();
        for frame in 0..FRAMES {
            for i in 0..slices {
                let coord = [i * 3 + 1, i * 7 + 2, i];
                let expected = crate::froxel::froxel_jitter(coord, frame);
                let actual = got[(frame * slices + i) as usize];
                assert_eq!(
                    actual, expected,
                    "froxel {coord:?} on frame {frame}: the WGSL jitter gave \
                     {actual}, the Rust one {expected}. The two are separate \
                     texts and have to stay identical, or the shader dithers \
                     with a pattern no test on the Rust side describes"
                );
                if frame == 0 {
                    distinct_within_a_frame.insert(actual.to_bits());
                }
            }
        }
        assert!(
            distinct_within_a_frame.len() > (slices as usize) / 2,
            "{} of {slices} froxels shared an offset within one frame -- a \
             jitter that barely varies is a constant shift of the whole grid, \
             which moves the slice bands rather than breaking them up",
            slices as usize - distinct_within_a_frame.len()
        );
    }

    /// The dithered sample depth must land strictly inside the froxel's own
    /// slice, and must actually move when the frame index does.
    ///
    /// Three separate failures this rules out, none implied by the others:
    /// reaching past a slice edge (which lights a froxel from its neighbour's
    /// depth -- worse than the banding the dither exists to remove), collapsing
    /// to a fixed point inside the slice (banding again, just at a different
    /// depth), and ignoring `frame` (a static pattern that temporal
    /// accumulation would average to itself forever rather than resolve).
    ///
    /// It probes the real shared `froxel_sample_depth`, which is why that
    /// function lives in [`FROXEL_COMMON_WGSL`] rather than beside its caller.
    #[test]
    fn the_jittered_sample_stays_inside_its_own_slice() {
        // Per slice: the sample depth on frames 0 and 1, then that slice's two
        // edges, so the bounds are the shader's own rather than a Rust
        // re-derivation of them.
        const PROBE_WGSL: &str = r#"
@group(1) @binding(0) var<storage, read_write> out_values: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn cs_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_Z {
        return;
    }
    let coord = vec3<u32>(11u, 7u, gid.x);
    out_values[gid.x * 4u + 0u] = froxel_sample_depth(coord, 0u);
    out_values[gid.x * 4u + 1u] = froxel_sample_depth(coord, 1u);
    out_values[gid.x * 4u + 2u] = select(froxel_slice_depth(gid.x - 1u), fog.near, gid.x == 0u);
    out_values[gid.x * 4u + 3u] = froxel_slice_depth(gid.x);
}
"#;
        let slices = crate::froxel::FROXEL_Z as usize;
        let got = run_froxel_probe(0.1, 100.0, PROBE_WGSL, slices * 4);

        let mut frames_differed = 0usize;
        for s in 0..slices {
            let [f0, f1, near_edge, far_edge] =
                [got[s * 4], got[s * 4 + 1], got[s * 4 + 2], got[s * 4 + 3]];
            for (frame, depth) in [(0, f0), (1, f1)] {
                assert!(
                    depth >= near_edge && depth <= far_edge,
                    "slice {s} on frame {frame} sampled at depth {depth}, outside \
                     its own span {near_edge}..{far_edge}. A sample past a slice \
                     edge lights that froxel from its neighbour's depth, which \
                     misplaces the shaft rather than smoothing it"
                );
            }
            if f0 != f1 {
                frames_differed += 1;
            }
        }
        assert!(
            frames_differed > slices / 2,
            "only {frames_differed} of {slices} slices sampled a different depth \
             on frame 1 than on frame 0 -- an offset that does not follow the \
             frame index is a fixed pattern, and temporal accumulation averages \
             a fixed pattern to itself instead of resolving it"
        );
    }

    /// The history reprojection must be the exact inverse of the mapping that
    /// placed the froxel in the world, and must reject rather than clamp when
    /// there is no history to find.
    ///
    /// A still camera is the case where the answer is knowable exactly: with the
    /// previous frame's matrices equal to this frame's, every froxel has to
    /// reproject onto *itself*. Nothing else about temporal accumulation can be
    /// checked this precisely -- a pixel test sees the blend, not the
    /// coordinate, so a reprojection off by a froxel or two would show up there
    /// as slightly soft shafts and nothing more.
    ///
    /// The rejection half matters just as much and is not implied: history from
    /// outside the previous volume does not exist, and taking the edge froxel's
    /// light instead would smear it along the frustum border. It is the same
    /// rule the TAA resolve applies to off-screen history.
    #[test]
    fn the_history_reprojection_inverts_the_froxel_mapping() {
        // Four outputs per thread: the reprojected coordinate and its validity
        // flag. `froxel_centre_world_pos` is what the injection pass actually
        // reprojects, and its round trip has an exactly known answer -- the
        // froxel's own texel centre -- so nothing here has to be compared
        // against another run of the same shader code.
        const PROBE_WGSL: &str = r#"
@group(1) @binding(0) var<storage, read_write> out_values: array<f32>;

@compute @workgroup_size(64, 1, 1)
fn cs_probe(@builtin(global_invocation_id) gid: vec3<u32>) {
    if gid.x >= FROXEL_Z {
        return;
    }
    // Deliberately off-centre in X and Y: a column through the middle of the
    // screen reprojects onto itself under almost any matrix mistake, because
    // both its U and its V are 0.5.
    let coord = vec3<u32>(37u, 19u, gid.x);
    let h = froxel_history_uvw(froxel_centre_world_pos(coord));
    out_values[gid.x * 4u + 0u] = h.x;
    out_values[gid.x * 4u + 1u] = h.y;
    out_values[gid.x * 4u + 2u] = h.z;
    out_values[gid.x * 4u + 3u] = h.w;
}
"#;
        use bytemuck::Zeroable as _;
        let (near, far) = (0.1f32, 100.0f32);
        let cam_pos = glam::Vec3::new(3.0, 2.0, 9.0);
        let proj = glam::Mat4::perspective_rh(60f32.to_radians(), 16.0 / 9.0, near, far);
        let view_proj = proj * glam::Mat4::look_at_rh(cam_pos, glam::Vec3::ZERO, glam::Vec3::Y);
        let still = FogUniform {
            inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            prev_view_proj: view_proj.to_cols_array_2d(),
            prev_inv_view_proj: view_proj.inverse().to_cols_array_2d(),
            camera_pos: cam_pos.to_array(),
            prev_camera_pos: cam_pos.to_array(),
            near,
            far,
            ..FogUniform::zeroed()
        };

        let slices = crate::froxel::FROXEL_Z as usize;
        let got = run_froxel_probe_with(still, PROBE_WGSL, slices * 4);

        let expect_u = (37.0 + 0.5) / crate::froxel::FROXEL_X as f32;
        let expect_v = (19.0 + 0.5) / crate::froxel::FROXEL_Y as f32;
        for s in 0..slices {
            let (u, v, w, valid) = (got[s * 4], got[s * 4 + 1], got[s * 4 + 2], got[s * 4 + 3]);
            // The froxel's own texel centre down the W axis. Landing anywhere
            // else means the trilinear read straddles two slices on every frame
            // of a *still* camera, which mixes the neighbours' light back in
            // instead of accumulating this froxel's.
            let expect_w = (s as f32 + 0.5) / crate::froxel::FROXEL_Z as f32;
            assert_eq!(
                valid, 1.0,
                "slice {s} reprojected to nothing under a still camera. Every \
                 froxel of this frame's volume was in the previous one too, so a \
                 rejection here is the mapping failing, not history genuinely \
                 missing"
            );
            assert!(
                (u - expect_u).abs() < 1e-4 && (v - expect_v).abs() < 1e-4,
                "slice {s} reprojected to ({u}, {v}), not its own ({expect_u}, \
                 {expect_v}). With the camera still, a froxel must find itself; \
                 an offset here blends each froxel with a neighbouring column's \
                 light and smears the shafts sideways"
            );
            assert!(
                (w - expect_w).abs() < 1e-3,
                "slice {s} reprojected to depth coordinate {w}, not its own \
                 texel centre {expect_w}. The recovered view depth does not \
                 invert the forward mapping, so the accumulation averages each \
                 froxel with one at a different distance"
            );
        }

        // The other half: a camera that was facing the other way last frame puts
        // every one of this frame's froxels behind the previous eye, where no
        // history exists.
        let backwards = proj
            * glam::Mat4::look_at_rh(
                cam_pos,
                cam_pos + (cam_pos - glam::Vec3::ZERO),
                glam::Vec3::Y,
            );
        let turned = FogUniform {
            prev_view_proj: backwards.to_cols_array_2d(),
            prev_inv_view_proj: backwards.inverse().to_cols_array_2d(),
            ..still
        };
        let got = run_froxel_probe_with(turned, PROBE_WGSL, slices * 4);
        for s in 0..slices {
            assert_eq!(
                got[s * 4 + 3],
                0.0,
                "slice {s} claimed history from a frame whose camera faced the \
                 other way. A point that was behind the previous eye has no \
                 history, and accepting one means the rejection is not \
                 rejecting -- which shows up as light smeared along the frustum \
                 border whenever the camera turns"
            );
        }
    }

    /// The generated preamble is the only thing keeping the WGSL grid bounds
    /// tied to the Rust ones, so assert it actually carries them.
    #[test]
    fn froxel_preamble_carries_the_rust_grid_dimensions() {
        let p = froxel_wgsl_preamble();
        for (name, value) in [
            ("FROXEL_X", crate::froxel::FROXEL_X),
            ("FROXEL_Y", crate::froxel::FROXEL_Y),
            ("FROXEL_Z", crate::froxel::FROXEL_Z),
        ] {
            let expected = format!("const {name}: u32 = {value}u;");
            assert!(
                p.contains(&expected),
                "the WGSL preamble must declare `{expected}`, got:\n{p}"
            );
        }
    }

    #[test]
    fn bloom_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
        });
    }

    #[test]
    fn ssao_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(SSAO_WGSL.into()),
        });
    }

    #[test]
    fn composite_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });
    }

    #[test]
    fn taa_shader_compiles() {
        let (device, _queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let _m = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(TAA_WGSL.into()),
        });
    }

    /// Stand-ins for the scene's shadow resources, so a test can build a
    /// [`PostProcessState`] without a whole [`WgpuSurface`] behind it.
    ///
    /// 1x1 textures on purpose: nothing here asserts on what the shadow lookup
    /// *reads*, only that the pipeline layout, the bind group and the WGSL
    /// agree about what is bound. The formats and view dimensions therefore do
    /// have to be the real ones -- those are exactly what a mismatch would show
    /// up in -- but the sizes do not.
    struct TestShadowResources {
        _shadow_texture: crate::profiler::TrackedTexture,
        shadow_view: wgpu::TextureView,
        shadow_sampler: wgpu::Sampler,
        _point_texture: crate::profiler::TrackedTexture,
        point_view: wgpu::TextureView,
        light_buffer: wgpu::Buffer,
        cookies: crate::cookie::CookieAtlas,
        _spot_texture: crate::profiler::TrackedTexture,
        spot_view: wgpu::TextureView,
        spot_buffer: wgpu::Buffer,
        rect_buffer: wgpu::Buffer,
    }

    impl TestShadowResources {
        fn new(device: &wgpu::Device) -> Self {
            let one = wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            };
            let shadow_texture = crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some("test shadow map"),
                    // One layer per cascade, matching the real shadow map: the
                    // fog pass binds whatever the surface hands it, and the
                    // layout now demands an array.
                    size: wgpu::Extent3d {
                        depth_or_array_layers: bsengine_core::shadow_config::MAX_CASCADES as u32,
                        ..one
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::surface::DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );
            let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let shadow_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("test shadow comparison sampler"),
                compare: Some(wgpu::CompareFunction::LessEqual),
                ..Default::default()
            });
            let point_texture = crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some("test point shadow array"),
                    size: wgpu::Extent3d {
                        depth_or_array_layers: 6,
                        ..one
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::R32Float,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );
            let point_view = point_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("test light uniform"),
                size: crate::surface::LIGHT_UNIFORM_SIZE,
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let spot_texture = crate::profiler::create_tracked_texture(
                device,
                &wgpu::TextureDescriptor {
                    label: Some("test spot shadow array"),
                    size: one,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: crate::surface::DEPTH_FORMAT,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
            );
            let spot_view = spot_texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let spot_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("test spot shadow matrices"),
                size: std::mem::size_of::<crate::spot_shadow::SpotShadowUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });
            let rect_buffer = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("test rect lights"),
                size: std::mem::size_of::<crate::area_light::RectLightUniform>() as u64,
                usage: wgpu::BufferUsages::UNIFORM,
                mapped_at_creation: false,
            });
            Self {
                _shadow_texture: shadow_texture,
                shadow_view,
                shadow_sampler,
                _point_texture: point_texture,
                point_view,
                light_buffer,
                cookies: crate::cookie::CookieAtlas::new(device),
                _spot_texture: spot_texture,
                spot_view,
                spot_buffer,
                rect_buffer,
            }
        }

        fn bindings(&self) -> FroxelShadowBindings<'_> {
            FroxelShadowBindings {
                shadow_map_view: &self.shadow_view,
                shadow_sampler: &self.shadow_sampler,
                point_shadow_view: &self.point_view,
                light_buffer: &self.light_buffer,
                cookie_view: &self.cookies.array_view,
                cookie_sampler: &self.cookies.sampler,
                spot_shadow_view: &self.spot_view,
                spot_shadow_buffer: &self.spot_buffer,
                rect_light_buffer: &self.rect_buffer,
            }
        }
    }

    /// The resolve binds five groups and writes two colour attachments, and
    /// every one of them has to line up with the WGSL: a wrong group index, a
    /// binding the layout does not declare, or a second target the fragment
    /// entry does not return is a `wgpu` validation error. Compiling the
    /// shader alone proves none of that -- outside this test the mismatch
    /// would surface only as a panic deep inside a pixel test.
    ///
    /// Two `apply` calls, because the two history states take different
    /// branches: the first has no history and binds the current LDR, the
    /// second binds what the first wrote.
    #[test]
    fn taa_pass_records_without_validation_errors() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let width = 64;
        let height = 64;
        let depth_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("taa test depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::surface::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = crate::output::create_offscreen_texture(&device, width, height);
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow = TestShadowResources::new(&device);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut pp = PostProcessState::new(
            &device,
            width,
            height,
            &depth_view,
            crate::output::OFFSCREEN_FORMAT,
            &shadow.bindings(),
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        pp.apply(&mut encoder, &target_view, false);
        queue.submit(Some(encoder.finish()));
        assert!(
            pp.history_valid,
            "the first resolve writes a history target, so the second one has \
             something to reproject"
        );
        assert_eq!(
            pp.history_read, 1,
            "the ping-pong must advance: the half just written is next \
             frame's read source, and never the half read this frame"
        );

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        pp.apply(&mut encoder, &target_view, false);
        queue.submit(Some(encoder.finish()));
        assert_eq!(pp.history_read, 0, "the second resolve flips it back");

        device.poll(wgpu::Maintain::Wait);
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "the TAA resolve pass must record cleanly; a validation error here \
             means the pipeline layout, the bind groups, or the two colour \
             attachments disagree with the shader"
        );
    }

    /// Compiling the two compute shaders proves nothing about the bindings
    /// around them: a wrong group index, a storage format the layout does not
    /// declare, or a dispatch over the device's workgroup limits is a `wgpu`
    /// validation error, not a WGSL one. No pixel test reaches this code
    /// either, because none of them enables fog.
    #[test]
    fn froxel_compute_passes_record_without_validation_errors() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let width = 64;
        let height = 64;
        let depth_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("froxel test depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::surface::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = crate::output::create_offscreen_texture(&device, width, height);
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow = TestShadowResources::new(&device);
        device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut pp = PostProcessState::new(
            &device,
            width,
            height,
            &depth_view,
            crate::output::OFFSCREEN_FORMAT,
            &shadow.bindings(),
        );
        assert!(
            !pp.fog_enabled,
            "fog must be off until something uploads a FogUniform, or every \
             pre-existing pixel test would start dispatching the froxel passes"
        );

        pp.update_fog(
            &queue,
            FogUniform {
                inv_view_proj: glam::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0)
                    .inverse()
                    .to_cols_array_2d(),
                cascade_view_proj: [glam::Mat4::orthographic_rh(
                    -30.0, 30.0, -30.0, 30.0, 0.1, 200.0,
                )
                .to_cols_array_2d();
                    bsengine_core::shadow_config::MAX_CASCADES],
                prev_view_proj: glam::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0).to_cols_array_2d(),
                prev_inv_view_proj: glam::Mat4::perspective_rh(1.0, 1.0, 0.1, 100.0)
                    .inverse()
                    .to_cols_array_2d(),
                camera_pos: [0.0, 0.0, 0.0],
                near: 0.1,
                light_dir: [0.0, 1.0, 0.0],
                far: 100.0,
                light_color: [1.0, 1.0, 1.0],
                density: 0.05,
                fog_color: [0.5, 0.6, 0.7],
                anisotropy: 0.3,
                prev_camera_pos: [0.0, 0.0, 0.0],
                cascade_blend: 0.0,
                enabled: 1,
                frame_index: 0,
                history_valid: 0,
                cascade_count: 1,
                cascade_splits: [f32::MAX; 4],
                cam_forward: [0.0, 0.0, -1.0],
                _pad0: 0.0,
            },
        );
        assert!(pp.fog_enabled, "a nonzero `enabled` must arm the dispatch");
        assert!(
            !pp.fog_history_valid,
            "the scattering ping-pong must start empty, or the very first fog \
             frame averages itself with an uninitialised volume"
        );

        // Twice, because the two history states take different branches through
        // the injection pass, exactly as they do in the TAA resolve: the first
        // dispatch has nothing to reproject, the second reprojects what the
        // first wrote.
        for expected_read in [1usize, 0] {
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            pp.apply(&mut encoder, &target_view, false);
            queue.submit(Some(encoder.finish()));
            assert_eq!(
                pp.fog_history_read, expected_read,
                "the scattering ping-pong must advance: the volume just written \
                 is the next frame's history, and never the one just sampled"
            );
            assert!(
                pp.fog_history_valid,
                "a fog dispatch leaves an accumulation behind for the next one"
            );
        }

        device.poll(wgpu::Maintain::Wait);
        let err = pollster::block_on(device.pop_error_scope());
        assert!(
            err.is_none(),
            "the froxel injection and integration passes must record cleanly; \
             a validation error here means the pipeline layouts, the bind \
             groups, or the storage-texture formats disagree with the WGSL: {err:?}"
        );
    }

    /// Fog switched off has to reset the accumulation, not merely pause it.
    ///
    /// The volumes are never cleared, so a frame with fog off leaves the last
    /// foggy frame's scattering sitting in them -- rendered with a different
    /// camera, and quite possibly a different scene. Blending against that when
    /// fog comes back would ghost the old frame into the new one, and it is the
    /// one stale-history case `history_valid`'s first-frame rule does not
    /// already cover.
    #[test]
    fn switching_fog_off_discards_the_accumulation() {
        let (device, queue) = pollster::block_on(WgpuSurface::headless_device_for_testing());
        let width = 64;
        let height = 64;
        let depth_texture = crate::profiler::create_tracked_texture(
            &device,
            &wgpu::TextureDescriptor {
                label: Some("fog history test depth"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: crate::surface::DEPTH_FORMAT,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                    | wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            },
        );
        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let target = crate::output::create_offscreen_texture(&device, width, height);
        let target_view = target.create_view(&wgpu::TextureViewDescriptor::default());

        let shadow = TestShadowResources::new(&device);
        let mut pp = PostProcessState::new(
            &device,
            width,
            height,
            &depth_view,
            crate::output::OFFSCREEN_FORMAT,
            &shadow.bindings(),
        );

        use bytemuck::Zeroable as _;
        let foggy = FogUniform {
            near: 0.1,
            far: 100.0,
            density: 0.05,
            enabled: 1,
            ..FogUniform::zeroed()
        };
        let run = |pp: &mut PostProcessState, fog: FogUniform| {
            pp.update_fog(&queue, fog);
            let mut encoder =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            pp.apply(&mut encoder, &target_view, false);
            queue.submit(Some(encoder.finish()));
        };

        run(&mut pp, foggy);
        assert!(pp.fog_history_valid, "a foggy frame builds an accumulation");

        run(
            &mut pp,
            FogUniform {
                enabled: 0,
                ..foggy
            },
        );
        assert!(
            !pp.fog_history_valid,
            "a frame with fog off must discard the accumulation: the volumes \
             still hold the last foggy frame, and blending the next one against \
             it would ghost a frame rendered under a different camera into it"
        );

        // And the uniform the next foggy frame actually reads has to carry that
        // decision -- the flag on this struct is only half of it.
        pp.update_fog(&queue, foggy);
        assert!(
            !pp.fog_history_valid,
            "uploading a foggy frame must not by itself revive the history; \
             only a dispatch that fills a volume may"
        );
    }
}
