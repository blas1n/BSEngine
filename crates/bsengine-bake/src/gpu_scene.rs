//! The GI scene as a GPU tracer reads it, shared by every one: the lightmap
//! bake ([`crate::GpuBaker`]) and the renderer's realtime probe update.
//!
//! [`TRACE_WGSL`] is the tracing half of the shader -- the tree, the
//! triangles, the light formulas, the sky -- and the types here are the bytes
//! it reads. A consumer appends its own entry point and declares the globals
//! the modules name (`nodes`, `tris`, `scene`, `sky`; see each `.wgsl` file's
//! header) under whatever group and bindings it uses, and the instance hooks
//! `traverse` calls ([`NO_INSTANCES_WGSL`] for none). Both tracers then trace
//! the same tree with the same formulas: there is one copy of each, so the
//! bake and the realtime GI cannot drift apart.
//!
//! Nothing here blocks or spawns threads, so it builds for the browser too.

use crate::bvh::Bvh;
use crate::scene::{BakeLight, BakeTriangle, Contribution, Sky};
use glam::Vec3;

/// The tracing half of every GI shader: `common.wgsl` (constants, random
/// numbers, directions), `trace.wgsl` (the tree) and `light.wgsl` (lights and
/// sky), in that order.
pub const TRACE_WGSL: &str = concat!(
    include_str!("common.wgsl"),
    include_str!("trace.wgsl"),
    include_str!("light.wgsl"),
);

/// A consumer with no instances -- the bake: `trace.wgsl`'s instance hooks,
/// answering that there are none. Appended after [`TRACE_WGSL`] by every
/// consumer that traces the scene's own tree alone; the realtime GI defines
/// the hooks itself, over its moving meshes.
pub const NO_INSTANCES_WGSL: &str = r#"
fn instance_count() -> u32 {
    return 0u;
}
fn instance_ray(i: u32, o: vec3<f32>, d: vec3<f32>, t_max: f32) -> InstanceRay {
    return InstanceRay(0u, 0u, o, d, false);
}
"#;

/// Most lights a scene carries: the `Scene` uniform's fixed array.
pub const MAX_GPU_LIGHTS: usize = 64;

/// `Node` in `trace.wgsl`: an interior node (`count == 0`) has its first child
/// next and its second at `start`; a leaf's triangles are `tris[start..start +
/// count]`.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuNode {
    lo: [f32; 3],
    start: u32,
    hi: [f32; 3],
    count: u32,
}

/// `Tri` in `trace.wgsl`: corners, the shading normal, albedo and emissive.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuTri {
    p0: [f32; 4],
    p1: [f32; 4],
    p2: [f32; 4],
    n: [f32; 4],
    albedo: [f32; 4],
    emissive: [f32; 4],
}

/// One light: `kind` 0 directional (`a` = direction, `c` = colour), 1 point
/// (`a` = position, `c` = colour), 2 spot (`a` position, `b` direction, `c`
/// colour, `d.x` inner cos, `d.y` outer cos), 3 rect (`a` centre, `b` half
/// width, `c` half height, `d` radiance).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuLight {
    kind: u32,
    contribution: u32,
    range: f32,
    intensity: f32,
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

/// `Scene` in `light.wgsl`: how much tree there is, the lights, the sky.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuSceneUniform {
    light_count: u32,
    node_count: u32,
    sky_kind: u32,
    sky_w: u32,
    sky_h: u32,
    _p0: u32,
    _p1: u32,
    _p2: u32,
    sky_uniform: [f32; 4],
    lights: [GpuLight; MAX_GPU_LIGHTS],
}

/// The tree and its triangles, ready to upload as the `nodes` and `tris`
/// storage buffers. Neither is ever empty -- a storage binding cannot be --
/// so an empty scene carries one zeroed placeholder of each, which the shader
/// never reads because [`Self::node_count`] is 0.
pub struct GpuGeometry {
    pub nodes: Vec<GpuNode>,
    pub tris: Vec<GpuTri>,
    /// The real node count, for [`scene_uniform`].
    pub node_count: u32,
}

impl GpuGeometry {
    /// Builds the tree over `triangles` and lays both out for the GPU.
    pub fn build(triangles: &[BakeTriangle]) -> Self {
        let bvh = Bvh::build(&triangles.iter().map(|t| t.positions).collect::<Vec<_>>());
        Self::from_bvh(&bvh, triangles)
    }

