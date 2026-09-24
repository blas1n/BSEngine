//! Compute skinning: the vertex blend that `bsengine-gltf` used to do on the
//! CPU and re-upload every frame, done on the GPU into the mesh's own vertex
//! buffer.
//!
//! # Why compute, and why nothing else changes
//!
//! Measured in release (`bsengine-gltf`'s `skinning_cost_table`, 2026-09-24),
//! CPU skinning cost 0.04-0.05 ms per animated character per frame, ~90% of
//! it the blend and the `write_buffer` of the deformed vertices: 100
//! characters were 4.2 ms and 300 a whole 60 fps budget. Unity's "GPU
//! (Batched)" mode, Unreal's GPU Skin Cache (on by default since UE5) and
//! Godot 4's skeleton pass all do the same thing to that cost -- a compute
//! dispatch writes the skinned vertices into a buffer, and every render pass
//! reads that buffer exactly as it would an unskinned mesh. That is what this
//! module does: the compute pass writes `GpuMesh::vertex_buffer`, the very
//! buffer the main, cascade-shadow and point-shadow passes already bind, so
//! none of them know skinning happened. The alternative -- skinning in each
//! pass's vertex shader -- would have meant a skinned variant of every one of
//! those pipelines.
//!
//! # What stays on the CPU
//!
//! Composing the joint matrices. It is ~10% of the old cost (0.4 ms for 100
//! characters), and its outputs are read by things that live on the CPU: the
//! IK solver's tip positions, the ragdoll's pose override, retargeting. The
//! GPU receives the finished palette, one `mat4` per joint, per frame.
//!
//! # Layout
//!
//! The rest vertices and the output share [`Vertex`]'s layout -- 11 floats,
//! no padding -- and the shader addresses them as `array<f32>` by index
//! rather than as a struct, because a `vec3<f32>` inside a storage-buffer
//! struct is padded to 16 bytes in WGSL and the `Vertex` the render passes
//! read is not. The per-vertex skin is its own buffer, 32 bytes, 16-aligned.

use crate::mesh::{GpuMeshRegistry, Vertex};
use glam::Mat4;
use std::sync::Arc;
use tracing::warn;
use wgpu::util::DeviceExt;

/// One vertex's skin binding as the compute shader reads it: up to four
/// joint indices into the palette and their weights. A weight of zero means
/// "unused", as it does on the CPU side, so a vertex bound to fewer than four
/// joints simply carries zeros.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVertexSkin {
    /// Joint indices into the palette handed to [`GpuMeshRegistry::skin`].
    pub joints: [u32; 4],
    /// Blend weights, same order as `joints`.
    pub weights: [f32; 4],
}

/// Floats per [`Vertex`], which the shader hardcodes as `11u`: this is the
/// check that keeps the two in step, since a `Vertex` that grew a field
/// would otherwise skin every vertex out of the wrong floats with no error.
const FLOATS_PER_VERTEX: u64 = (std::mem::size_of::<Vertex>() / 4) as u64;
const _: () = assert!(
    FLOATS_PER_VERTEX == 11,
    "the compute skinning shader hardcodes 11 floats per Vertex"
);
/// Threads per workgroup, matching `@workgroup_size` in the shader.
const WORKGROUP: u32 = 64;
/// Bytes of the per-dispatch parameter block (`joint_count`, padded).
const PARAMS_SIZE: u64 = 16;

/// The shader. Mirrors `bsengine-gltf`'s `blend_vertex_position` and
/// `blend_vertex_normal` exactly -- a zero weight is skipped, a joint index
/// past the palette contributes nothing, the normal takes the joint matrix's
/// linear part (correct under uniform scale, the same documented
/// simplification) and is normalised -- so the GPU result is the CPU result
/// to float precision, which is what the equivalence test in `bsengine-gltf`
/// holds it to.
const SHADER: &str = r#"
struct SkinIn {
    joints: vec4<u32>,
    weights: vec4<f32>,
};

