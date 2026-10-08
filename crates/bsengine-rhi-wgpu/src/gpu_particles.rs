//! GPU-simulated particles: emitters whose particles live in GPU memory and
//! are emitted, moved and aged by a compute shader.
//!
//! The CPU path (`bsengine-app`'s particle tick) owns every particle in a
//! `Vec` and uploads them each frame, which caps an emitter at a few thousand.
//! This is the other half of the choice Niagara, the VFX Graph and Godot's
//! `GPUParticles3D` offer: the same parameters, simulated where the particles
//! are drawn, so an emitter can hold hundreds of thousands and nothing crosses
//! the bus but a few dozen bytes of parameters per step.
//!
//! # Layout
//!
//! Each emitter has a ring of `capacity` slots in two buffers:
//!
//! - `instances` -- position, size, colour: exactly [`ParticleInstance`], so
//!   the ordinary particle pipeline draws it as a vertex buffer, unchanged.
//! - `motion` -- velocity and seconds of life left. Zero-initialised, which
//!   is what makes a fresh buffer all dead particles rather than all live ones
//!   at the origin.
//!
//! A step emits `spawn` particles into the slots after the last step's, and
//! advances every live one. The ring is what replaces a free list: every
//! particle of one emitter lives equally long, so the slot about to be reused
//! always holds the oldest -- full means the oldest are replaced, which is
//! what Godot does at its `amount` and the VFX Graph at its capacity.
//!
//! A dead slot is drawn as a zero-size quad. Drawing all `capacity` instances
//! costs vertex work on dead ones; it is what lets the draw need no count read
//! back from the GPU.

use std::collections::HashMap;

use crate::particles::ParticleInstance;

/// One GPU emitter's next step and the parameters that shape it, as the
/// render system gathers them each frame.
#[derive(Debug, Clone, Copy)]
pub struct GpuEmitterFrame {
    /// `GpuParticleStep::id`: which emitter's buffers this is.
    pub id: u64,
    /// `GpuParticleStep::tick`: a new value means a new step to simulate.
    pub tick: u64,
    /// Particles to emit this step.
    pub spawn: u32,
    /// Where from, in world space.
    pub origin: glam::Vec3,
    /// Seconds to advance by.
    pub dt: f32,
    /// Seeds this step's random directions.
    pub seed: u32,
    /// The ring's size: `ParticleEmitter::max_particles`.
    pub capacity: u32,
    /// Seconds each particle lives.
    pub lifetime: f32,
    /// Speed at birth.
    pub speed: f32,
    /// Emission cone half-angle, in degrees.
    pub spread_degrees: f32,
    /// Downward acceleration.
    pub gravity: f32,
    /// Billboard half-size at birth.
    pub start_size: f32,
    /// Billboard half-size at death.
    pub end_size: f32,
    /// Colour at birth (alpha fades out over the life, as on the CPU path).
    pub start_color: [f32; 3],
    /// Colour at death.
    pub end_color: [f32; 3],
    /// The emitter's texture, or `None` for a flat quad.
    pub texture_id: Option<u64>,
}

/// The compute shader's per-step parameters. Matches `StepParams` in
/// [`GPU_PARTICLES_WGSL`].
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct StepParams {
    origin: [f32; 3],
    dt: f32,
    start_color: [f32; 3],
    lifetime: f32,
    end_color: [f32; 3],
    speed: f32,
    gravity: f32,
    cos_spread: f32,
    start_size: f32,
    end_size: f32,
    spawn_start: u32,
    spawn_count: u32,
    capacity: u32,
    seed: u32,
}

/// One emitter's GPU state.
struct EmitterState {
    capacity: u32,
    instances: wgpu::Buffer,
    motion: wgpu::Buffer,
    params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    /// The slot the next step's first new particle goes in.
    cursor: u32,
    /// The last `tick` simulated, so a frame drawn twice steps once.
    last_tick: u64,
    texture_id: Option<u64>,
}

/// Every GPU emitter's buffers and the compute pipeline that steps them.
pub struct GpuParticles {
    pipeline: wgpu::ComputePipeline,
    layout: wgpu::BindGroupLayout,
    emitters: HashMap<u64, EmitterState>,
}