    /// [`Self::build`] over a tree already built for `triangles` (the bake
    /// builds one for its CPU half anyway).
    pub(crate) fn from_bvh(bvh: &Bvh, triangles: &[BakeTriangle]) -> Self {
        // The shader's traversal stack is sized for `MAX_DEPTH`; the builder
        // promises no deeper, and this is where a broken promise would show.
        debug_assert!(bvh.depth() <= crate::bvh::MAX_DEPTH + 1);
        let (flat_nodes, order) = bvh.flat();
        let mut nodes: Vec<GpuNode> = flat_nodes
            .iter()
            .map(|&(lo, start, hi, count)| GpuNode {
                lo,
                start,
                hi,
                count,
            })
            .collect();
        let node_count = nodes.len() as u32;
        if nodes.is_empty() {
            nodes.push(bytemuck::Zeroable::zeroed());
        }
        // In the tree's leaf order, so a leaf names its triangles directly and
        // the index buffer the CPU tree keeps is not needed: one storage
        // buffer fewer, against a baseline of four.
        let mut tris: Vec<GpuTri> = order
            .iter()
            .map(|&i| {
                let t = &triangles[i as usize];
                let [a, b, c] = t.positions;
                // `trace.rs`'s shading normal: the geometric one, turned to
                // the side the vertex normals face.
                let mut n = (b - a).cross(c - a).normalize_or_zero();
                if n.dot(t.normals[0] + t.normals[1] + t.normals[2]) < 0.0 {
                    n = -n;
                }
                GpuTri {
                    p0: v4(a),
                    p1: v4(b),
                    p2: v4(c),
                    n: v4(n),
                    albedo: v4(t.albedo),
                    emissive: v4(t.emissive),
                }
            })
            .collect();
        if tris.is_empty() {
            tris.push(bytemuck::Zeroable::zeroed());
        }
        Self {
            nodes,
            tris,
            node_count,
        }
    }
}

/// The sky as the `sky` texture: `width` x `height` Rgba32Float texels, row
/// by row. A uniform sky is a 1 x 1 placeholder the shader never reads.
pub struct GpuSkyImage {
    pub width: u32,
    pub height: u32,
    pub texels: Vec<[f32; 4]>,
}

/// The `Scene` uniform and the sky texture's contents, for `node_count` nodes
/// (from [`GpuGeometry`]). Lights past [`MAX_GPU_LIGHTS`] are dropped;
/// [`GpuSceneUniform::light_count`] says how many were kept.
pub fn scene_uniform(
    node_count: u32,
    lights: &[BakeLight],
    sky: &Sky,
) -> (GpuSceneUniform, GpuSkyImage) {
    let mut uniform = <GpuSceneUniform as bytemuck::Zeroable>::zeroed();
    for (slot, l) in uniform.lights.iter_mut().zip(lights) {
        *slot = gpu_light(l);
    }
    uniform.light_count = lights.len().min(MAX_GPU_LIGHTS) as u32;
    uniform.node_count = node_count;
    let image = match sky {
        Sky::Uniform(l) => {
            uniform.sky_kind = 0;
            uniform.sky_uniform = v4(*l);
            GpuSkyImage {
                width: 1,
                height: 1,
                texels: vec![[0.0; 4]],
            }
        }
        Sky::Equirect {
            width,
            height,
            texels,
        } => {
            uniform.sky_kind = 1;
            GpuSkyImage {
                width: *width,
                height: *height,
                texels: texels.iter().map(|t| [t.x, t.y, t.z, 1.0]).collect(),
            }
        }
    };
    uniform.sky_w = image.width;
    uniform.sky_h = image.height;
    (uniform, image)
}

impl GpuSceneUniform {
    /// How many lights the uniform carries.
    pub fn light_count(&self) -> u32 {
        self.light_count
    }
}

/// The layout entry for the `sky` texture. Spelled out because a layout
/// wgpu infers from the shader (`layout: None`) would ask for a *filterable*
/// float texture, and `Rgba32Float` is not one: the bind group then fails
/// validation. Every consumer takes this one rather than finding that out.
pub fn sky_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: false },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

/// A read-only (or read-write) storage buffer entry, for `nodes`, `tris` and
/// a consumer's own buffers.
pub fn storage_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
    read_only: bool,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

/// A uniform buffer entry, for `scene` and a consumer's own parameters.
pub fn uniform_layout_entry(
    binding: u32,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn v4(v: Vec3) -> [f32; 4] {
    [v.x, v.y, v.z, 0.0]
}

fn gpu_light(light: &BakeLight) -> GpuLight {
    let contribution = u32::from(light.contribution() == Contribution::Baked);
    let zero = [0.0; 4];
    match *light {
        BakeLight::Directional {
            direction, color, ..
        } => GpuLight {
            kind: 0,
            contribution,
            range: 0.0,
            intensity: 0.0,
            a: v4(direction),
            b: zero,
            c: v4(color),
            d: zero,
        },
        BakeLight::Point {
            position,
            color,
            intensity,
            range,
            ..
        } => GpuLight {
            kind: 1,
            contribution,
            range,
            intensity,
            a: v4(position),
            b: zero,
            c: v4(color),
            d: zero,
        },
        BakeLight::Spot {
            position,
            direction,
            color,
            intensity,
            range,
            inner_cos,
            outer_cos,
            ..
        } => GpuLight {
            kind: 2,
            contribution,
            range,
            intensity,
            a: v4(position),
            b: v4(direction),
            c: v4(color),
            d: [inner_cos, outer_cos, 0.0, 0.0],
        },
        BakeLight::Rect {
            position,
            half_width,
            half_height,
            radiance,
            range,
            ..
        } => GpuLight {
            kind: 3,
            contribution,
            range,
            intensity: 0.0,
            a: v4(position),
            b: v4(half_width),
            c: v4(half_height),
            d: v4(radiance),
        },
    }
}

// `Scene` in light.wgsl: a 48-byte header, then 64 lights of 80 bytes each
// (the uniform array stride, already a multiple of 16).
const _: () = assert!(std::mem::size_of::<GpuLight>() == 80);
const _: () = assert!(std::mem::size_of::<GpuSceneUniform>() == 48 + 80 * MAX_GPU_LIGHTS);