struct Params {
    joint_count: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<storage, read> rest: array<f32>;
@group(0) @binding(1) var<storage, read> skins: array<SkinIn>;
@group(0) @binding(2) var<storage, read> palette: array<mat4x4<f32>>;
@group(0) @binding(3) var<storage, read_write> out: array<f32>;
@group(0) @binding(4) var<uniform> params: Params;

const FLOATS_PER_VERTEX: u32 = 11u;

@compute @workgroup_size(64, 1, 1)
fn cs_skin(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= arrayLength(&skins)) {
        return;
    }
    let base = i * FLOATS_PER_VERTEX;
    let p = vec3<f32>(rest[base], rest[base + 1u], rest[base + 2u]);
    let n = vec3<f32>(rest[base + 6u], rest[base + 7u], rest[base + 8u]);
    let s = skins[i];

    var pos = vec3<f32>(0.0);
    var nrm = vec3<f32>(0.0);
    for (var k = 0u; k < 4u; k = k + 1u) {
        let w = s.weights[k];
        if (w == 0.0) {
            continue;
        }
        let j = s.joints[k];
        if (j >= params.joint_count) {
            continue;
        }
        let m = palette[j];
        pos = pos + w * (m * vec4<f32>(p, 1.0)).xyz;
        let linear = mat3x3<f32>(m[0].xyz, m[1].xyz, m[2].xyz);
        nrm = nrm + w * (linear * n);
    }
    let len = length(nrm);
    if (len > 0.0) {
        nrm = nrm / len;
    } else {
        nrm = vec3<f32>(0.0);
    }

    out[base] = pos.x;
    out[base + 1u] = pos.y;
    out[base + 2u] = pos.z;
    out[base + 3u] = rest[base + 3u];
    out[base + 4u] = rest[base + 4u];
    out[base + 5u] = rest[base + 5u];
    out[base + 6u] = nrm.x;
    out[base + 7u] = nrm.y;
    out[base + 8u] = nrm.z;
    out[base + 9u] = rest[base + 9u];
    out[base + 10u] = rest[base + 10u];
}
"#;

/// The one compute pipeline every skinned mesh dispatches through; built on
/// first use and kept on the registry.
pub struct SkinningPipeline {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
}

impl SkinningPipeline {
    fn new(device: &wgpu::Device) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute skinning"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let storage = |binding: u32, read_only: bool| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute skinning"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, true),
                storage(3, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(PARAMS_SIZE),
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("compute skinning"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute skinning"),
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: "cs_skin",
            compilation_options: Default::default(),
            cache: None,
        });
        Self { pipeline, layout }
    }
}

/// What a skinned mesh keeps beside its drawable buffers: the rest pose the
/// blend reads, the per-vertex skin, the palette the CPU writes each frame,
/// and the bind group tying them to the mesh's own vertex buffer as output.
pub struct SkinnedBuffers {
    rest: wgpu::Buffer,
    skin: wgpu::Buffer,
    palette: wgpu::Buffer,
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertex_count: u32,
    /// Joints the palette has room for; a palette longer than this is
    /// refused rather than overrun.
    joint_capacity: u32,
}

impl SkinnedBuffers {
    /// Builds the skinning side of a mesh whose `vertex_buffer` was created
    /// with `STORAGE` usage. `vertices` and `skin` are the same length.
    fn new(
        device: &wgpu::Device,
        pipeline: &SkinningPipeline,
        vertex_buffer: &wgpu::Buffer,
        vertices: &[Vertex],
        skin: &[GpuVertexSkin],
        joint_capacity: u32,
    ) -> Self {
        let rest = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skinning rest vertices"),
            contents: bytemuck::cast_slice(vertices),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let skin_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("skinning per-vertex skin"),
            contents: bytemuck::cast_slice(skin),
            usage: wgpu::BufferUsages::STORAGE,
        });
        // Never empty: a zero-sized storage binding is invalid, and a mesh
        // with no joints still has to bind something. One identity is the
        // harmless minimum.
        let joint_capacity = joint_capacity.max(1);
        let palette = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skinning joint palette"),
            size: u64::from(joint_capacity) * std::mem::size_of::<Mat4>() as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("skinning params"),
            size: PARAMS_SIZE,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute skinning"),
            layout: &pipeline.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: rest.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: skin_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: palette.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: vertex_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        Self {
            rest,
            skin: skin_buffer,
            palette,
            params,
            bind_group,
            vertex_count: vertices.len() as u32,
            joint_capacity,
        }
    }
}