/// Threads per workgroup; must match `@workgroup_size` in the shader.
const WORKGROUP: u32 = 64;

impl GpuParticles {
    /// Builds the compute pipeline. Emitters' buffers are made on first use.
    pub fn new(device: &wgpu::Device) -> Self {
        let entry = |binding: u32, ty: wgpu::BufferBindingType| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu particles bgl"),
            entries: &[
                entry(0, wgpu::BufferBindingType::Uniform),
                entry(1, wgpu::BufferBindingType::Storage { read_only: false }),
                entry(2, wgpu::BufferBindingType::Storage { read_only: false }),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu particles shader"),
            source: wgpu::ShaderSource::Wgsl(GPU_PARTICLES_WGSL.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu particles pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("gpu particles pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: "cs_step",
            compilation_options: Default::default(),
            cache: None,
        });
        Self {
            pipeline,
            layout,
            emitters: HashMap::new(),
        }
    }

    /// Steps every emitter in `frames` whose tick is new, makes the buffers of
    /// any seen for the first time (or whose capacity changed), and drops the
    /// buffers of any not in `frames` -- an emitter that was despawned, or
    /// switched back to the CPU. One submission for all of them, ahead of the
    /// frame that draws them.
    pub fn step(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, frames: &[GpuEmitterFrame]) {
        self.emitters
            .retain(|id, _| frames.iter().any(|f| f.id == *id));
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu particles step"),
        });
        let mut stepped = false;
        for frame in frames {
            let capacity = frame.capacity.max(1);
            if self
                .emitters
                .get(&frame.id)
                .is_none_or(|state| state.capacity != capacity)
            {
                self.emitters
                    .insert(frame.id, self.make_state(device, capacity));
            }
            let state = self.emitters.get_mut(&frame.id).expect("made above");
            state.texture_id = frame.texture_id;
            if state.last_tick == frame.tick {
                continue;
            }
            state.last_tick = frame.tick;
            let spawn = frame.spawn.min(capacity);
            let params = StepParams {
                origin: frame.origin.to_array(),
                dt: frame.dt,
                start_color: frame.start_color,
                lifetime: frame.lifetime.max(1e-6),
                end_color: frame.end_color,
                speed: frame.speed,
                gravity: frame.gravity,
                cos_spread: frame
                    .spread_degrees
                    .to_radians()
                    .clamp(0.0, std::f32::consts::PI)
                    .cos(),
                start_size: frame.start_size,
                end_size: frame.end_size,
                spawn_start: state.cursor,
                spawn_count: spawn,
                capacity,
                seed: frame.seed,
            };
            state.cursor = (state.cursor + spawn) % capacity;
            queue.write_buffer(&state.params, 0, bytemuck::bytes_of(&params));
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("gpu particles step"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &state.bind_group, &[]);
            pass.dispatch_workgroups(capacity.div_ceil(WORKGROUP), 1, 1);
            stepped = true;
        }
        if stepped {
            queue.submit(Some(encoder.finish()));
        }
    }

    fn make_state(&self, device: &wgpu::Device, capacity: u32) -> EmitterState {
        // Zero-initialised by wgpu: every slot starts dead (no life left).
        let instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu particle instances"),
            size: u64::from(capacity) * std::mem::size_of::<ParticleInstance>() as u64,
            usage: wgpu::BufferUsages::VERTEX
                | wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let motion = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu particle motion"),
            size: u64::from(capacity) * 16,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu particle params"),
            size: std::mem::size_of::<StepParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu particles bind group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: instances.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: motion.as_entire_binding(),
                },
            ],
        });
        EmitterState {
            capacity,
            instances,
            motion,
            params,
            bind_group,
            cursor: 0,
            last_tick: 0,
            texture_id: None,
        }
    }

    /// What to draw: each emitter's texture, instance buffer and capacity.
    pub fn draws(&self) -> Vec<(Option<u64>, &wgpu::Buffer, u32)> {
        let mut out: Vec<(&u64, &EmitterState)> = self.emitters.iter().collect();
        // A stable order, so a frame does not reshuffle its draw calls.
        out.sort_by_key(|(id, _)| **id);
        out.into_iter()
            .map(|(_, s)| (s.texture_id, &s.instances, s.capacity))
            .collect()
    }

    /// Reads one emitter's particles back: each slot's instance (size 0 when
    /// dead) and its velocity. Blocks on the GPU; for tests and tools, never
    /// for a frame. `None` for an emitter with no buffers.
    pub fn read_back(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        id: u64,
    ) -> Option<Vec<(ParticleInstance, glam::Vec3)>> {
        let state = self.emitters.get(&id)?;
        let read = |buffer: &wgpu::Buffer| -> Vec<u8> {
            let staging = device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("gpu particles read back"),
                size: buffer.size(),
                usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            let mut encoder = device.create_command_encoder(&Default::default());
            encoder.copy_buffer_to_buffer(buffer, 0, &staging, 0, buffer.size());
            queue.submit(Some(encoder.finish()));
            let slice = staging.slice(..);
            slice.map_async(wgpu::MapMode::Read, |_| {});
            device.poll(wgpu::Maintain::Wait);
            let bytes = slice.get_mapped_range().to_vec();
            staging.unmap();
            bytes
        };
        let instances: Vec<ParticleInstance> =
            bytemuck::cast_slice(&read(&state.instances)).to_vec();
        let motion: Vec<[f32; 4]> = bytemuck::cast_slice(&read(&state.motion)).to_vec();
        Some(
            instances
                .into_iter()
                .zip(motion)
                .map(|(i, m)| (i, glam::Vec3::new(m[0], m[1], m[2])))
                .collect(),
        )
    }
}

