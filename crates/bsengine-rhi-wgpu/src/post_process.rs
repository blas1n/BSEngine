/// Texture format used for the HDR scene-color render target, before tonemapping.
pub const HDR_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const BLOOM_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;
const AO_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;
const CONFIG_SIZE: u64 = 64;
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
const FOG_UNIFORM_SIZE: u64 = 352;

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
    light_view_proj: mat4x4<f32>,
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
    pad0: f32,
    enabled: u32,
    frame_index: u32,
    history_valid: u32,
    pad1: f32,
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
    _pad0: f32,
    color: vec3<f32>,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}
struct SpotLightEntry {
    position: vec3<f32>,
    _pad0: f32,
    direction: vec3<f32>,
    inner_cos: f32,
    color: vec3<f32>,
    outer_cos: f32,
    intensity: f32,
    range: f32,
    _pad1: f32,
    _pad2: f32,
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
@group(2) @binding(0) var shadow_map: texture_depth_2d;
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
fn directional_shadow_factor(world_pos: vec3<f32>) -> f32 {
    let lsp = fog.light_view_proj * vec4<f32>(world_pos, 1.0);
    let proj = lsp.xyz / lsp.w;
    let uv = proj.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5, 0.5);
    let depth = proj.z;
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0
        || depth < 0.0 || depth > 1.0) {
        return 1.0;
    }
    return textureSampleCompareLevel(shadow_map, shadow_sampler, uv, depth - 0.003);
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
            in_scatter += pl.color * (pl.intensity * t * t) * scattering * pt_phase * pt_lit;
        }
    }

    // Spot lights, likewise mirroring the scene shader: the same cone
    // `smoothstep(outer_cos, inner_cos, cos_angle)` and the same
    // `intensity * t * t * spot_factor`. No shadow term, for the same reason
    // the scene shader has none -- spot lights have no shadow map at all in
    // this renderer, so inventing one here would make the fog claim an
    // occlusion the lit surfaces below it do not show.
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
                in_scatter += sl.color * (sl.intensity * t * t * spot_factor)
                    * scattering * sp_phase;
            }
        }
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
fn fog_inject_wgsl() -> String {
    froxel_wgsl_preamble()
        + &fog_uniform_wgsl(0)
        + FROXEL_COMMON_WGSL
        + FROXEL_SHADOW_WGSL
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
    let pos_r = reconstruct_view_pos(in.uv + vec2<f32>(texel.x, 0.0), load_depth(in.uv + vec2<f32>(texel.x, 0.0)));
    let pos_u = reconstruct_view_pos(in.uv + vec2<f32>(0.0, texel.y), load_depth(in.uv + vec2<f32>(0.0, texel.y)));
    let normal = normalize(cross(pos_r - pos, pos_u - pos));
    var hemisphere: array<vec3<f32>, 8> = array<vec3<f32>, 8>(
        vec3<f32>( 0.5411,  0.5,     0.5),
        vec3<f32>(-0.5411,  0.5,     0.5),
        vec3<f32>( 0.5,    -0.5411,  0.5),
        vec3<f32>(-0.5,    -0.5411,  0.5),
        vec3<f32>( 0.7071,  0.0,     0.3),
        vec3<f32>(-0.7071,  0.0,     0.3),
        vec3<f32>( 0.0,     0.7071,  0.3),
        vec3<f32>( 0.0,    -0.7071,  0.3),
    );
    var occlusion = 0.0;
    let max_samples = min(config.ssao_sample_count, 8u);
    for (var i: u32 = 0u; i < max_samples; i++) {
        let s = normalize(hemisphere[i] + normal * 0.5);
        let sample_pos = pos + s * config.ssao_radius;
        let clip = cam.proj * vec4<f32>(sample_pos, 1.0);
        if clip.w <= 0.0 { continue; }
        let ndc = clip.xyz / clip.w;
        let sample_uv = vec2<f32>(ndc.x * 0.5 + 0.5, -ndc.y * 0.5 + 0.5);
        if sample_uv.x < 0.0 || sample_uv.x > 1.0 || sample_uv.y < 0.0 || sample_uv.y > 1.0 {
            continue;
        }
        let actual_pos = reconstruct_view_pos(sample_uv, load_depth(sample_uv));
        let range_check = smoothstep(0.0, 1.0, config.ssao_radius / max(abs(pos.z - actual_pos.z), 0.0001));
        if actual_pos.z >= sample_pos.z + config.ssao_bias {
            occlusion += range_check;
        }
    }
    let ao = 1.0 - (occlusion / f32(max_samples)) * config.ssao_intensity;
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
}
@group(0) @binding(0) var hdr_tex: texture_2d<f32>;
@group(0) @binding(1) var hdr_sampler: sampler;
@group(1) @binding(0) var bloom_tex: texture_2d<f32>;
@group(1) @binding(1) var bloom_sampler: sampler;
@group(2) @binding(0) var ao_tex: texture_2d<f32>;
@group(2) @binding(1) var ao_sampler: sampler;
@group(3) @binding(0) var<uniform> config: PostProcessConfig;

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

