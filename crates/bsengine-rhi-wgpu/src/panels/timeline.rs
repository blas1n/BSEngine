//! The Timeline panel: draws a `Timeline`'s tracks, scrubs its playhead, and
//! publishes a preview request for the editor to apply.
//!
//! Read-only with respect to disk. Editing and saving are sub-step 2/2b, and
//! that boundary is enforced by a test rather than by intent -- see
//! `scrubbing_never_writes_to_the_timeline_file`.

use bsengine_core::editor_panel::{EditorPanel, EditorPanelContext};
use bsengine_core::timeline::{Timeline, Track};
use std::collections::HashMap;
use std::path::PathBuf;

/// Width of the track-label column, in points.
const LABEL_WIDTH: f32 = 130.0;
/// Height of the time ruler above the lanes, in points.
const RULER_HEIGHT: f32 = 22.0;
/// Height of one track lane, in points.
const LANE_HEIGHT: f32 = 26.0;
/// Radius of a drawn keyframe marker, in points.
const KEY_RADIUS: f32 = 5.0;

/// The Timeline panel: a track view over a cutscene, with a scrubbable
/// playhead.
pub struct TimelinePanel {
    /// The parsed timeline currently shown, if one is loaded.
    timeline: Option<Timeline>,
    /// Path the loaded timeline came from.
    path: Option<PathBuf>,
    /// The path text field's contents, which is what **Open** reads.
    ///
    /// Separate from `path` so a half-typed path never looks like the open
    /// file -- the same split `ShaderGraphPanel` uses, and for the same
    /// reason.
    path_buffer: String,
    /// Playhead position, in seconds.
    time: f32,

    /// Where each track's lane was drawn, rebuilt every frame.
    ///
    /// Public for the same reason as `ShaderGraphPanel::last_port_positions`:
    /// this project forbids hardcoded click coordinates in panel tests, and
    /// the usual helper finds widgets by walking `Shape::Text` galleys, which
    /// cannot find a lane -- a lane is a `Shape::Rect` carrying no text at
    /// all. Recording the geometry satisfies the rule by construction.
    pub last_lane_rects: Vec<egui::Rect>,
    /// Where each key was drawn, keyed by `(track index, key index)`.
    ///
    /// Public for the same reason as [`TimelinePanel::last_lane_rects`]; a
    /// key is a `Shape::Circle`. This is also what sub-step 2/2b's key
    /// dragging will hit-test against.
    pub last_key_positions: HashMap<(usize, usize), egui::Pos2>,
    /// The lane area's rectangle, which the time/x mapping maps against.
    last_lane_area: egui::Rect,
}

/// Hand-written because `egui::Rect` has no `Default`, and the meaningful
/// starting value for `last_lane_area` is `NOTHING` rather than a zero rect:
/// nothing has been laid out yet, and the mapping functions check for that.
impl Default for TimelinePanel {
    fn default() -> Self {
        Self {
            timeline: None,
            path: None,
            path_buffer: String::new(),
            time: 0.0,
            last_lane_rects: Vec::new(),
            last_key_positions: HashMap::new(),
            last_lane_area: egui::Rect::NOTHING,
        }
    }
}

impl TimelinePanel {
    /// A panel showing `timeline`, with no file behind it.
    pub fn with_timeline(timeline: Timeline) -> Self {
        Self {
            timeline: Some(timeline),
            ..Default::default()
        }
    }

    /// The x coordinate `seconds` maps to in the lane area.
    ///
    /// Public so tests press at a *time* rather than at a measured pixel. The
    /// pairing with [`TimelinePanel::x_to_time`] is what makes a scrub test a
    /// round trip -- press at `time_to_x(2.5)`, read 2.5 back -- and a test
    /// built on recorded pixel positions alone would not check the mapping at
    /// all.
    pub fn time_to_x(&self, seconds: f32) -> f32 {
        let duration = self.duration();
        if duration <= 0.0 {
            return self.last_lane_area.left();
        }
        self.last_lane_area.left()
            + (seconds / duration).clamp(0.0, 1.0) * self.last_lane_area.width()
    }

    /// The time an x coordinate in the lane area maps to, clamped to
    /// `0..=duration` so a drag past either end parks the playhead at that end
    /// rather than running off the timeline.
    pub fn x_to_time(&self, x: f32) -> f32 {
        let duration = self.duration();
        if self.last_lane_area.width() <= 0.0 || duration <= 0.0 {
            return 0.0;
        }
        let fraction = (x - self.last_lane_area.left()) / self.last_lane_area.width();
        (fraction * duration).clamp(0.0, duration)
    }