/// The step: emit into this step's slots, then advance every live particle
/// and write what it looks like. One thread per slot.
pub const GPU_PARTICLES_WGSL: &str = r#"
struct StepParams {
    origin: vec3<f32>,
    dt: f32,
    start_color: vec3<f32>,
    lifetime: f32,
    end_color: vec3<f32>,
    speed: f32,
    gravity: f32,
    cos_spread: f32,
    start_size: f32,
    end_size: f32,
    spawn_start: u32,
    spawn_count: u32,
    capacity: u32,
    seed: u32,
};
// Laid out as `ParticleInstance`: position, size, colour.
struct Instance {
    position: vec3<f32>,
    size: f32,
    color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> params: StepParams;
@group(0) @binding(1) var<storage, read_write> instances: array<Instance>;
// xyz velocity, w seconds of life left (0 = dead).
@group(0) @binding(2) var<storage, read_write> motion: array<vec4<f32>>;

// PCG hash: a well-mixed u32 from a u32, so neighbouring slots and
// neighbouring seeds give unrelated directions.
fn pcg(v: u32) -> u32 {
    let state = v * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}
fn unit(v: u32) -> f32 {
    return f32(pcg(v) >> 8u) / 16777216.0;
}

@compute @workgroup_size(64)
fn cs_step(@builtin(global_invocation_id) gid: vec3<u32>) {
    let i = gid.x;
    if (i >= params.capacity) {
        return;
    }
    var m = motion[i];
    var position = instances[i].position;

    // Is this slot one of this step's new particles? The ring position
    // relative to where this step's emission starts.
    let offset = (i + params.capacity - params.spawn_start) % params.capacity;
    if (offset < params.spawn_count) {
        // Uniform over the cone's spherical cap around +Y, as the CPU path
        // draws it: cos(theta) uniform in [cos_spread, 1], phi uniform.
        let a = unit(params.seed ^ ((i * 2u + 1u) * 0x9E3779B9u));
        let b = unit(pcg(params.seed + i) ^ 0x85EBCA6Bu);
        let cos_theta = 1.0 - a * (1.0 - params.cos_spread);
        let sin_theta = sqrt(max(0.0, 1.0 - cos_theta * cos_theta));
        let phi = b * 6.28318530718;
        let dir = vec3<f32>(sin_theta * cos(phi), cos_theta, sin_theta * sin(phi));
        m = vec4<f32>(dir * params.speed, params.lifetime);
        position = params.origin;
    }

    // Advance, as the CPU path does after emitting: gravity, move, age.
    if (m.w > 0.0) {
        m.y = m.y - params.gravity * params.dt;
        position = position + m.xyz * params.dt;
        m.w = m.w - params.dt;
    }
    motion[i] = m;

    if (m.w > 0.0) {
        let t = clamp(1.0 - m.w / params.lifetime, 0.0, 1.0);
        let colour = mix(params.start_color, params.end_color, t);
        instances[i] = Instance(
            position,
            mix(params.start_size, params.end_size, t),
            vec4<f32>(colour, 1.0 - t),
        );
    } else {
        // Dead: a zero-size quad, which rasterises nothing.
        instances[i] = Instance(position, 0.0, vec4<f32>(0.0));
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    fn device() -> (std::sync::Arc<wgpu::Device>, std::sync::Arc<wgpu::Queue>) {
        pollster::block_on(crate::surface::WgpuSurface::headless_device_for_testing())
    }

    /// Straight up at 2 units/s, no gravity, long-lived: the simplest
    /// emitter whose every particle is predictable.
    fn frame(tick: u64, spawn: u32) -> GpuEmitterFrame {
        GpuEmitterFrame {
            id: 7,
            tick,
            spawn,
            origin: Vec3::new(1.0, 2.0, 3.0),
            dt: 0.5,
            seed: 12345,
            capacity: 64,
            lifetime: 10.0,
            speed: 2.0,
            spread_degrees: 0.0,
            gravity: 0.0,
            start_size: 0.4,
            end_size: 0.0,
            start_color: [1.0, 0.0, 0.0],
            end_color: [0.0, 0.0, 1.0],
            texture_id: None,
        }
    }

    fn alive(p: &[(ParticleInstance, Vec3)]) -> usize {
        p.iter().filter(|(i, _)| i.size > 0.0).count()
    }

    fn step_and_read(
        gpu: &mut GpuParticles,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frames: &[GpuEmitterFrame],
    ) -> Vec<(ParticleInstance, Vec3)> {
        gpu.step(device, queue, frames);
        gpu.read_back(device, queue, frames[0].id)
            .expect("the emitter has buffers")
    }

    /// The pipeline builds: the step shader is valid WGSL for this device.
    #[test]
    fn the_step_pipeline_builds() {
        let (device, _) = device();
        let _ = GpuParticles::new(&device);
    }

    /// One step emits exactly `spawn` particles into the first slots, from
    /// the origin, moving them one step along their velocity -- the CPU
    /// path's emit-then-advance -- and leaves every other slot dead.
    #[test]
    fn a_step_emits_its_particles_and_moves_them() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        let p = step_and_read(&mut gpu, &device, &queue, &[frame(1, 10)]);
        assert_eq!(p.len(), 64, "the ring is the capacity");
        assert_eq!(alive(&p), 10, "exactly the spawned count lives");
        for (instance, velocity) in &p[..10] {
            assert_eq!(
                *velocity,
                Vec3::new(0.0, 2.0, 0.0),
                "zero spread: straight up"
            );
            let at = Vec3::from_array(instance.position);
            assert!(
                (at - Vec3::new(1.0, 3.0, 3.0)).length() < 1e-4,
                "origin plus one step of 2 units/s over 0.5 s: {at}"
            );
        }
    }

    /// Gravity pulls velocity down every step, and the position follows.
    #[test]
    fn gravity_bends_the_path() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        let mut f = frame(1, 1);
        f.gravity = 4.0;
        step_and_read(&mut gpu, &device, &queue, &[f]);
        f.tick = 2;
        f.spawn = 0;
        let p = step_and_read(&mut gpu, &device, &queue, &[f]);
        // v: 2 -> 0 -> -2; y: 2 -> 2 (0 * 0.5) -> 1 (-2 * 0.5).
        assert!((p[0].1.y - -2.0).abs() < 1e-4, "velocity {}", p[0].1);
        assert!(
            (p[0].0.position[1] - 1.0).abs() < 1e-4,
            "height {:?}",
            p[0].0.position
        );
    }