fn apply_tonemap(color: vec3<f32>) -> vec3<f32> {
    let scaled = color * pow(2.0, config.tonemap_exposure);
    if config.tonemap_enabled == 0u { return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0)); }
    if config.tonemap_mode == 0u { return clamp(scaled, vec3<f32>(0.0), vec3<f32>(1.0)); }
    if config.tonemap_mode == 1u { return reinhard(scaled); }
    if config.tonemap_mode == 2u { return reinhard_lum(scaled); }
    if config.tonemap_mode == 4u { return filmic(scaled); }
    return aces(scaled);
}

@fragment
fn fs_composite(in: FullscreenOut) -> @location(0) vec4<f32> {
    let hdr   = textureSample(hdr_tex,   hdr_sampler,   in.uv).rgb;
    let bloom = textureSample(bloom_tex, bloom_sampler, in.uv).rgb;
    let ao    = textureSample(ao_tex,    ao_sampler,    in.uv).r;
    let combined = hdr * ao + bloom;
    return vec4<f32>(apply_tonemap(combined), 1.0);
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
    let prev_uv = vec2<f32>(prev_ndc.x * 0.5 + 0.5, 0.5 - prev_ndc.y * 0.5);

    // History from outside the previous frame simply does not exist.
    if prev_uv.x < 0.0 || prev_uv.x > 1.0 || prev_uv.y < 0.0 || prev_uv.y > 1.0 {
        return taa_out(current);
    }

    // `textureSampleLevel`, not `textureSample`: this read sits under the
    // per-pixel branches above, and an implicit-derivative sample there is
    // non-uniform control flow. The history texture has a single mip, so
    // level 0 is the only level either call could have read.
    let history = textureSampleLevel(history_tex, history_sampler, prev_uv, 0.0).rgb;

    // Clamp history into the local range. This is what stops a moving
    // object -- which this version reprojects incorrectly, by design --
    // from smearing across the frame.
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
    /// The directional shadow map's view-projection -- the same matrix the
    /// scene shader's camera uniform carries as `light_view_proj`.
    ///
    /// It lives here rather than being read out of the shared light uniform
    /// because it is not in there: on the scene side it belongs to the *camera*
    /// uniform, next to `view_proj`, and this pass already carries its own
    /// camera matrices for the same reason (they are the jittered ones).
    pub light_view_proj: [[f32; 4]; 4],
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
    /// Padding to the 16-byte uniform stride. See `fog_uniform_size`.
    pub _pad0: f32,
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
    /// Padding to the 16-byte uniform stride. See `fog_uniform_size`.
    pub _pad1: f32,
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
    ldr_texture: crate::profiler::TrackedTexture,
    ldr_view: wgpu::TextureView,
    ldr_bg: wgpu::BindGroup,
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
    /// LDR output of the composite pass. Composite used to write straight to
    /// the swapchain; it writes here instead so the TAA pass has a readable
    /// copy of this frame's finished colour -- a pass cannot sample the
    /// texture it is rendering into.
    ldr_view: wgpu::TextureView,
    _ldr_texture: crate::profiler::TrackedTexture,
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
    ldr_bg: wgpu::BindGroup,
    depth_bg: wgpu::BindGroup,
    sampler: wgpu::Sampler,
    config_buffer: wgpu::Buffer,
    config_bg: wgpu::BindGroup,
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
    fog_inject_pipeline: wgpu::ComputePipeline,
    /// Compute pipeline that marches each froxel column front to back.
    fog_integrate_pipeline: wgpu::ComputePipeline,
    /// Pipeline for the volumetric-fog apply shader, the first pass of the
    /// chain: it composites the integrated volume onto the scene's HDR image.
    /// An exact passthrough while `fog.enabled` is zero.
    pub fog_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the bright-pass bloom extraction shader.
    pub bloom_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the SSAO occlusion shader.
    pub ssao_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the final composite (HDR + bloom + AO, tonemapped) shader.
    pub composite_pipeline: wgpu::RenderPipeline,
    /// Pipeline for the temporal-antialiasing resolve. Writes both the
    /// swapchain image and next frame's history target.
    pub taa_pipeline: wgpu::RenderPipeline,
    tex2d_bgl: wgpu::BindGroupLayout,
    depth_bgl: wgpu::BindGroupLayout,
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

        let tex2d_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let depth_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp depth bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Depth,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }],
        });

        let config_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let ssao_cam_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let taa_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let fog_uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let froxel_shadow_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("pp froxel shadow bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
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
            ],
        });
        let froxel_write_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let froxel_read_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let froxel_apply_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
        let composite_pipeline =
            Self::make_composite_pipeline(device, &tex2d_bgl, &config_bgl, surface_format);
        let taa_pipeline = Self::make_taa_pipeline(
            device,
            &tex2d_bgl,
            &depth_bgl,
            &taa_uniform_bgl,
            surface_format,
        );

        let targets = Self::create_targets(
            device,
            width,
            height,
            depth_view,
            &sampler,
            &tex2d_bgl,
            &depth_bgl,
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
            ldr_view: targets.ldr_view,
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
            ldr_bg: targets.ldr_bg,
            depth_bg: targets.depth_bg,
            sampler,
            config_buffer,
            config_bg,
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
            composite_pipeline,
            taa_pipeline,
            tex2d_bgl,
            depth_bgl,
            surface_format,
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

        let hdr_texture = make_tex("pp hdr", HDR_FORMAT);
        let hdr_view = hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // Same format and size as `hdr_texture`: the fog pass copies the scene
        // image through it, so anything narrower would requantize the frame.
        let fog_hdr_texture = make_tex("pp fog hdr", HDR_FORMAT);
        let fog_hdr_view = fog_hdr_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bloom_texture = make_tex("pp bloom", BLOOM_FORMAT);
        let bloom_view = bloom_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let ao_texture = make_tex("pp ao", AO_FORMAT);
        let ao_view = ao_texture.create_view(&wgpu::TextureViewDescriptor::default());
        // `surface_format`, not HDR_FORMAT: this holds the already-tonemapped
        // LDR image on its way to the swapchain, so matching the swapchain's
        // format keeps the round trip through it exact.
        let ldr_texture = make_tex("pp ldr", surface_format);
        let ldr_view = ldr_texture.create_view(&wgpu::TextureViewDescriptor::default());
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
        let ldr_bg = make_tex2d_bg("pp ldr bg", &ldr_view);
        let history_bgs = [
            make_tex2d_bg("pp history bg 0", &history_views[0]),
            make_tex2d_bg("pp history bg 1", &history_views[1]),
        ];

        let depth_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("pp depth bg"),
            layout: depth_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(depth_view),
            }],
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
            ldr_texture,
            ldr_view,
            ldr_bg,
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
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fog inject shader"),
            source: wgpu::ShaderSource::Wgsl(fog_inject_wgsl().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fog inject pll"),
            bind_group_layouts: &[
                fog_uniform_bgl,
                froxel_write_bgl,
                froxel_shadow_bgl,
                froxel_history_bgl,
            ],
            push_constant_ranges: &[],
        });
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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
    ) -> wgpu::ComputePipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fog integrate shader"),
            source: wgpu::ShaderSource::Wgsl(fog_integrate_wgsl().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fog integrate pll"),
            bind_group_layouts: &[fog_uniform_bgl, froxel_read_bgl, froxel_write_bgl],
            push_constant_ranges: &[],
        });
        device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
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
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("fog shader"),
            source: wgpu::ShaderSource::Wgsl(fog_apply_wgsl().into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("fog pll"),
            bind_group_layouts: &[tex2d_bgl, depth_bgl, froxel_apply_bgl, fog_uniform_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bloom shader"),
            source: wgpu::ShaderSource::Wgsl(BLOOM_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("bloom pll"),
            bind_group_layouts: &[tex2d_bgl, config_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

    fn make_ssao_pipeline(
        device: &wgpu::Device,
        depth_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
        ssao_cam_bgl: &wgpu::BindGroupLayout,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ssao shader"),
            source: wgpu::ShaderSource::Wgsl(SSAO_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ssao pll"),
            bind_group_layouts: &[depth_bgl, config_bgl, ssao_cam_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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

    fn make_composite_pipeline(
        device: &wgpu::Device,
        tex2d_bgl: &wgpu::BindGroupLayout,
        config_bgl: &wgpu::BindGroupLayout,
        surface_format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("composite shader"),
            source: wgpu::ShaderSource::Wgsl(COMPOSITE_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("composite pll"),
            bind_group_layouts: &[tex2d_bgl, tex2d_bgl, tex2d_bgl, config_bgl],
            push_constant_ranges: &[],
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
    ) -> wgpu::RenderPipeline {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("taa shader"),
            source: wgpu::ShaderSource::Wgsl(TAA_WGSL.into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("taa pll"),
            bind_group_layouts: &[tex2d_bgl, tex2d_bgl, depth_bgl, taa_uniform_bgl],
            push_constant_ranges: &[],
        });
        let color_target = Some(wgpu::ColorTargetState {
            format: surface_format,
            blend: Some(wgpu::BlendState::REPLACE),
            write_mask: wgpu::ColorWrites::ALL,
        });
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
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
            self.surface_format,
        );
        self.hdr_view = t.hdr_view;
        self._hdr_texture = t.hdr_texture;
        self.fog_hdr_view = t.fog_hdr_view;
        self._fog_hdr_texture = t.fog_hdr_texture;
        self.bloom_view = t.bloom_view;
        self._bloom_texture = t.bloom_texture;
        self.ao_view = t.ao_view;
        self._ao_texture = t.ao_texture;
        self.ldr_view = t.ldr_view;
        self._ldr_texture = t.ldr_texture;
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
    }

    /// Uploads new bloom/tonemap/SSAO settings to the config uniform buffer.
    pub fn update_config(&self, queue: &wgpu::Queue, config: PostProcessConfigGpu) {
        queue.write_buffer(&self.config_buffer, 0, bytemuck::cast_slice(&[config]));
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
                pass.set_bind_group(0, &self.fog_hdr_bg, &[]);
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
            pass.set_bind_group(0, &self.fog_hdr_bg, &[]);
            pass.set_bind_group(1, &self.bloom_bg, &[]);
            pass.set_bind_group(2, &self.ao_bg, &[]);
            pass.set_bind_group(3, &self.config_bg, &[]);
            pass.draw(0..3, 0..1);
            draw_calls += 1;
            triangles += 1;
        }

        {
            // Without a valid history the source is this frame's own LDR, so
            // the blend degenerates to `mix(current, current, blend)` -- a
            // no-op -- instead of reading an uninitialised texture.
            let history_src = if self.history_valid {
                &self.history_bgs[self.history_read]
            } else {
                &self.ldr_bg
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
            pass.set_bind_group(0, &self.ldr_bg, &[]);
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

    #[test]
    fn config_gpu_size() {
        assert_eq!(std::mem::size_of::<PostProcessConfigGpu>(), 64);
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
            let shadow_view = shadow_texture.create_view(&wgpu::TextureViewDescriptor::default());
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
            Self {
                _shadow_texture: shadow_texture,
                shadow_view,
                shadow_sampler,
                _point_texture: point_texture,
                point_view,
                light_buffer,
            }
        }

        fn bindings(&self) -> FroxelShadowBindings<'_> {
            FroxelShadowBindings {
                shadow_map_view: &self.shadow_view,
                shadow_sampler: &self.shadow_sampler,
                point_shadow_view: &self.point_view,
                light_buffer: &self.light_buffer,
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
                light_view_proj: glam::Mat4::orthographic_rh(-30.0, 30.0, -30.0, 30.0, 0.1, 200.0)
                    .to_cols_array_2d(),
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
                _pad0: 0.0,
                enabled: 1,
                frame_index: 0,
                history_valid: 0,
                _pad1: 0.0,
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
        assert!(
            pollster::block_on(device.pop_error_scope()).is_none(),
            "the froxel injection and integration passes must record cleanly; \
             a validation error here means the pipeline layouts, the bind \
             groups, or the storage-texture formats disagree with the WGSL"
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
