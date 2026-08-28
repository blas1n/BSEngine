//! Profiler panel: shows recent frame timing, per-pass GPU cost, and
//! draw-call/triangle/texture-memory stats. See
//! `docs/superpowers/specs/2026-08-27-frame-profiler-gpu-debugger-design.md`.

use bsengine_core::{EditorPanel, EditorPanelContext};
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use crate::profiler::FrameStats;

/// Displays the rolling frame-stats history collected by
/// `WgpuSurface::render_frame`: a scrolling CPU frame-time bar graph, the
/// latest frame's draw-call/triangle/texture-memory counts, and (when the
/// adapter supports `wgpu::Features::TIMESTAMP_QUERY`) per-pass GPU timings.
pub struct ProfilerPanel {
    history: Arc<Mutex<VecDeque<FrameStats>>>,
}

impl ProfilerPanel {
    /// Wraps the shared frame-stats history handle exposed by
    /// `WgpuSurface::frame_stats_history`.
    pub fn new(history: Arc<Mutex<VecDeque<FrameStats>>>) -> Self {
        Self { history }
    }
}

impl EditorPanel for ProfilerPanel {
    fn id(&self) -> &str {
        "profiler"
    }

    fn title(&self) -> String {
        "Profiler".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        let snapshot: Vec<FrameStats> = self.history.lock().unwrap().iter().cloned().collect();
        let Some(latest) = snapshot.last() else {
            ui.label("No frames rendered yet.");
            return;
        };

        // Scrolling CPU frame-time bar graph, hand-drawn (no plotting crate in
        // this workspace -- egui/egui-wgpu/egui_dock/egui-phosphor only).
        let (rect, _response) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 80.0), egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 0.0, ui.visuals().extreme_bg_color);
        let bar_width = rect.width() / snapshot.len().max(1) as f32;
        let max_ms = snapshot
            .iter()
            .map(|s| s.cpu_frame_time_ms)
            .fold(16.6_f32, f32::max); // at least the 60fps line, so a fast frame doesn't look huge
        for (i, s) in snapshot.iter().enumerate() {
            let h = (s.cpu_frame_time_ms / max_ms).clamp(0.0, 1.0) * rect.height();
            let x = rect.left() + i as f32 * bar_width;
            let bar = egui::Rect::from_min_max(
                egui::pos2(x, rect.bottom() - h),
                egui::pos2(x + bar_width.max(1.0), rect.bottom()),
            );
            let color = if s.cpu_frame_time_ms <= 16.6 {
                egui::Color32::GREEN
            } else {
                egui::Color32::RED
            };
            painter.rect_filled(bar, 0.0, color);
        }

        ui.separator();
        ui.label(format!(
            "CPU frame time: {:.2} ms",
            latest.cpu_frame_time_ms
        ));
        ui.label(format!(
            "Draw calls: {}   Triangles: {}   Occluded: {}",
            latest.draw_calls, latest.triangles, latest.occluded_count
        ));
        ui.label(format!(
            "Texture memory: {:.1} MB ({} textures)",
            latest.texture_memory_bytes as f64 / (1024.0 * 1024.0),
            latest.texture_count
        ));

        ui.separator();
        if latest.gpu_timestamps_supported {
            egui::Grid::new("profiler_gpu_pass_times").show(ui, |ui| {
                for pass in &latest.gpu_pass_times_ms {
                    ui.label(&pass.name);
                    ui.label(format!("{:.2} ms", pass.duration_ms));
                    ui.end_row();
                }
            });
        } else {
            ui.label("GPU 타이밍 미지원 (adapter has no TIMESTAMP_QUERY support)");
        }
    }
}