    /// A particle dies once its lifetime has run out, and is drawn as
    /// nothing; until then its size and colour move from start to end.
    #[test]
    fn particles_age_fade_and_die() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        let mut f = frame(1, 1);
        f.lifetime = 1.0;
        let p = step_and_read(&mut gpu, &device, &queue, &[f]);
        let (half, _) = p[0];
        assert!(
            (half.size - 0.2).abs() < 1e-4,
            "half way, half the size: {}",
            half.size
        );
        assert!(
            (half.color[3] - 0.5).abs() < 1e-4,
            "and half the alpha: {:?}",
            half.color
        );
        assert!(
            (half.color[0] - 0.5).abs() < 1e-4 && (half.color[2] - 0.5).abs() < 1e-4,
            "and half way from red to blue: {:?}",
            half.color
        );
        f.tick = 2;
        f.spawn = 0;
        let p = step_and_read(&mut gpu, &device, &queue, &[f]);
        assert_eq!(alive(&p), 0, "a second half second ends its life");
    }

    /// A full ring replaces its oldest particles: two steps of 5 into 8 slots
    /// leave 8 alive, not 10, and the first two slots hold the newest.
    #[test]
    fn a_full_ring_replaces_the_oldest() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        let mut f = frame(1, 5);
        f.capacity = 8;
        step_and_read(&mut gpu, &device, &queue, &[f]);
        f.tick = 2;
        let p = step_and_read(&mut gpu, &device, &queue, &[f]);
        assert_eq!(alive(&p), 8, "capacity, not the 10 emitted");
        let height = |slot: usize| p[slot].0.position[1];
        assert!(
            height(0) < height(2),
            "slot 0 was reused for a new particle (one step of travel), slot 2 \
             holds an old one (two steps): {} vs {}",
            height(0),
            height(2)
        );
    }

    /// A frame drawn twice without a tick between them steps once.
    #[test]
    fn the_same_tick_is_stepped_once() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        let once = step_and_read(&mut gpu, &device, &queue, &[frame(1, 3)]);
        let again = step_and_read(&mut gpu, &device, &queue, &[frame(1, 3)]);
        assert_eq!(alive(&once), 3);
        assert_eq!(alive(&again), 3, "no second emission");
        assert_eq!(
            once[0].0.position, again[0].0.position,
            "and no second advance"
        );
    }

    /// The spread is honoured: a full sphere sends particles every way,
    /// different slots get different directions, all at the emitter's speed.
    #[test]
    fn the_spread_shapes_the_directions() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        let mut f = frame(1, 64);
        f.spread_degrees = 180.0;
        let p = step_and_read(&mut gpu, &device, &queue, &[f]);
        let downward = p.iter().filter(|(_, v)| v.y < 0.0).count();
        let sideways: Vec<f32> = p.iter().map(|(_, v)| v.x).collect();
        assert!(
            downward > 10,
            "a full sphere sends some down: {downward} of 64"
        );
        assert!(
            sideways.iter().any(|x| (x - sideways[0]).abs() > 0.1),
            "and not all one way"
        );
        for (_, v) in &p {
            assert!(
                (v.length() - 2.0).abs() < 1e-3,
                "at the emitter's speed: {v}"
            );
        }
    }

    /// An emitter no frame names any more -- despawned, or back on the CPU --
    /// loses its buffers.
    #[test]
    fn an_emitter_not_named_is_dropped() {
        let (device, queue) = device();
        let mut gpu = GpuParticles::new(&device);
        step_and_read(&mut gpu, &device, &queue, &[frame(1, 3)]);
        gpu.step(&device, &queue, &[]);
        assert!(gpu.read_back(&device, &queue, 7).is_none());
        assert!(gpu.draws().is_empty(), "and nothing is drawn for it");
    }

    #[test]
    fn step_params_match_the_shader_layout() {
        // Three 16-byte rows of vec3 + f32, then four f32 and four u32: 80
        // bytes, as WGSL lays the uniform out. A mismatch would shift every
        // field after it.
        assert_eq!(std::mem::size_of::<StepParams>(), 80);
    }
}
