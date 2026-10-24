//! `pipeline_cache`: renderers on the tests' shared device share their
//! pipelines, and only what is the same is shared.
//!
//! The statistics are the device's, and every test in a binary shares the
//! device, so the tests here take turns (`SERIAL`) -- otherwise one test's
//! renderer would move another's counts.

mod common;

use bsengine_rhi_wgpu::pipeline_cache::{self, CachedCreate};
use common::{Draw, Harness, Light, Scene};
use glam::Vec3;
use std::sync::{Arc, Mutex};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> std::sync::MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|e| e.into_inner())
}

fn lit_cube(cube: u64) -> Scene {
    Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        light: Light {
            direction: Vec3::new(-0.4, -1.0, -0.3),
            ..Light::default()
        },
        camera_pos: Vec3::new(2.0, 2.0, 3.0),
        ..Scene::default()
    }
}

/// The point of the cache: a second renderer on the device makes no
/// pipeline, module or layout of its own -- every one is a hit -- and draws
/// the same frame as the first, so what it was handed is what it would have
/// built.
#[test]
fn a_second_renderer_compiles_nothing_and_draws_the_same_frame() {
    let _turn = serial();
    let mut first = Harness::new();
    let cube = first.cube();
    let a = first.render(&lit_cube(cube));
    let device = first.device();
    let before =
        pipeline_cache::stats(&device).expect("premise: the shared device shares its pipelines");

    let mut second = Harness::new();
    let cube = second.cube();
    let b = second.render(&lit_cube(cube));
    let after = pipeline_cache::stats(&device).expect("still registered");

    assert_eq!(
        after.misses, before.misses,
        "the second renderer made objects of its own: {before:?} -> {after:?}"
    );
    assert!(
        after.hits - before.hits >= 60,
        "a renderer asks for about seventy pipelines, modules and layouts; the second \
         was handed only {} of them",
        after.hits - before.hits
    );
    let mut colours: Vec<[u8; 4]> = a
        .data
        .chunks_exact(4)
        .map(|p| [p[0], p[1], p[2], p[3]])
        .collect();
    colours.sort_unstable();
    colours.dedup();
    assert!(
        colours.len() > 3,
        "premise: the frame has a lit cube in it, not one colour: {} colours",
        colours.len()
    );
    assert_eq!(a.data, b.data, "the two renderers drew different frames");
}

const SHADER: &str = "
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    return vec4<f32>(f32(i), 0.0, 0.0, 1.0);
}
@fragment fn fs() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0);
}
";

fn module_desc(source: &str) -> wgpu::ShaderModuleDescriptor<'_> {
    wgpu::ShaderModuleDescriptor {
        label: Some("pipeline cache test"),
        source: wgpu::ShaderSource::Wgsl(source.into()),
    }
}

fn pipeline(
    device: &wgpu::Device,
    module: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    samples: u32,
    format: wgpu::TextureFormat,
) -> Arc<wgpu::RenderPipeline> {
    device.cached_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("pipeline cache test"),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module,
            entry_point: "vs",
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
        fragment: Some(wgpu::FragmentState {
            module,
            entry_point: "fs",
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        multiview: None,
        cache: None,
    })
}

/// Same request, same object; any field that differs, another one. The
/// sample count and the target format are the fields a renderer varies
/// between its own pipelines of one label (MSAA variants, HDR output), so a
/// key that left either out would hand an MSAA pass a single-sample pipeline.
#[test]
fn the_same_request_is_shared_and_a_different_one_is_not() {
    let _turn = serial();
    let device = Harness::new().device();
    let module = device.cached_shader_module(module_desc(SHADER));
    assert!(
        Arc::ptr_eq(&module, &device.cached_shader_module(module_desc(SHADER))),
        "one source, one module"
    );
    let other_source = format!("{SHADER}// another\n");
    assert!(
        !Arc::ptr_eq(
            &module,
            &device.cached_shader_module(module_desc(&other_source))
        ),
        "another source, another module"
    );
    let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline cache test"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });

    let rgba = wgpu::TextureFormat::Rgba8Unorm;
    let one = pipeline(&device, &module, &layout, 1, rgba);
    assert!(
        Arc::ptr_eq(&one, &pipeline(&device, &module, &layout, 1, rgba)),
        "the same descriptor, the same pipeline"
    );
    assert!(
        !Arc::ptr_eq(&one, &pipeline(&device, &module, &layout, 4, rgba)),
        "four samples is another pipeline"
    );
    assert!(
        !Arc::ptr_eq(
            &one,
            &pipeline(
                &device,
                &module,
                &layout,
                1,
                wgpu::TextureFormat::Bgra8Unorm
            )
        ),
        "another target format is another pipeline"
    );
}

/// A descriptor naming a module the cache did not make is created every
/// time, never cached: the cache tells modules apart by address, and an
/// address this cache does not keep alive can be freed and reused by another
/// module -- which would then be handed this pipeline.
#[test]
fn a_pipeline_over_a_module_the_cache_did_not_make_is_never_cached() {
    let _turn = serial();
    let device = Harness::new().device();
    let layout = device.cached_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("pipeline cache test"),
        bind_group_layouts: &[],
        push_constant_ranges: &[],
    });
    let module = device.create_shader_module(module_desc(SHADER));
    let rgba = wgpu::TextureFormat::Rgba8Unorm;
    let before = pipeline_cache::stats(&device).expect("premise: registered");
    let a = pipeline(&device, &module, &layout, 1, rgba);
    let b = pipeline(&device, &module, &layout, 1, rgba);
    let after = pipeline_cache::stats(&device).expect("registered");
    assert!(!Arc::ptr_eq(&a, &b), "made twice, not shared");
    assert_eq!(
        after.uncached - before.uncached,
        2,
        "{before:?} -> {after:?}"
    );
    assert_eq!(after.misses, before.misses, "and nothing was kept");
}

/// A device that is not registered keeps nothing: a game's renderer, or a
/// runtime test app's, which makes a device of its own and drops it -- a
/// cache there would keep every dropped device's pipelines alive.
#[test]
fn a_renderer_on_its_own_device_shares_nothing() {
    let _turn = serial();
    let own = pollster::block_on(bsengine_rhi_wgpu::surface::WgpuSurface::new_offscreen(
        16, 16, false,
    ))
    .expect("a renderer on a device of its own");
    assert_eq!(pipeline_cache::stats(&own.device_arc()), None);
}