impl GpuMeshRegistry {
    /// Uploads a mesh the GPU will skin: its rest-pose vertices, its
    /// per-vertex skin (one entry per vertex) and its indices, with room in
    /// the palette for `joint_count` joints. Returns the mesh id, which draws
    /// exactly like any other -- the skinned vertices land in the same
    /// buffer the render passes bind.
    ///
    /// Until the first [`GpuMeshRegistry::skin`] the buffer holds the rest
    /// pose, so a character that has not been posed yet draws at rest rather
    /// than as garbage.
    pub fn register_skinned(
        &mut self,
        vertices: &[Vertex],
        skin: &[GpuVertexSkin],
        indices: &[u32],
        joint_count: u32,
    ) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        let mesh = self.build_skinned(vertices, skin, indices, joint_count);
        self.meshes.insert(id, mesh);
        id
    }

    /// [`GpuMeshRegistry::replace`] for a skinned mesh: rebuilds every
    /// buffer under the same id, for a hot reload that changed the geometry,
    /// the skin or the skeleton. Returns whether `id` was registered.
    #[must_use]
    pub fn replace_skinned(
        &mut self,
        id: u64,
        vertices: &[Vertex],
        skin: &[GpuVertexSkin],
        indices: &[u32],
        joint_count: u32,
    ) -> bool {
        if !self.meshes.contains_key(&id) {
            return false;
        }
        let mesh = self.build_skinned(vertices, skin, indices, joint_count);
        self.meshes.insert(id, mesh);
        true
    }

    /// Whether `id` names a mesh registered through
    /// [`GpuMeshRegistry::register_skinned`].
    pub fn is_skinned(&self, id: u64) -> bool {
        self.meshes.get(&id).is_some_and(|m| m.skinned.is_some())
    }

    /// Poses mesh `id` with `joints` -- the finished joint matrices,
    /// `global * inverse_bind` per joint in joint order. The palette is
    /// uploaded now; the dispatch that writes the mesh's vertex buffer is
    /// recorded by the next [`GpuMeshRegistry::flush_skinning`], which the
    /// skinning system calls once per frame after posing every character.
    ///
    /// # Why the dispatch is deferred
    ///
    /// The first version submitted one command buffer per mesh here, and
    /// measured in release that cost 0.028 ms per character -- only a third
    /// less than the CPU blend it replaced, because `queue.submit` is tens of
    /// microseconds and 300 characters meant 300 submits a frame. The blend
    /// itself is nearly free; the submission was the whole bill. Recording
    /// every dispatch into one encoder and submitting once is what actually
    /// takes the cost off the CPU.
    ///
    /// Returns whether the pose was accepted. `false` for an unregistered or
    /// unskinned id, or a palette longer than the mesh was registered with --
    /// refused with a warning rather than written past the buffer.
    pub fn skin(&mut self, queue: &wgpu::Queue, id: u64, joints: &[Mat4]) -> bool {
        let Some(mesh) = self.meshes.get(&id) else {
            return false;
        };
        let Some(skinned) = mesh.skinned.as_ref() else {
            return false;
        };
        if joints.len() as u32 > skinned.joint_capacity {
            warn!(
                "refusing to skin mesh {id} with {} joints; it was registered with room for {}",
                joints.len(),
                skinned.joint_capacity
            );
            return false;
        }
        if !joints.is_empty() {
            let cols: Vec<[[f32; 4]; 4]> = joints.iter().map(|m| m.to_cols_array_2d()).collect();
            queue.write_buffer(&skinned.palette, 0, bytemuck::cast_slice(&cols));
        }
        let params = [joints.len() as u32, 0, 0, 0];
        queue.write_buffer(&skinned.params, 0, bytemuck::cast_slice(&params));
        if !self.pending_skins.contains(&id) {
            self.pending_skins.push(id);
        }
        true
    }

    /// How many meshes have been posed since the last flush -- zero after
    /// the skinning system has run, which is what its test checks, since
    /// [`GpuMeshRegistry::read_back_vertices`] flushes on its own and could
    /// not tell a system that flushed from one that forgot.
    pub fn pending_skins(&self) -> usize {
        self.pending_skins.len()
    }

    /// Dispatches the compute pass for every mesh posed by
    /// [`GpuMeshRegistry::skin`] since the last flush -- one encoder, one
    /// compute pass, one submission -- so the next render pass to bind those
    /// vertex buffers sees the poses. Returns how many meshes were skinned.
    ///
    /// Called once per frame by the skinning system after it has posed every
    /// character; a caller that skins outside that system (a test, a tool)
    /// flushes itself. A mesh that was replaced or never registered since it
    /// was posed is skipped rather than dispatched against buffers that no
    /// longer exist.
    pub fn flush_skinning(&mut self, queue: &wgpu::Queue) -> usize {
        let pending = std::mem::take(&mut self.pending_skins);
        if pending.is_empty() {
            return 0;
        }
        let Some(pipeline) = self.skinning.as_ref() else {
            // Unreachable in practice: `build_skinned` creates the pipeline
            // before any mesh can be pending.
            return 0;
        };
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("compute skinning"),
            });
        let mut dispatched = 0;
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute skinning"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&pipeline.pipeline);
            for id in pending {
                let Some(skinned) = self.meshes.get(&id).and_then(|m| m.skinned.as_ref()) else {
                    continue;
                };
                pass.set_bind_group(0, &skinned.bind_group, &[]);
                pass.dispatch_workgroups(skinned.vertex_count.div_ceil(WORKGROUP), 1, 1);
                dispatched += 1;
            }
        }
        queue.submit(Some(encoder.finish()));
        dispatched
    }

    /// Copies mesh `id`'s vertex buffer back to the CPU and waits for it,
    /// flushing any pending skinning first so the read sees the latest pose.
    ///
    /// For tests and tools only -- a round trip through a staging buffer and
    /// a device wait is the one thing a frame must never do. It is how the
    /// equivalence test in `bsengine-gltf` sees what the compute pass wrote,
    /// which no assertion on the CPU side of the registry could.
    pub fn read_back_vertices(&mut self, queue: &wgpu::Queue, id: u64) -> Option<Vec<Vertex>> {
        self.flush_skinning(queue);
        let mesh = self.meshes.get(&id)?;
        let size = mesh.vertex_buffer.size();
        if size == 0 {
            return Some(Vec::new());
        }
        let staging = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vertex readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("vertex readback"),
            });
        encoder.copy_buffer_to_buffer(&mesh.vertex_buffer, 0, &staging, 0, size);
        queue.submit(Some(encoder.finish()));
        let slice = staging.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv()
            .expect("map_async never reported a result")
            .expect("mapping the vertex readback buffer failed");
        let vertices: Vec<Vertex> = bytemuck::cast_slice(&slice.get_mapped_range()).to_vec();
        staging.unmap();
        Some(vertices)
    }

    fn build_skinned(
        &mut self,
        vertices: &[Vertex],
        skin: &[GpuVertexSkin],
        indices: &[u32],
        joint_count: u32,
    ) -> crate::mesh::GpuMesh {
        assert_eq!(
            vertices.len(),
            skin.len(),
            "a skinned mesh needs exactly one skin entry per vertex"
        );
        let device: Arc<wgpu::Device> = self.device.clone();
        if self.skinning.is_none() {
            self.skinning = Some(SkinningPipeline::new(&device));
        }
        // `COPY_SRC` for `read_back_vertices`; `STORAGE` is what lets the
        // compute pass write the same buffer the render passes bind.
        let mut mesh = self.build_with_usage(
            vertices,
            indices,
            wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::STORAGE,
        );
        let pipeline = self
            .skinning
            .as_ref()
            .expect("created just above when absent");
        mesh.skinned = Some(SkinnedBuffers::new(
            &device,
            pipeline,
            &mesh.vertex_buffer,
            vertices,
            skin,
            joint_count,
        ));
        mesh
    }
}