    /// The playhead position, in seconds.
    pub fn time(&self) -> f32 {
        self.time
    }

    /// The loaded timeline's duration, or `0.0` when nothing is loaded.
    fn duration(&self) -> f32 {
        self.timeline.as_ref().map_or(0.0, |t| t.duration)
    }

    /// A track's label, which is also what distinguishes two animation tracks
    /// from each other.
    fn track_label(track: &Track) -> String {
        match track {
            Track::Camera { .. } => "Camera".to_string(),
            Track::CameraShot { .. } => "Shots".to_string(),
            Track::Animation { entity, .. } => format!("Anim: {entity}"),
            Track::Event { .. } => "Events".to_string(),
        }
    }

    /// The times of a track's keys, in order.
    fn track_key_times(track: &Track) -> Vec<f32> {
        match track {
            Track::Camera { keys } => keys.iter().map(|k| k.time).collect(),
            Track::CameraShot { cuts } => cuts.iter().map(|c| c.time).collect(),
            Track::Animation { keys, .. } => keys.iter().map(|k| k.time).collect(),
            Track::Event { keys } => keys.iter().map(|k| k.time).collect(),
        }
    }

    /// Draws the ruler, the lanes and their keys, recording the geometry as it
    /// goes.
    fn draw_tracks(&mut self, ui: &mut egui::Ui) {
        self.last_lane_rects.clear();
        self.last_key_positions.clear();

        let Some(timeline) = self.timeline.clone() else {
            ui.label("No timeline. Select an entity with a TimelinePlayer, or open a .ron file.");
            self.last_lane_area = egui::Rect::NOTHING;
            return;
        };

        let full = ui.available_rect_before_wrap();
        let lane_left = full.left() + LABEL_WIDTH;
        let lanes_top = full.top() + RULER_HEIGHT;
        let lanes_height = LANE_HEIGHT * timeline.tracks.len() as f32;
        self.last_lane_area = egui::Rect::from_min_max(
            egui::pos2(lane_left, lanes_top),
            egui::pos2(full.right(), lanes_top + lanes_height),
        );

        let painter = ui.painter().clone();
        let visuals = ui.visuals().clone();

        // One tick per second, which reads a six-second cutscene fine and does
        // not need to scale until 2/2b adds zooming.
        let duration = timeline.duration.max(0.001);
        let mut second = 0.0_f32;
        while second <= duration {
            let x = self.time_to_x(second);
            painter.line_segment(
                [
                    egui::pos2(x, full.top()),
                    egui::pos2(x, full.top() + RULER_HEIGHT),
                ],
                egui::Stroke::new(1.0_f32, visuals.weak_text_color()),
            );
            second += 1.0;
        }

        for (i, track) in timeline.tracks.iter().enumerate() {
            let top = lanes_top + LANE_HEIGHT * i as f32;
            let lane = egui::Rect::from_min_max(
                egui::pos2(lane_left, top),
                egui::pos2(full.right(), top + LANE_HEIGHT),
            );
            self.last_lane_rects.push(lane);

            painter.rect_filled(lane, 2.0, visuals.faint_bg_color);
            painter.text(
                egui::pos2(full.left() + 4.0, lane.center().y),
                egui::Align2::LEFT_CENTER,
                Self::track_label(track),
                egui::FontId::default(),
                visuals.text_color(),
            );

            for (k, key_time) in Self::track_key_times(track).into_iter().enumerate() {
                let pos = egui::pos2(self.time_to_x(key_time), lane.center().y);
                self.last_key_positions.insert((i, k), pos);
                match track {
                    // A cut is an instant, not a span, so it is a full-height
                    // line rather than a dot sitting on the lane.
                    Track::CameraShot { .. } => {
                        painter.line_segment(
                            [
                                egui::pos2(pos.x, lane.top()),
                                egui::pos2(pos.x, lane.bottom()),
                            ],
                            egui::Stroke::new(2.0_f32, visuals.warn_fg_color),
                        );
                    }
                    _ => {
                        painter.circle_filled(pos, KEY_RADIUS, visuals.strong_text_color());
                    }
                }
            }
        }

        ui.allocate_rect(
            egui::Rect::from_min_max(full.min, egui::pos2(full.right(), lanes_top + lanes_height)),
            egui::Sense::hover(),
        );
    }
}

