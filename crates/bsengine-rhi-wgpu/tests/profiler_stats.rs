//! Draw-call/triangle counting and texture-memory tracking, verified against
//! the real render_frame path via the same Harness the pixel tests use.
mod common;

use common::{Draw, Harness, Scene};
use glam::Vec3;

#[test]
fn rendering_one_opaque_cube_reports_at_least_one_draw_call_and_some_triangles() {
    let mut h = Harness::new();
    let cube = h.cube();
    h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        ..Scene::default()
    });

    let stats = h.frame_stats();
    assert!(
        stats.draw_calls >= 1,
        "expected at least the main pass's draw call, got {}",
        stats.draw_calls
    );
    assert!(stats.triangles > 0, "a cube has triangles; got 0");
}

#[test]
fn rendering_an_empty_scene_reports_fewer_draw_calls_than_a_populated_one() {
    let mut h = Harness::new();
    h.render(&Scene::default());
    let empty_calls = h.frame_stats().draw_calls;

    let cube = h.cube();
    h.render(&Scene {
        draws: vec![Draw::new(cube, Vec3::ZERO)],
        ..Scene::default()
    });
    let populated_calls = h.frame_stats().draw_calls;

    assert!(
        populated_calls > empty_calls,
        "a scene with a cube should draw more than an empty one: {populated_calls} vs {empty_calls}"
    );
}

#[test]
fn rendering_populates_cpu_timing_and_nonzero_texture_stats() {
    let mut h = Harness::new();
    h.render(&Scene::default());

    let stats = h.frame_stats();
    assert!(stats.cpu_frame_time_ms > 0.0);
    assert!(stats.texture_count > 0); // depth/white/shadow textures alone guarantee this
    assert!(stats.texture_memory_bytes > 0);
}

#[test]
fn gpu_pass_times_are_consistent_with_gpu_timestamps_supported() {
    let mut h = Harness::new();
    h.render(&Scene::default());
    let stats = h.frame_stats();

    if stats.gpu_timestamps_supported {
        assert!(
            !stats.gpu_pass_times_ms.is_empty(),
            "adapter reports timestamp support but no pass times were recorded"
        );
    } else {
        assert!(
            stats.gpu_pass_times_ms.is_empty(),
            "adapter has no timestamp support but pass times were recorded anyway"
        );
    }
}

#[test]
fn frame_stats_history_caps_at_its_capacity() {
    let mut h = Harness::new();
    for _ in 0..(bsengine_rhi_wgpu::profiler::FRAME_STATS_HISTORY_CAPACITY + 10) {
        h.render(&Scene::default());
    }
    assert_eq!(
        h.frame_stats_history().lock().unwrap().len(),
        bsengine_rhi_wgpu::profiler::FRAME_STATS_HISTORY_CAPACITY
    );
}