// Silences the "field never read" lint for the buffers whose only job is to
// be alive for the bind group that references them.
impl SkinnedBuffers {
    #[allow(dead_code)]
    fn keep_alive(&self) -> (&wgpu::Buffer, &wgpu::Buffer) {
        (&self.rest, &self.skin)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;
    use std::sync::OnceLock;

    /// One device for every test here -- see `CLAUDE.md` on the Windows CI
    /// device budget.
    fn device_and_queue() -> (Arc<wgpu::Device>, Arc<wgpu::Queue>) {
        static SHARED: OnceLock<(Arc<wgpu::Device>, Arc<wgpu::Queue>)> = OnceLock::new();
        SHARED
            .get_or_init(|| {
                let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                    backends: wgpu::Backends::all(),
                    ..Default::default()
                });
                let adapter =
                    pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                        power_preference: wgpu::PowerPreference::None,
                        compatible_surface: None,
                        force_fallback_adapter: false,
                    }))
                    .expect("a headless adapter; the rest of this crate's tests need one too");
                let (device, queue) = pollster::block_on(adapter.request_device(
                    &wgpu::DeviceDescriptor {
                        label: Some("compute skinning tests"),
                        required_features: wgpu::Features::empty(),
                        required_limits: wgpu::Limits::downlevel_defaults(),
                        memory_hints: wgpu::MemoryHints::default(),
                    },
                    None,
                ))
                .expect("headless device");
                (Arc::new(device), Arc::new(queue))
            })
            .clone()
    }

    fn vertex(position: [f32; 3], normal: [f32; 3]) -> Vertex {
        Vertex {
            position,
            color: [0.25, 0.5, 0.75],
            normal,
            uv: [0.125, 0.875],
        }
    }

    fn close(a: [f32; 3], b: [f32; 3]) -> bool {
        a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-5)
    }

    /// Two joints, three vertices bound three ways -- one joint, the other
    /// joint, half and half -- against a palette that moves joint 1 and
    /// rotates its normals. Expected values are written down from the
    /// definition of linear blend skinning, not computed by the code under
    /// test. Colour and UV must come through untouched.
    #[test]
    fn a_dispatch_blends_positions_and_normals_by_weight_and_copies_the_rest() {
        let (device, queue) = device_and_queue();
        let mut registry = GpuMeshRegistry::new(device);
        let vertices = vec![
            vertex([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            vertex([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
            vertex([1.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
        ];
        let skin = vec![
            GpuVertexSkin {
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            GpuVertexSkin {
                joints: [1, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            GpuVertexSkin {
                joints: [0, 1, 0, 0],
                weights: [0.5, 0.5, 0.0, 0.0],
            },
        ];
        let id = registry.register_skinned(&vertices, &skin, &[0, 1, 2], 2);
        assert!(registry.is_skinned(id));

        // Joint 1: translate by (0, 10, 0) and rotate 90 degrees about Z, so
        // a +Y normal becomes -X and the position (1, 0, 0) becomes (0, 1, 0)
        // before the translation.
        let joint1 = Mat4::from_translation(Vec3::new(0.0, 10.0, 0.0))
            * Mat4::from_rotation_z(std::f32::consts::FRAC_PI_2);
        assert!(registry.skin(&queue, id, &[Mat4::IDENTITY, joint1]));

        let out = registry.read_back_vertices(&queue, id).expect("registered");
        assert_eq!(out.len(), 3);
        assert!(
            close(out[0].position, [1.0, 0.0, 0.0]) && close(out[0].normal, [0.0, 1.0, 0.0]),
            "joint 0 is identity: {:?}",
            out[0]
        );
        assert!(
            close(out[1].position, [0.0, 11.0, 0.0]) && close(out[1].normal, [-1.0, 0.0, 0.0]),
            "joint 1 rotates then translates: {:?}",
            out[1]
        );
        // Half of each: position (0.5, 5.5, 0), normal the normalised mean of
        // +Y and -X.
        let d = std::f32::consts::FRAC_1_SQRT_2;
        assert!(
            close(out[2].position, [0.5, 5.5, 0.0]) && close(out[2].normal, [-d, d, 0.0]),
            "half and half: {:?}",
            out[2]
        );
        for v in &out {
            assert_eq!(v.color, [0.25, 0.5, 0.75], "colour passes through");
            assert_eq!(v.uv, [0.125, 0.875], "uv passes through");
        }
    }

    /// The buffer holds the rest pose until the first skin, and a palette
    /// longer than the mesh was registered for is refused rather than
    /// written past the buffer.
    #[test]
    fn rest_pose_until_skinned_and_an_oversized_palette_is_refused() {
        let (device, queue) = device_and_queue();
        let mut registry = GpuMeshRegistry::new(device);
        let vertices = vec![vertex([3.0, 4.0, 5.0], [0.0, 0.0, 1.0])];
        let skin = vec![GpuVertexSkin {
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }];
        let id = registry.register_skinned(&vertices, &skin, &[0], 1);
        let before = registry.read_back_vertices(&queue, id).unwrap();
        assert!(
            close(before[0].position, [3.0, 4.0, 5.0]),
            "rest pose: {:?}",
            before[0]
        );

        assert!(
            !registry.skin(&queue, id, &[Mat4::IDENTITY, Mat4::IDENTITY]),
            "two joints into a one-joint palette must be refused"
        );
        assert!(
            !registry.skin(&queue, 999, &[Mat4::IDENTITY]),
            "an unregistered id is refused"
        );
        let plain = registry.register(&vertices, &[0]);
        assert!(!registry.is_skinned(plain));
        assert!(
            !registry.skin(&queue, plain, &[Mat4::IDENTITY]),
            "an unskinned mesh is refused"
        );
    }

    /// A joint index past the palette contributes nothing, like the CPU's
    /// `joint_matrices.get(j)` returning `None` -- rather than reading
    /// whatever the buffer holds there.
    #[test]
    fn a_joint_past_the_palette_contributes_nothing() {
        let (device, queue) = device_and_queue();
        let mut registry = GpuMeshRegistry::new(device);
        let vertices = vec![vertex([1.0, 2.0, 3.0], [1.0, 0.0, 0.0])];
        // Weight split between joint 0 and joint 7; the palette has room
        // for 8 but only one matrix is handed over, so joint 7 is "past".
        let skin = vec![GpuVertexSkin {
            joints: [0, 7, 0, 0],
            weights: [0.5, 0.5, 0.0, 0.0],
        }];
        let id = registry.register_skinned(&vertices, &skin, &[0], 8);
        // Fill every palette slot first, so slot 7 holds an identity rather
        // than the zeros a fresh buffer starts with: a shader that skipped
        // the bound check would then add joint 7's half back in, whereas
        // against zeros it would contribute nothing and pass by accident.
        assert!(registry.skin(&queue, id, &[Mat4::IDENTITY; 8]));
        assert!(registry.skin(&queue, id, &[Mat4::IDENTITY]));
        let out = registry.read_back_vertices(&queue, id).unwrap();
        assert!(
            close(out[0].position, [0.5, 1.0, 1.5]),
            "only joint 0's half survives: {:?}",
            out[0]
        );
        assert!(
            close(out[0].normal, [1.0, 0.0, 0.0]),
            "normalised: {:?}",
            out[0]
        );
    }

    /// Replacing under the same id rebuilds the skinning side too: a
    /// reload that changed the skeleton skins with the new palette size and
    /// the new rest pose.
    #[test]
    fn replace_skinned_rebuilds_the_skinning_buffers_under_the_same_id() {
        let (device, queue) = device_and_queue();
        let mut registry = GpuMeshRegistry::new(device);
        let skin1 = vec![GpuVertexSkin {
            joints: [0, 0, 0, 0],
            weights: [1.0, 0.0, 0.0, 0.0],
        }];
        let id =
            registry.register_skinned(&[vertex([1.0, 0.0, 0.0], [0.0, 1.0, 0.0])], &skin1, &[0], 1);
        let skin2 = vec![
            GpuVertexSkin {
                joints: [1, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
            GpuVertexSkin {
                joints: [0, 0, 0, 0],
                weights: [1.0, 0.0, 0.0, 0.0],
            },
        ];
        assert!(registry.replace_skinned(
            id,
            &[
                vertex([2.0, 0.0, 0.0], [0.0, 1.0, 0.0]),
                vertex([3.0, 0.0, 0.0], [0.0, 1.0, 0.0])
            ],
            &skin2,
            &[0, 1],
            2
        ));
        assert!(registry.skin(
            &queue,
            id,
            &[
                Mat4::IDENTITY,
                Mat4::from_translation(Vec3::new(0.0, 0.0, 5.0))
            ]
        ));
        let out = registry.read_back_vertices(&queue, id).unwrap();
        assert_eq!(out.len(), 2, "the new vertex count");
        assert!(close(out[0].position, [2.0, 0.0, 5.0]), "{:?}", out[0]);
        assert!(close(out[1].position, [3.0, 0.0, 0.0]), "{:?}", out[1]);
        assert!(
            !registry.replace_skinned(4242, &[], &[], &[], 1),
            "an unregistered id is refused"
        );
    }
}