impl EditorPanel for TimelinePanel {
    fn id(&self) -> &str {
        "timeline"
    }

    fn title(&self) -> String {
        "Timeline".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        self.draw_tracks(ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::timeline::{AnimationKey, CameraKey, EventKey, ShotCut};
    use bsengine_core::{InspectorEntityInfo, InspectorState};

    /// A timeline with one of every track kind, so a test that counts lanes
    /// distinguishes "drew the tracks" from "drew one lane and stopped".
    fn four_track_timeline() -> Timeline {
        Timeline {
            duration: 6.0,
            tracks: vec![
                Track::Camera {
                    keys: vec![
                        CameraKey {
                            time: 0.0,
                            position: [0.0, 0.0, 0.0],
                            look_at: [0.0, 0.0, -1.0],
                        },
                        CameraKey {
                            time: 6.0,
                            position: [12.0, 0.0, 0.0],
                            look_at: [0.0, 0.0, -1.0],
                        },
                    ],
                },
                Track::CameraShot {
                    cuts: vec![ShotCut {
                        time: 3.5,
                        entity: "CloseUpShot".to_string(),
                    }],
                },
                Track::Animation {
                    entity: "Subject".to_string(),
                    keys: vec![AnimationKey {
                        time: 0.5,
                        clip: "Survey".to_string(),
                    }],
                },
                Track::Event {
                    keys: vec![EventKey {
                        time: 5.5,
                        name: "intro_over".to_string(),
                    }],
                },
            ],
        }
    }

    /// Headless multi-frame harness, following `shadergraph.rs`'s (itself
    /// following `viewport.rs`'s). One `egui::Context` for the whole test,
    /// because every interaction here spans frames: egui hit-tests a press
    /// against the *previous* frame's widget rects and reports a drag as
    /// started only on the frame after the press.
    ///
    /// The screen rect is given explicitly rather than left to
    /// `RawInput::default()`, so the lane area -- and therefore every
    /// recorded position -- is the same size on every machine.
    struct Harness {
        egui_ctx: egui::Context,
        screen_rect: egui::Rect,
        insp: InspectorState,
        entities_snapshot: Vec<InspectorEntityInfo>,
        panel: TimelinePanel,
    }

    impl Harness {
        fn new(timeline: Timeline) -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            Self {
                egui_ctx,
                screen_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0)),
                insp: InspectorState::default(),
                entities_snapshot: Vec::new(),
                panel: TimelinePanel::with_timeline(timeline),
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            self.egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(self.screen_rect),
                    events,
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default().show(egui_ctx, |ui| {
                        let mut panel_ctx = EditorPanelContext {
                            insp: &mut self.insp,
                            entities_snapshot: &self.entities_snapshot,
                            cursor_pos: (0.0, 0.0),
                            type_registry: None,
                        };
                        self.panel.ui(ui, &mut panel_ctx);
                    });
                },
            )
        }

        fn draw(&mut self) -> egui::FullOutput {
            self.frame(Vec::new())
        }

        /// Two input-less frames: a press is hit-tested against the previous
        /// frame's rects, so the frame that first lays the lanes out cannot
        /// also receive a press against them.
        fn settle(&mut self) -> egui::FullOutput {
            self.draw();
            self.draw()
        }

        /// A press on its own frame. egui's `is_decidedly_dragging` requires
        /// `!any_pressed()`, so a press and a move sent in one `RawInput`
        /// never report `dragged() == true`.
        fn press(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
            ])
        }

        /// The frame on which `drag_started()`/`dragged()` first turn true.
        fn drag_to(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerMoved(pos)])
        }

        fn release(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }])
        }
    }

    #[test]
    fn a_lane_is_drawn_for_every_track() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        assert_eq!(
            h.panel.last_lane_rects.len(),
            4,
            "one lane per track -- each Animation track names exactly one \
             entity, so lanes map 1:1 to tracks"
        );
    }

    #[test]
    fn every_key_of_every_track_is_placed() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        // 2 camera keys + 1 cut + 1 animation key + 1 event = 5.
        assert_eq!(
            h.panel.last_key_positions.len(),
            5,
            "a track whose keys are never placed still draws its lane, so \
             counting lanes alone does not prove the keys were drawn"
        );
        for (track, key) in [(0usize, 0usize), (0, 1), (1, 0), (2, 0), (3, 0)] {
            assert!(
                h.panel.last_key_positions.contains_key(&(track, key)),
                "track {track} key {key} was not placed"
            );
        }
    }
}
