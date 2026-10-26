//! A GPU job longer than a minute, waited for by `surface::wait_for_gpu`.
//!
//! `Maintain::Wait` gives up after wgpu-core's 60 s fence wait and then,
//! in wgpu 22, frees the submission's allocators and staging buffers while the
//! GPU is still running it; with the D3D12 debug layer on, the process dies
//! with exit code 2173. Windows CI hit it when WARP's first dispatch of the
//! realtime GI update ran past a minute. This is the reproduction: one dispatch
//! spinning for over a minute on WARP, with a staging buffer in flight.
//!
//! Ignored because it takes over a minute by construction. Run it on Windows
//! with `cargo test -p bsengine-rhi-wgpu --test long_gpu_wait -- --ignored`.
//! It passes with `wait_for_gpu` as it is. With `wait_for_gpu` turned back
//! into `device.poll(wgpu::PollType::wait_indefinitely())`, the process exits with 2173
//! (shown as 125 in a POSIX shell) 60 s in. In CI the realtime GI tests stand
//! guard instead: on Windows CI their first dispatch is the minute-long job.

/// The spin's iteration count, in millions. 8000 is about 74 s on WARP on a
/// desktop CPU: past the 60 s limit with room to spare, short enough to run.
const SPIN_MILLIONS: u32 = 8000;

#[cfg(windows)]
#[test]
#[ignore]
fn a_gpu_job_past_a_minute_is_waited_for_not_freed() {
    use wgpu::util::DeviceExt;
    let instance = wgpu::Instance::new({
        let mut d = wgpu::InstanceDescriptor::new_without_display_handle();
        d.backends = wgpu::Backends::DX12;
        // The debug layer is what ends the process on the misuse; without
        // it the bug corrupts silently.
        d.flags = wgpu::InstanceFlags::debugging();
        d
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::None,
        compatible_surface: None,
        force_fallback_adapter: true,
        apply_limit_buckets: false,
    }))
    .expect("WARP is on every Windows; a skip would look like a pass");
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("spin"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
            @group(0) @binding(0) var<storage, read_write> out: array<u32>;
            @group(0) @binding(1) var<uniform> n: vec4<u32>;
            @compute @workgroup_size(1) fn main() {
                var x = n.y;
                for (var i = 0u; i < n.x; i++) {
                    for (var j = 0u; j < 1000000u; j++) { x = x * 1664525u + 1013904223u; }
                }
                out[0] = x;
            }"#
            .into(),
        ),
    });
    let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
        label: Some("spin"),
        layout: None,
        module: &module,
        entry_point: Some("main"),
        compilation_options: Default::default(),
        cache: None,
    });
    let out = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("spin out"),
        size: 256,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // The count through a uniform, so the compiler cannot fold the loop away.
    let n = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("spin count"),
        contents: bytemuck::cast_slice(&[SPIN_MILLIONS, 7u32, 0, 0]),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: out.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: n.as_entire_binding(),
            },
        ],
    });
    // A staging buffer in flight with the job, as a frame's uniform writes are:
    // what the timed-out wait frees under the GPU.
    queue.write_buffer(&out, 64, &[1u8; 64]);
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }
    queue.submit(Some(encoder.finish()));

    let started = std::time::Instant::now();
    bsengine_rhi_wgpu::surface::wait_for_gpu(&device, &queue);
    let waited = started.elapsed();
    // The premise: the job ran past the limit the bug lives behind. A faster
    // machine that finishes sooner proves nothing; raise SPIN_MILLIONS.
    assert!(
        waited.as_secs() > 60,
        "premise: the job must outlast wgpu's 60 s wait, took {waited:?}"
    );
    // And the device is still usable: the next frame's work goes through.
    queue.write_buffer(&out, 128, &[2u8; 64]);
    queue.submit(None);
    bsengine_rhi_wgpu::surface::wait_for_gpu(&device, &queue);
}
