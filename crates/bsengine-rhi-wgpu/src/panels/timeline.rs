//! The Timeline panel: draws a `Timeline`'s tracks, scrubs its playhead, and
//! publishes a preview request for the editor to apply.
//!
//! Edits it too: keys move, are added and deleted, tracks come and go, and
//! Save writes the file back.
//!
//! **Only Save writes.** Scrubbing, previewing and undo never touch the disk,
//! which is enforced by a test rather than by intent -- see
//! `only_saving_writes_to_the_timeline_file`.

use bsengine_core::editor_panel::{EditorPanel, EditorPanelContext};
use bsengine_core::timeline::{Timeline, Track};
use bsengine_core::{PreviewCamera, TimelinePreview};
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

/// What a press grabbed, decided once and held for the whole drag.
///
/// egui reports a drag as started on the frame *after* the press, by which
/// time the pointer may well have left the key -- so the decision cannot be
/// re-made per frame. `ShaderGraphPanel` holds node and wire drags the same
/// way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Grab {
    /// The playhead.
    Scrub,
    /// A key, by `(track index, key index)`. The index is updated whenever a
    /// move re-sorts the track, so it always names the key under the pointer.
    Key { track: usize, key: usize },
}

/// Moves one key to `new_time`, keeping the track ordered by time, and returns
/// the index it ended up at.
///
/// Remove-retime-reinsert rather than assign-then-sort: it keeps the sort
/// invariant true on every frame of a drag *and* hands back the moved key's
/// new index, which is what lets the drag keep hold of the right key when a
/// move crosses a neighbour. Sorting afterwards would leave the caller
/// guessing which entry is now theirs, and two keys sharing a time makes that
/// guess unresolvable.
fn move_key(track: &mut Track, index: usize, new_time: f32) -> usize {
    macro_rules! retime {
        ($keys:expr) => {{
            if index >= $keys.len() {
                return index;
            }
            let mut key = $keys.remove(index);
            key.time = new_time;
            let at = $keys.partition_point(|k| k.time <= new_time);
            $keys.insert(at, key);
            at
        }};
    }
    match track {
        Track::Camera { keys } => retime!(keys),
        Track::CameraShot { cuts } => retime!(cuts),
        Track::Animation { keys, .. } => retime!(keys),
        Track::Event { keys } => retime!(keys),
    }
}

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
    /// The comment block that preceded the data in the file as loaded,
    /// re-emitted verbatim on save.
    ///
    /// RON has no comments in its data model, so a parse-and-reserialise loses
    /// them. `games/cutscene-demo/assets/timelines/intro.ron` carries five
    /// lines explaining why the demo is shaped as it is, and keeping those is
    /// worth more than the inline ones this cannot keep.
    leading_comments: String,
    /// Playhead position, in seconds.
    time: f32,
    /// A path the user opened explicitly, which outranks the selection until
    /// a different timeline entity is selected.
    explicit_path: Option<PathBuf>,
    /// The selection this panel last saw, so it can tell a *change* of
    /// selection from the same entity staying selected.
    last_selected_id: Option<u64>,
    /// What the last open did, shown in the toolbar.
    status: Option<String>,
    /// The key the value strip edits and Delete removes, as
    /// `(track index, key index)`.
    selected_key: Option<(usize, usize)>,
    /// What the in-progress drag grabbed, if one is in progress.
    dragging: Option<Grab>,
    /// Whether the loaded timeline has unsaved edits.
    dirty: bool,
    /// Timelines as they were before each edit, most recent last.
    ///
    /// Whole clones rather than diffs: `Timeline` is a small value type, so
    /// there is no patch machinery here to get wrong. The editor's own undo
    /// stack holds `EditorSnapshot`s -- entity and scene state -- and a
    /// timeline is an asset that is not in them, so the two cannot merge.
    undo_stack: Vec<Timeline>,
    /// Timelines undone, for redo. Cleared by any fresh edit.
    redo_stack: Vec<Timeline>,
    /// Whether the value strip drew this frame.
    value_strip_shown: bool,
    /// Whether the panel is publishing a preview request.
    ///
    /// Off by default. Silently turning the viewport into a cutscene camera
    /// because a panel happens to be open would be surprising -- and an off
    /// switch is also a measurement instrument, since it is what lets a test
    /// assert that the orbit camera wins when preview is off.
    preview: bool,

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
    /// key is a `Shape::Circle`. It is also what key dragging hit-tests
    /// against, via `key_at`.
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
            leading_comments: String::new(),
            selected_key: None,
            dragging: None,
            dirty: false,
            undo_stack: Vec::new(),
            redo_stack: Vec::new(),
            value_strip_shown: false,
            time: 0.0,
            explicit_path: None,
            last_selected_id: None,
            status: None,
            preview: false,
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

    /// Loads `path`, replacing whatever was shown.
    ///
    /// Errors are values, not panics: the path box is user input, and a
    /// mistyped path must show a status line rather than take the editor
    /// down.
    fn load(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let text = std::fs::read_to_string(&path)
            .map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        let timeline: Timeline =
            ron::from_str(&text).map_err(|e| format!("cannot parse {}: {e}", path.display()))?;

        // The maximal prefix of blank and `//` lines. A rule about lines
        // rather than a RON parse, so it cannot fail on a file it does not
        // understand.
        let mut prefix = String::new();
        for line in text.lines() {
            let trimmed = line.trim_start();
            if trimmed.is_empty() || trimmed.starts_with("//") {
                prefix.push_str(line);
                prefix.push('\n');
            } else {
                break;
            }
        }
        while prefix.ends_with("\n\n") {
            prefix.pop();
        }

        self.time = self.time.clamp(0.0, timeline.duration);
        self.timeline = Some(timeline);
        self.path = Some(path);
        self.leading_comments = prefix;
        // A different file's history is not this file's, and a freshly loaded
        // file has nothing unsaved. This is also what makes `revert` complete:
        // it is just `load` again.
        self.undo_stack.clear();
        self.redo_stack.clear();
        self.selected_key = None;
        self.dirty = false;
        Ok(())
    }

    /// Opens `path` as the **Open** button would.
    ///
    /// Exists for the panel's own tests, which drive the precedence rule
    /// without having to type into an egui text field.
    #[cfg(test)]
    fn open_for_test(&mut self, path: &std::path::Path) {
        self.explicit_path = Some(path.to_path_buf());
        if let Err(e) = self.load(path) {
            self.status = Some(e);
        }
    }

    /// Resolves which timeline should be shown this frame.
    ///
    /// An explicitly opened path wins and keeps winning; selecting a
    /// *different* entity that has a `TimelinePlayer` clears it and resumes
    /// following the selection. Selecting an entity **without** one leaves
    /// the opened file alone rather than blanking the panel, because "I
    /// clicked a light" is not a request to close the cutscene I was
    /// looking at.
    fn resolve_source(&mut self, ctx: &EditorPanelContext) {
        let selected_path = ctx
            .insp
            .reflected_components
            .iter()
            .find(|(type_path, _)| type_path == "bsengine_core::timeline::TimelinePlayer")
            .and_then(|(_, value)| {
                value
                    .as_any()
                    .downcast_ref::<bsengine_core::timeline::TimelinePlayer>()
                    .map(|p| PathBuf::from(&p.timeline))
            });

        let selection_changed = ctx.insp.selected_id != self.last_selected_id;
        self.last_selected_id = ctx.insp.selected_id;
        if selection_changed && selected_path.is_some() {
            self.explicit_path = None;
        }

        let Some(wanted) = self.explicit_path.clone().or(selected_path) else {
            return;
        };
        if self.path.as_deref() == Some(wanted.as_path()) {
            return;
        }
        // Unsaved edits outrank the selection. Not auto-save, which would
        // change a committed asset behind the user's back; not silent discard,
        // which is the data loss this exists to prevent.
        if self.dirty {
            self.status = Some("unsaved changes -- Save, Revert or Discard first".to_string());
            return;
        }
        if let Err(e) = self.load(&wanted) {
            self.status = Some(e);
            self.timeline = None;
            self.path = None;
        }
    }

    /// Builds this frame's preview request from the playhead.
    ///
    /// A cut resolves against the entity snapshot rather than against the
    /// timeline, because a shot names an ordinary entity and its pose lives
    /// in the scene. The snapshot's rotation is euler degrees in `XYZ` order
    /// (`bsengine_editor`'s `plugin.rs:76`), so the inverse used here is the
    /// same pairing that file uses to write a rotation back.
    fn build_preview(&self, ctx: &EditorPanelContext) -> Option<TimelinePreview> {
        let timeline = self.timeline.as_ref()?;
        let at = bsengine_core::timeline::evaluate(timeline, self.time);

        let camera = if let Some(shot) = &at.shot {
            ctx.entities_snapshot
                .iter()
                .find(|e| e.name.as_deref() == Some(shot.as_str()))
                .map(|e| {
                    let rot = e.rotation.unwrap_or([0.0; 3]);
                    let quat = glam::Quat::from_euler(
                        glam::EulerRot::XYZ,
                        rot[0].to_radians(),
                        rot[1].to_radians(),
                        rot[2].to_radians(),
                    );
                    PreviewCamera {
                        position: e.position.unwrap_or([0.0; 3]),
                        rotation: quat.to_array(),
                        fov_y_degrees: e.camera_fov,
                    }
                })
        } else {
            at.camera.as_ref().map(|c| PreviewCamera {
                position: c.position,
                rotation: c.rotation().to_array(),
                fov_y_degrees: None,
            })
        };

        // The most recent animation key at or before the playhead, per track,
        // with the time reached *within* that clip. Scrubbing asks what this
        // instant looks like, so the clip time is how long the clip has been
        // running by now.
        let mut clips = Vec::new();
        for track in &timeline.tracks {
            if let Track::Animation { entity, keys } = track {
                if let Some(key) = keys
                    .iter()
                    .filter(|k| k.time <= self.time)
                    .max_by(|a, b| a.time.total_cmp(&b.time))
                {
                    clips.push((entity.clone(), key.clip.clone(), self.time - key.time));
                }
            }
        }

        Some(TimelinePreview { camera, clips })
    }

    /// The currently selected key.
    ///
    /// Public because the panel's tests assert on selection, and this project
    /// forbids reaching selection through hardcoded click coordinates.
    pub fn selected_key(&self) -> Option<(usize, usize)> {
        self.selected_key
    }

    /// Whether there are unsaved edits.
    ///
    /// Public so tests assert the dirty guard's precondition rather than
    /// inferring it from behaviour.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// How many undo steps are available.
    ///
    /// Public so a test can assert a drag produced *one* entry, which is the
    /// property that separates a correct push-once rule from a stack full of
    /// near-identical frames.
    pub fn undo_depth(&self) -> usize {
        self.undo_stack.len()
    }

    /// Whether the value strip drew for the current selection.
    ///
    /// Public because the strip is conditional and a test must be able to
    /// assert it stayed away.
    pub fn value_strip_shown(&self) -> bool {
        self.value_strip_shown
    }

    /// The key drawn nearest `pos`, if one is within grabbing distance.
    ///
    /// Uses the positions recorded last frame -- the same set the tests press
    /// at -- so a click that looks like it is on a key is one.
    fn key_at(&self, pos: egui::Pos2) -> Option<(usize, usize)> {
        let grab = KEY_RADIUS * 2.0_f32;
        self.last_key_positions
            .iter()
            .filter(|(_, p)| p.distance(pos) <= grab)
            .min_by(|a, b| a.1.distance(pos).total_cmp(&b.1.distance(pos)))
            .map(|(k, _)| *k)
    }

    /// Records the current timeline as an undo point.
    ///
    /// Call once per *interaction*, not per frame: a drag spans many frames,
    /// and pushing on each would bury one edit under dozens of identical
    /// states so that a single Undo appeared to do nothing.
    fn push_undo(&mut self) {
        if let Some(t) = self.timeline.clone() {
            self.undo_stack.push(t);
            self.redo_stack.clear();
        }
    }

    /// Steps back one edit.
    pub fn undo(&mut self) {
        if let (Some(previous), Some(current)) = (self.undo_stack.pop(), self.timeline.clone()) {
            self.redo_stack.push(current);
            self.timeline = Some(previous);
            self.dirty = true;
            self.selected_key = None;
        }
    }

    /// Steps forward one undone edit.
    pub fn redo(&mut self) {
        if let (Some(next), Some(current)) = (self.redo_stack.pop(), self.timeline.clone()) {
            self.undo_stack.push(current);
            self.timeline = Some(next);
            self.dirty = true;
            self.selected_key = None;
        }
    }

    /// Adds a key to the selected key's track, at the playhead.
    ///
    /// A camera key takes the editor camera's position and target, which are
    /// exactly a `CameraKey`'s `position` and `look_at` -- so framing a shot
    /// and pressing Add is the whole gesture. The other kinds have no such
    /// source and start empty, which reads as incomplete rather than plausibly
    /// wrong.
    fn add_key(&mut self, insp: &bsengine_core::InspectorState) {
        use bsengine_core::timeline::{AnimationKey, CameraKey, EventKey, ShotCut};

        let Some((track_index, _)) = self.selected_key else {
            self.status = Some("select a key first, to say which track".to_string());
            return;
        };
        self.push_undo();
        let time = self.time;
        let (cam_pos, cam_target) = (insp.editor_cam_pos, insp.cam_target);
        let Some(track) = self
            .timeline
            .as_mut()
            .and_then(|t| t.tracks.get_mut(track_index))
        else {
            return;
        };
        let at = match track {
            Track::Camera { keys } => {
                let at = keys.partition_point(|k| k.time <= time);
                keys.insert(
                    at,
                    CameraKey {
                        time,
                        position: cam_pos,
                        look_at: cam_target,
                    },
                );
                at
            }
            Track::CameraShot { cuts } => {
                let at = cuts.partition_point(|k| k.time <= time);
                cuts.insert(
                    at,
                    ShotCut {
                        time,
                        entity: String::new(),
                    },
                );
                at
            }
            Track::Animation { keys, .. } => {
                let at = keys.partition_point(|k| k.time <= time);
                keys.insert(
                    at,
                    AnimationKey {
                        time,
                        clip: String::new(),
                    },
                );
                at
            }
            Track::Event { keys } => {
                let at = keys.partition_point(|k| k.time <= time);
                keys.insert(
                    at,
                    EventKey {
                        time,
                        name: String::new(),
                    },
                );
                at
            }
        };
        self.selected_key = Some((track_index, at));
        self.dirty = true;
    }

    /// Removes the selected key.
    fn delete_selected(&mut self) {
        let Some((track_index, key_index)) = self.selected_key else {
            return;
        };
        self.push_undo();
        if let Some(track) = self
            .timeline
            .as_mut()
            .and_then(|t| t.tracks.get_mut(track_index))
        {
            match track {
                Track::Camera { keys } if key_index < keys.len() => {
                    keys.remove(key_index);
                }
                Track::CameraShot { cuts } if key_index < cuts.len() => {
                    cuts.remove(key_index);
                }
                Track::Animation { keys, .. } if key_index < keys.len() => {
                    keys.remove(key_index);
                }
                Track::Event { keys } if key_index < keys.len() => {
                    keys.remove(key_index);
                }
                _ => {}
            }
        }
        self.selected_key = None;
        self.dirty = true;
    }

    /// Appends a track.
    fn add_track(&mut self, track: Track) {
        self.push_undo();
        if let Some(timeline) = self.timeline.as_mut() {
            timeline.tracks.push(track);
        }
        self.dirty = true;
    }

    /// Removes a track, and clears the selection, which named a track index
    /// that may no longer mean the same track.
    fn delete_track(&mut self, index: usize) {
        self.push_undo();
        if let Some(timeline) = self.timeline.as_mut() {
            if index < timeline.tracks.len() {
                timeline.tracks.remove(index);
            }
        }
        self.selected_key = None;
        self.dirty = true;
    }

    /// Draws editors for the selected key's fields.
    ///
    /// Ordinary widgets in their own strip rather than floating over the
    /// painter-drawn canvas: no coordinate arithmetic, and their labels are
    /// reachable by the galley-walking test helper, which cannot see a rect or
    /// a circle.
    fn draw_value_strip(&mut self, ui: &mut egui::Ui) {
        self.value_strip_shown = false;
        let Some((track_index, key_index)) = self.selected_key else {
            return;
        };
        // Both `self` writes happen before the mutable borrow below is taken:
        // setting a field on `self` while `track` is alive is a borrow error,
        // and that borrow lives until its last use inside the closure.
        if self
            .timeline
            .as_ref()
            .and_then(|t| t.tracks.get(track_index))
            .is_none()
        {
            return;
        }
        self.value_strip_shown = true;
        ui.separator();

        let mut changed = false;
        let Some(track) = self
            .timeline
            .as_mut()
            .and_then(|t| t.tracks.get_mut(track_index))
        else {
            return;
        };
        ui.horizontal(|ui| match track {
            Track::Camera { keys } if key_index < keys.len() => {
                let key = &mut keys[key_index];
                ui.label("position");
                for a in key.position.iter_mut() {
                    changed |= ui.add(egui::DragValue::new(a).speed(0.05_f32)).changed();
                }
                ui.label("look at");
                for a in key.look_at.iter_mut() {
                    changed |= ui.add(egui::DragValue::new(a).speed(0.05_f32)).changed();
                }
            }
            Track::CameraShot { cuts } if key_index < cuts.len() => {
                ui.label("entity");
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut cuts[key_index].entity))
                    .changed();
            }
            Track::Animation { keys, entity } if key_index < keys.len() => {
                ui.label("entity");
                changed |= ui.add(egui::TextEdit::singleline(entity)).changed();
                ui.label("clip");
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut keys[key_index].clip))
                    .changed();
            }
            Track::Event { keys } if key_index < keys.len() => {
                ui.label("name");
                changed |= ui
                    .add(egui::TextEdit::singleline(&mut keys[key_index].name))
                    .changed();
            }
            _ => {}
        });
        if changed {
            self.dirty = true;
        }
    }

    /// Saves and clears the dirty flag.
    ///
    /// Separate from [`TimelinePanel::save`] so `save` stays a pure write with
    /// no state change, which is what lets the boundary test call it without
    /// perturbing anything else.
    ///
    /// # Errors
    ///
    /// As [`TimelinePanel::save`].
    pub fn save_and_clear(&mut self) -> Result<(), String> {
        self.save()?;
        self.dirty = false;
        Ok(())
    }

    /// Re-reads the open file, discarding unsaved edits.
    ///
    /// Until Save, the file on disk *is* the last saved state, so revert is
    /// simply load again -- and load already clears the dirty flag and both
    /// undo stacks.
    ///
    /// # Errors
    ///
    /// As [`TimelinePanel::load`], plus a message when nothing is open.
    pub fn revert(&mut self) -> Result<(), String> {
        let path = self
            .path
            .clone()
            .ok_or_else(|| "no timeline is open to revert".to_string())?;
        self.load(path)
    }

    /// Gives up unsaved edits so the panel can follow the selection again.
    fn discard(&mut self) {
        self.dirty = false;
        self.path = None;
    }

    /// Writes the timeline back to the file it was loaded from.
    ///
    /// Writes `path`, never `path_buffer`: the two are separate precisely so a
    /// half-typed name in the text box cannot become the file that is written.
    ///
    /// Pretty rather than compact for the reason `ShaderGraphPanel::save`
    /// records -- these are committed assets, and one long line is
    /// unreviewable in a diff.
    ///
    /// # Errors
    ///
    /// Returns a message when nothing is open, when serialisation fails, or
    /// when the write fails. A save that cannot happen must show a status line
    /// rather than take the editor down.
    pub fn save(&self) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| "no timeline is open to save to".to_string())?;
        let timeline = self
            .timeline
            .as_ref()
            .ok_or_else(|| "no timeline is open to save to".to_string())?;
        // `struct_names(true)` keeps the leading `Timeline(` that hand-written
        // files carry. The default drops it, and a file that opens with a bare
        // `(` no longer says what it is -- which matters more here than for
        // `.shadergraph.ron`, whose committed form never had the name.
        let body = ron::ser::to_string_pretty(
            timeline,
            ron::ser::PrettyConfig::default().struct_names(true),
        )
        .map_err(|e| format!("{}: {e}", path.display()))?;
        let text = format!("{}{body}", self.leading_comments);
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// The loaded timeline's duration, for the round-trip test.
    #[cfg(test)]
    fn duration_for_test(&self) -> f32 {
        self.duration()
    }

    /// Sets the duration, for the round-trip test.
    #[cfg(test)]
    fn set_duration_for_test(&mut self, duration: f32) {
        if let Some(t) = self.timeline.as_mut() {
            t.duration = duration;
        }
    }

    #[cfg(test)]
    fn add_key_for_test(&mut self, insp: &bsengine_core::InspectorState) {
        self.add_key(insp);
    }

    #[cfg(test)]
    fn delete_selected_for_test(&mut self) {
        self.delete_selected();
    }

    #[cfg(test)]
    fn add_track_for_test(&mut self, track: Track) {
        self.add_track(track);
    }

    #[cfg(test)]
    fn delete_track_for_test(&mut self, index: usize) {
        self.delete_track(index);
    }

    #[cfg(test)]
    fn set_time_for_test(&mut self, time: f32) {
        self.time = time;
    }

    #[cfg(test)]
    fn select_for_test(&mut self, track: usize, key: usize) {
        self.selected_key = Some((track, key));
    }

    /// A camera key's authored values, for the capture test.
    #[cfg(test)]
    fn camera_key_values_for_test(&self, track: usize, key: usize) -> ([f32; 3], [f32; 3]) {
        match self.timeline.as_ref().and_then(|t| t.tracks.get(track)) {
            Some(Track::Camera { keys }) => (keys[key].position, keys[key].look_at),
            _ => panic!("track {track} is not a camera track"),
        }
    }

    /// An event key's name, for the empty-add and value tests.
    #[cfg(test)]
    fn event_name_for_test(&self, track: usize, key: usize) -> String {
        match self.timeline.as_ref().and_then(|t| t.tracks.get(track)) {
            Some(Track::Event { keys }) => keys[key].name.clone(),
            _ => panic!("track {track} is not an event track"),
        }
    }

    /// Edits the selected camera key's position, standing in for the drag the
    /// value strip's `DragValue` performs.
    #[cfg(test)]
    fn set_camera_position_for_test(&mut self, position: [f32; 3]) {
        let Some((t, k)) = self.selected_key else {
            return;
        };
        if let Some(Track::Camera { keys }) =
            self.timeline.as_mut().and_then(|tl| tl.tracks.get_mut(t))
        {
            keys[k].position = position;
            self.dirty = true;
        }
    }

    /// Edits the selected shot key's entity, standing in for typing into the
    /// value strip's `TextEdit`.
    #[cfg(test)]
    fn set_shot_entity_for_test(&mut self, entity: &str) {
        let Some((t, k)) = self.selected_key else {
            return;
        };
        if let Some(Track::CameraShot { cuts }) =
            self.timeline.as_mut().and_then(|tl| tl.tracks.get_mut(t))
        {
            cuts[k].entity = entity.to_string();
            self.dirty = true;
        }
    }

    #[cfg(test)]
    fn shot_entity_for_test(&self, track: usize, key: usize) -> String {
        match self.timeline.as_ref().and_then(|t| t.tracks.get(track)) {
            Some(Track::CameraShot { cuts }) => cuts[key].entity.clone(),
            _ => panic!("track {track} is not a shot track"),
        }
    }

    #[cfg(test)]
    fn discard_for_test(&mut self) {
        self.discard();
    }

    #[cfg(test)]
    fn mark_dirty_for_test(&mut self) {
        self.dirty = true;
    }

    /// One key's time. For tests, which must not reach into the timeline.
    #[cfg(test)]
    fn key_time_for_test(&self, track: usize, key: usize) -> f32 {
        self.key_times_for_test(track)[key]
    }

    /// Every key time in a track, in stored order -- so a test can assert the
    /// order itself and not merely the values.
    #[cfg(test)]
    fn key_times_for_test(&self, track: usize) -> Vec<f32> {
        self.timeline
            .as_ref()
            .and_then(|t| t.tracks.get(track))
            .map(Self::track_key_times)
            .unwrap_or_default()
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

        // One interaction region covering the ruler and every lane, so a
        // press anywhere on the time axis scrubs. Allocated after the lanes
        // are laid out, so `last_lane_area` is already the rect being mapped.
        let scrub_area = egui::Rect::from_min_max(
            egui::pos2(lane_left, full.top()),
            egui::pos2(full.right(), lanes_top + lanes_height),
        );
        let response = ui.allocate_rect(scrub_area, egui::Sense::click_and_drag());

        // Decide once what the press grabbed, and push a single undo point for
        // the whole drag -- not per frame, which would bury the edit.
        if response.drag_started() || response.clicked() {
            // The press origin, not `interact_pointer_pos()`: egui reports a
            // drag as started on the frame the pointer first *moved* while
            // held, and by then the interact position has advanced to where it
            // moved to. Grabbing there misses the key whenever the first frame
            // of movement exceeds the grab radius -- which is every drag worth
            // making. `ShaderGraphPanel` grabs ports the same way.
            let press = ui
                .ctx()
                .input(|i| i.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos());
            let grabbed = press.and_then(|p| self.key_at(p));
            self.selected_key = grabbed;
            if grabbed.is_some() {
                self.push_undo();
            }
            self.dragging = Some(match grabbed {
                Some((track, key)) => Grab::Key { track, key },
                None => Grab::Scrub,
            });
        }

        if let (Some(grab), Some(pointer)) = (self.dragging, response.interact_pointer_pos()) {
            let t = self.x_to_time(pointer.x);
            match grab {
                Grab::Scrub => self.time = t,
                Grab::Key { track, key } => {
                    if let Some(tr) = self
                        .timeline
                        .as_mut()
                        .and_then(|tl| tl.tracks.get_mut(track))
                    {
                        let now = move_key(tr, key, t);
                        self.dragging = Some(Grab::Key { track, key: now });
                        self.selected_key = Some((track, now));
                        self.dirty = true;
                    }
                }
            }
        }

        if response.drag_stopped() {
            self.dragging = None;
        }

        let playhead_x = self.time_to_x(self.time);
        painter.line_segment(
            [
                egui::pos2(playhead_x, full.top()),
                egui::pos2(playhead_x, lanes_top + lanes_height),
            ],
            egui::Stroke::new(2.0_f32, visuals.error_fg_color),
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

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        self.resolve_source(ctx);

        let mut open_clicked = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.path_buffer)
                    .hint_text("assets/timelines/intro.ron")
                    .desired_width(260.0_f32),
            );
            open_clicked = ui
                .button(format!("{} Open", egui_phosphor::regular::FOLDER_OPEN))
                .clicked();
            ui.checkbox(&mut self.preview, "Preview");
            if ui
                .add_enabled(
                    self.path.is_some(),
                    egui::Button::new(format!("{} Save", egui_phosphor::regular::FLOPPY_DISK)),
                )
                .clicked()
            {
                self.status = Some(match self.save_and_clear() {
                    Ok(()) => "saved".to_string(),
                    Err(e) => e,
                });
            }
            if ui
                .add_enabled(
                    self.path.is_some() && self.dirty,
                    egui::Button::new("Revert"),
                )
                .clicked()
            {
                if let Err(e) = self.revert() {
                    self.status = Some(e);
                }
            }
            if ui
                .add_enabled(self.dirty, egui::Button::new("Discard"))
                .clicked()
            {
                self.discard();
            }
            if ui
                .add_enabled(!self.undo_stack.is_empty(), egui::Button::new("Undo"))
                .clicked()
            {
                self.undo();
            }
            if ui
                .add_enabled(!self.redo_stack.is_empty(), egui::Button::new("Redo"))
                .clicked()
            {
                self.redo();
            }
            ui.label(format!("t = {:.2}", self.time));
            if let Some(timeline) = self.timeline.as_mut() {
                ui.label("duration");
                if ui
                    .add(egui::DragValue::new(&mut timeline.duration).speed(0.1_f32))
                    .changed()
                {
                    self.dirty = true;
                }
            }
        });
        if let Some(status) = &self.status {
            ui.label(status.clone());
        }

        // The text field is read only when Open is clicked, so a half-typed
        // path never becomes the loaded path.
        if open_clicked {
            let path = PathBuf::from(self.path_buffer.trim());
            self.explicit_path = Some(path.clone());
            match self.load(&path) {
                Ok(()) => self.status = Some(format!("opened {}", path.display())),
                Err(e) => {
                    self.status = Some(e);
                    self.timeline = None;
                    self.path = None;
                }
            }
        }

        // The editing row. In `ui` rather than `draw_tracks` because the
        // camera capture needs `ctx.insp`, which `draw_tracks` has no access
        // to.
        ui.horizontal(|ui| {
            let has_selection = self.selected_key.is_some();
            if ui
                .add_enabled(has_selection, egui::Button::new("Add Key"))
                .clicked()
            {
                self.add_key(ctx.insp);
            }
            if ui
                .add_enabled(has_selection, egui::Button::new("Delete Key"))
                .clicked()
            {
                self.delete_selected();
            }
            ui.menu_button("Add Track", |ui| {
                let kinds: [(&str, Track); 4] = [
                    ("Camera", Track::Camera { keys: Vec::new() }),
                    ("Shots", Track::CameraShot { cuts: Vec::new() }),
                    (
                        "Animation",
                        Track::Animation {
                            entity: String::new(),
                            keys: Vec::new(),
                        },
                    ),
                    ("Events", Track::Event { keys: Vec::new() }),
                ];
                for (label, track) in kinds {
                    if ui.button(label).clicked() {
                        self.add_track(track);
                        ui.close_menu();
                        break;
                    }
                }
            });
            if let Some((track, _)) = self.selected_key {
                if ui.button("Delete Track").clicked() {
                    self.delete_track(track);
                }
            }
        });

        ui.separator();
        self.draw_tracks(ui);
        self.draw_value_strip(ui);

        // Republished every frame while previewing, and cleared the moment it
        // is not, so a stale request cannot outlive the toggle.
        ctx.insp.timeline_preview = if self.preview {
            self.build_preview(ctx)
        } else {
            None
        };
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

    /// Writes a timeline to a temp file so the entry-path tests drive the
    /// real parse rather than a hand-built value.
    struct TimelineFile(std::path::PathBuf);

    impl Drop for TimelineFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }

    fn write_timeline(timeline: &Timeline) -> TimelineFile {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "bsengine-timeline-panel-{}-{}.ron",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::write(&path, ron::to_string(timeline).expect("serialise")).expect("write");
        TimelineFile(path)
    }

    /// An empty timeline, which draws zero lanes -- so a test can tell "the
    /// other file loaded" from "the four-track file loaded" by lane count
    /// alone.
    fn empty_timeline() -> Timeline {
        Timeline {
            duration: 1.0,
            tracks: Vec::new(),
        }
    }

    /// Makes `id` the selection and gives it a `TimelinePlayer` naming
    /// `path`, the way the editor populates `reflected_components` for the
    /// selected entity.
    fn select_timeline_entity(h: &mut Harness, id: u64, name: &str, path: &std::path::Path) {
        h.insp.selected_id = Some(id);
        h.insp.reflected_components = vec![(
            "bsengine_core::timeline::TimelinePlayer".to_string(),
            Box::new(bsengine_core::timeline::TimelinePlayer {
                timeline: path.to_string_lossy().to_string(),
                time: 0.0,
                playing: false,
                speed: 1.0,
            }) as Box<dyn bevy_reflect::Reflect>,
        )];
        h.entities_snapshot = vec![InspectorEntityInfo {
            id,
            name: Some(name.to_string()),
            ..Default::default()
        }];
    }

    /// With Preview on, the panel publishes the pose `evaluate` gives for the
    /// playhead.
    #[test]
    fn preview_publishes_the_camera_pose_for_the_playhead() {
        let mut h = Harness::new(four_track_timeline());
        h.panel.preview = true;
        h.settle();

        let y = h.panel.last_lane_rects[0].center().y;
        let x = h.panel.time_to_x(3.0);
        h.press(egui::pos2(x, y));
        h.drag_to(egui::pos2(x, y));
        h.release(egui::pos2(x, y));
        h.draw();

        let preview = h.insp.timeline_preview.clone().expect("a preview request");
        let camera = preview.camera.expect("a camera pose");
        // Half way along a 0..12 dolly over 6 seconds.
        assert!(
            (camera.position[0] - 6.0).abs() < 0.2,
            "expected x near 6.0 at t=3.0, got {}",
            camera.position[0]
        );
    }

    /// A cut resolves against the scene, not the timeline: the pose comes
    /// from the named entity's snapshot entry, including its field of view.
    ///
    /// Bounded on both sides. "Past where the dolly was" is not the same
    /// claim as "at the shot", and only the upper bound rules out a preview
    /// that simply kept dollying.
    #[test]
    fn a_cut_previews_the_shot_entitys_pose() {
        let mut h = Harness::new(four_track_timeline());
        h.entities_snapshot = vec![InspectorEntityInfo {
            id: 1,
            name: Some("CloseUpShot".to_string()),
            position: Some([1.5, 1.2, 2.5]),
            rotation: Some([0.0, 0.0, 0.0]),
            camera_fov: Some(35.0),
            ..Default::default()
        }];
        h.panel.preview = true;
        h.settle();

        let y = h.panel.last_lane_rects[0].center().y;
        let x = h.panel.time_to_x(4.0); // past the cut at 3.5
        h.press(egui::pos2(x, y));
        h.drag_to(egui::pos2(x, y));
        h.release(egui::pos2(x, y));
        h.draw();

        let camera = h
            .insp
            .timeline_preview
            .clone()
            .expect("a preview request")
            .camera
            .expect("a camera pose");
        assert!(
            (camera.position[0] - 1.5).abs() < 1e-3,
            "expected the shot entity's x=1.5, got {} -- the dolly would be \
             past x=8 by now",
            camera.position[0]
        );
        assert_eq!(
            camera.fov_y_degrees,
            Some(35.0),
            "a shot carries its own field of view"
        );
    }

    /// Paired with the above: Preview off must publish nothing at all.
    /// Without this, a panel that always previews passes the test above.
    #[test]
    fn preview_off_publishes_nothing() {
        let mut h = Harness::new(four_track_timeline());
        h.panel.preview = false;
        h.settle();

        assert!(
            h.insp.timeline_preview.is_none(),
            "preview is off, so the panel must publish no request"
        );
    }

    #[test]
    fn selecting_an_entity_opens_its_timeline() {
        let file = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        select_timeline_entity(&mut h, 7, "Director", &file.0);
        h.settle();

        assert_eq!(
            h.panel.last_lane_rects.len(),
            4,
            "selecting an entity with a TimelinePlayer must load the timeline \
             it names"
        );
    }

    /// An explicitly opened path wins over the selection, and keeps winning
    /// while the same entity stays selected.
    #[test]
    fn an_explicitly_opened_path_overrides_the_selection() {
        let selected = write_timeline(&empty_timeline());
        let opened = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        select_timeline_entity(&mut h, 7, "Director", &selected.0);
        h.settle();
        assert_eq!(
            h.panel.last_lane_rects.len(),
            0,
            "the selection's timeline has no tracks"
        );

        h.panel.open_for_test(&opened.0);
        h.settle();

        assert_eq!(
            h.panel.last_lane_rects.len(),
            4,
            "the opened path must win over the still-selected entity"
        );
    }

    /// Paired with the above: selecting a *different* entity that has a
    /// `TimelinePlayer` clears the override. Without this the override would
    /// be a trap -- the panel would keep showing a stale file forever -- and
    /// the test above alone cannot tell the difference.
    #[test]
    fn selecting_another_timeline_entity_clears_the_override() {
        let opened = write_timeline(&four_track_timeline());
        let other = write_timeline(&empty_timeline());
        let mut h = Harness::new(empty_timeline());

        h.panel.open_for_test(&opened.0);
        h.settle();
        assert_eq!(h.panel.last_lane_rects.len(), 4);

        select_timeline_entity(&mut h, 9, "OtherDirector", &other.0);
        h.settle();

        assert_eq!(
            h.panel.last_lane_rects.len(),
            0,
            "selecting a different entity with a TimelinePlayer must resume \
             following the selection"
        );
    }

    /// Only Save writes. Scrubbing and previewing still must not, which was
    /// 2/2a's whole boundary -- this is that test rewritten rather than
    /// deleted, so the guarantee narrowed on purpose instead of lapsing.
    ///
    /// Asserts the bytes rather than the modification time, because a write
    /// that happens to produce identical content is still a write path, and
    /// mtime granularity is coarse enough on some filesystems to miss a fast
    /// one.
    #[test]
    fn only_saving_writes_to_the_timeline_file() {
        let file = write_timeline(&four_track_timeline());
        let before = std::fs::read(&file.0).expect("read back");

        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.panel.preview = true;
        h.settle();

        let y = h.panel.last_lane_rects[0].center().y;
        for t in [0.5_f32, 2.5, 4.0, 5.9] {
            let x = h.panel.time_to_x(t);
            h.press(egui::pos2(x, y));
            h.drag_to(egui::pos2(x, y));
            h.release(egui::pos2(x, y));
        }

        assert_eq!(
            before,
            std::fs::read(&file.0).expect("read back"),
            "scrubbing and previewing must still not touch the file"
        );

        h.panel.save().expect("save");
        assert_ne!(
            before,
            std::fs::read(&file.0).expect("read back"),
            "and Save must actually write -- otherwise the assertion above \
             passes for a panel with no write path at all"
        );
    }

    /// Saving writes the data back so a reload sees it. The round trip is what
    /// exercises save and load together -- asserting on the written text alone
    /// would pass for a file that cannot be parsed back.
    #[test]
    fn saving_then_loading_round_trips_the_timeline() {
        let file = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.settle();

        h.panel.set_duration_for_test(9.5);
        h.panel.save().expect("save");

        let mut reloaded = Harness::new(empty_timeline());
        reloaded.panel.open_for_test(&file.0);
        reloaded.settle();
        assert!(
            (reloaded.panel.duration_for_test() - 9.5).abs() < 1e-6,
            "expected the saved 9.5, got {}",
            reloaded.panel.duration_for_test()
        );
    }

    /// A hand-written timeline's leading comments survive a save. RON has no
    /// comments in its data model, so without this they vanish the first time
    /// anyone presses Save on a committed asset.
    #[test]
    fn saving_keeps_the_leading_comment_block() {
        let file = write_timeline(&four_track_timeline());
        let original = std::fs::read_to_string(&file.0).expect("read");
        std::fs::write(
            &file.0,
            format!("// why this cutscene exists\n// second line\n{original}"),
        )
        .expect("write");

        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.settle();
        h.panel.save().expect("save");

        let saved = std::fs::read_to_string(&file.0).expect("read back");
        assert!(
            saved.starts_with("// why this cutscene exists\n// second line\n"),
            "the leading comments must come back verbatim, got:\n{saved}"
        );
    }

    /// Paired with the above: a file with no leading comments must not gain
    /// any. Without this, an implementation that always writes a header passes
    /// the test above.
    #[test]
    fn saving_a_file_without_comments_invents_none() {
        let file = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.settle();
        h.panel.save().expect("save");

        let saved = std::fs::read_to_string(&file.0).expect("read back");
        assert!(
            !saved.starts_with("//"),
            "nothing was there to preserve, got:\n{saved}"
        );
    }

    /// Saving with nothing open is an error value, not a panic. The button is
    /// disabled in that state, but a disabled button is a UI fact and this is
    /// the guarantee.
    #[test]
    fn saving_with_no_file_open_is_an_error() {
        let panel = TimelinePanel::default();
        assert!(panel.save().is_err());
    }

    /// A saved file still names its types, so it stays as readable as the
    /// hand-written one it replaces. `PrettyConfig::default()` drops struct
    /// names, which turns `Timeline(` into a bare `(` -- still parseable, but
    /// a file that no longer says what it is.
    #[test]
    fn a_saved_timeline_still_names_its_types() {
        let file = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.settle();
        h.panel.save().expect("save");

        let saved = std::fs::read_to_string(&file.0).expect("read back");
        assert!(
            saved.contains("Timeline("),
            "the struct name must survive, got:\n{saved}"
        );
        assert!(
            saved.contains("Camera(") && saved.contains("CameraKey("),
            "variant and key type names too, got:\n{saved}"
        );
    }

    /// A new camera key carries the editor camera's pose -- "frame the shot,
    /// then add a key" -- rather than a key at the origin to be retyped.
    #[test]
    fn adding_a_camera_key_captures_the_editor_camera() {
        let mut h = Harness::new(four_track_timeline());
        h.insp.editor_cam_pos = [3.0, 4.0, 5.0];
        h.insp.cam_target = [1.0, 2.0, 3.0];
        h.settle();

        h.panel.select_for_test(0, 0);
        h.panel.set_time_for_test(2.0);
        let insp = std::mem::take(&mut h.insp);
        h.panel.add_key_for_test(&insp);
        h.insp = insp;

        let (position, look_at) = h.panel.camera_key_values_for_test(0, 1);
        assert_eq!(position, [3.0, 4.0, 5.0], "not zeros -- the editor camera");
        assert_eq!(look_at, [1.0, 2.0, 3.0]);
        assert!((h.panel.key_time_for_test(0, 1) - 2.0).abs() < 1e-6);
    }

    /// The gap the coverage audit predicted: only the camera kind had an
    /// add-key test. A non-camera key arrives empty at the playhead, which is
    /// visibly incomplete rather than plausibly wrong.
    #[test]
    fn adding_an_event_key_arrives_empty_at_the_playhead() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        h.panel.select_for_test(3, 0);
        h.panel.set_time_for_test(2.0);
        let insp = std::mem::take(&mut h.insp);
        h.panel.add_key_for_test(&insp);
        h.insp = insp;

        assert_eq!(h.panel.key_times_for_test(3).len(), 2);
        assert!((h.panel.key_time_for_test(3, 0) - 2.0).abs() < 1e-6);
        assert_eq!(
            h.panel.event_name_for_test(3, 0),
            "",
            "an added event key has no name until one is typed"
        );
    }

    /// Delete removes the selected key, and the *right* one.
    #[test]
    fn deleting_removes_the_selected_key() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        assert_eq!(h.panel.key_times_for_test(0).len(), 2);

        h.panel.select_for_test(0, 0);
        h.panel.delete_selected_for_test();

        let times = h.panel.key_times_for_test(0);
        assert_eq!(times.len(), 1);
        assert!(
            (times[0] - 6.0).abs() < 1e-6,
            "the key at 0.0 was selected, so 6.0 is what must be left: {times:?}"
        );
    }

    /// Paired: deleting with nothing selected removes nothing.
    #[test]
    fn deleting_with_no_selection_removes_nothing() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        h.panel.delete_selected_for_test();
        assert_eq!(h.panel.key_times_for_test(0).len(), 2);
    }

    /// Adding a track adds a lane, and the new track is empty.
    #[test]
    fn adding_a_track_adds_an_empty_lane() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        assert_eq!(h.panel.last_lane_rects.len(), 4);

        h.panel
            .add_track_for_test(Track::Event { keys: Vec::new() });
        h.settle();

        assert_eq!(h.panel.last_lane_rects.len(), 5);
        assert!(h.panel.key_times_for_test(4).is_empty());
    }

    /// Deleting a track removes its lane and re-indexes the keys after it --
    /// track indices shift, and `last_key_positions` is keyed by them.
    #[test]
    fn deleting_a_track_reindexes_the_remaining_keys() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        // Track 1 is the single-cut shot track; deleting it must slide the
        // animation track from index 2 to 1.
        h.panel.delete_track_for_test(1);
        h.settle();

        assert_eq!(h.panel.last_lane_rects.len(), 3);
        assert!(
            (h.panel.key_time_for_test(1, 0) - 0.5).abs() < 1e-6,
            "the animation key at 0.5 should now be track 1"
        );
        assert!(
            h.panel.last_key_positions.contains_key(&(1, 0)),
            "recorded positions must be re-keyed to the new indices"
        );
        assert!(
            !h.panel.last_key_positions.contains_key(&(3, 0)),
            "and the old fourth track must be gone"
        );
    }

    /// The value strip edits the selected key in place.
    #[test]
    fn editing_a_shot_entity_changes_the_key() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        h.panel.select_for_test(1, 0);

        h.panel.set_shot_entity_for_test("NewShot");

        assert_eq!(h.panel.shot_entity_for_test(1, 0), "NewShot");
        assert!(h.panel.is_dirty(), "a value edit is an edit");
    }

    /// The second gap the audit predicted: only the shot kind had a value
    /// test. A camera key's position is editable too.
    #[test]
    fn editing_a_camera_position_changes_the_key() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        h.panel.select_for_test(0, 0);

        h.panel.set_camera_position_for_test([7.0, 8.0, 9.0]);

        assert_eq!(h.panel.camera_key_values_for_test(0, 0).0, [7.0, 8.0, 9.0]);
        assert!(h.panel.is_dirty());
    }

    /// Paired: the strip draws nothing when no key is selected, so it cannot
    /// edit something the user is not pointing at.
    #[test]
    fn the_value_strip_is_absent_without_a_selection() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        assert!(!h.panel.value_strip_shown());

        h.panel.select_for_test(1, 0);
        h.settle();
        assert!(h.panel.value_strip_shown());
    }

    /// An edit marks the panel dirty and the selection can no longer pull the
    /// file out from under it, so a stray click cannot discard work.
    #[test]
    fn a_dirty_panel_refuses_to_follow_the_selection() {
        let edited = write_timeline(&four_track_timeline());
        let other = write_timeline(&empty_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&edited.0);
        h.settle();

        let from = h.panel.last_key_positions[&(2, 0)];
        let to = egui::pos2(h.panel.time_to_x(4.0), from.y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();
        assert!(h.panel.is_dirty(), "an edit must mark the panel dirty");

        select_timeline_entity(&mut h, 9, "OtherDirector", &other.0);
        h.settle();

        assert_eq!(
            h.panel.last_lane_rects.len(),
            4,
            "the edited timeline must still be shown, not silently replaced"
        );
    }

    /// Paired: Discard gives up the edits and lets the switch happen. Without
    /// it the guard would be a trap with no way out.
    #[test]
    fn discarding_lets_the_selection_switch() {
        let edited = write_timeline(&four_track_timeline());
        let other = write_timeline(&empty_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&edited.0);
        h.settle();

        let from = h.panel.last_key_positions[&(2, 0)];
        let to = egui::pos2(h.panel.time_to_x(4.0), from.y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();

        select_timeline_entity(&mut h, 9, "OtherDirector", &other.0);
        h.settle();
        h.panel.discard_for_test();
        h.settle();

        assert_eq!(
            h.panel.last_lane_rects.len(),
            0,
            "after discarding, the selection's timeline is shown"
        );
    }

    /// Saving clears dirty -- otherwise the guard would keep refusing after
    /// the work was safely on disk.
    #[test]
    fn saving_clears_dirty() {
        let file = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.settle();
        h.panel.set_duration_for_test(2.0);
        h.panel.mark_dirty_for_test();
        assert!(h.panel.is_dirty());

        h.panel.save_and_clear().expect("save");

        assert!(!h.panel.is_dirty());
    }

    /// Revert re-reads the file, throwing away unsaved edits.
    #[test]
    fn revert_restores_the_file_on_disk() {
        let file = write_timeline(&four_track_timeline());
        let mut h = Harness::new(empty_timeline());
        h.panel.open_for_test(&file.0);
        h.settle();
        let before = h.panel.key_time_for_test(2, 0);

        let from = h.panel.last_key_positions[&(2, 0)];
        let to = egui::pos2(h.panel.time_to_x(4.0), from.y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();
        assert!(h.panel.is_dirty());

        h.panel.revert().expect("revert");

        assert!(
            (h.panel.key_time_for_test(2, 0) - before).abs() < 1e-6,
            "revert must restore the key's time"
        );
        assert!(!h.panel.is_dirty(), "revert leaves nothing unsaved");
        assert_eq!(h.panel.undo_depth(), 0, "and no history from the old file");
    }

    /// Clicking a key selects it; the value strip and Delete act on the
    /// selection, so it is its own state rather than a derived one.
    #[test]
    fn clicking_a_key_selects_it() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let pos = h.panel.last_key_positions[&(2, 0)];
        h.press(pos);
        h.release(pos);
        h.draw();

        assert_eq!(h.panel.selected_key(), Some((2, 0)));
    }

    /// Paired: clicking empty lane space clears it, so Delete is only live
    /// while something is actually pointed at.
    #[test]
    fn clicking_empty_space_clears_the_selection() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let pos = h.panel.last_key_positions[&(2, 0)];
        h.press(pos);
        h.release(pos);
        h.draw();
        assert!(h.panel.selected_key().is_some());

        let empty = egui::pos2(
            h.panel.time_to_x(4.7),
            h.panel.last_lane_rects[3].center().y,
        );
        h.press(empty);
        h.release(empty);
        h.draw();

        assert_eq!(h.panel.selected_key(), None);
    }

    /// Dragging a key moves it to the dragged time.
    #[test]
    fn dragging_a_key_moves_it() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let from = h.panel.last_key_positions[&(2, 0)];
        let to = egui::pos2(h.panel.time_to_x(4.0), from.y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();

        assert!(
            (h.panel.key_time_for_test(2, 0) - 4.0).abs() < 0.05,
            "expected 4.0, got {}",
            h.panel.key_time_for_test(2, 0)
        );
    }

    /// Paired: a press that is not on a key scrubs instead and leaves every
    /// key where it was. Without this, "the key moved" passes for an
    /// implementation that moves a key on any press anywhere.
    #[test]
    fn dragging_empty_space_scrubs_and_moves_no_key() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        let before = h.panel.key_time_for_test(2, 0);

        let y = h.panel.last_lane_rects[3].center().y;
        let from = egui::pos2(h.panel.time_to_x(1.0), y);
        let to = egui::pos2(h.panel.time_to_x(4.0), y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();

        assert!(
            (h.panel.time() - 4.0).abs() < 0.05,
            "it should have scrubbed"
        );
        assert!(
            (h.panel.key_time_for_test(2, 0) - before).abs() < 1e-6,
            "no key may move"
        );
    }

    /// Dragging a key past its neighbour keeps the track sorted. `evaluate`
    /// and `events_between` read keys in order, so an out-of-order track is
    /// silently wrong rather than loudly broken.
    #[test]
    fn dragging_a_key_past_its_neighbour_keeps_the_track_sorted() {
        // A fixture whose keys can actually cross. `four_track_timeline`'s
        // camera keys sit at 0.0 and 6.0 with a duration of 6.0, so the clamp
        // stops the first from ever passing the second -- a test built on it
        // stays green even when the sorted insert is replaced by an assignment
        // in place. Mutation testing is what exposed that; the assertion was
        // broad and the case was too narrow to reach it.
        let crossable = Timeline {
            duration: 6.0,
            tracks: vec![Track::Camera {
                keys: vec![
                    CameraKey {
                        time: 1.0,
                        position: [0.0, 0.0, 0.0],
                        look_at: [0.0, 0.0, -1.0],
                    },
                    CameraKey {
                        time: 3.0,
                        position: [1.0, 0.0, 0.0],
                        look_at: [0.0, 0.0, -1.0],
                    },
                ],
            }],
        };
        let mut h = Harness::new(crossable);
        h.settle();

        // Drag the key at 1.0 to 5.0, which is past the one at 3.0.
        let from = h.panel.last_key_positions[&(0, 0)];
        let to = egui::pos2(h.panel.time_to_x(5.0), from.y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();

        let times = h.panel.key_times_for_test(0);
        assert!(
            times.windows(2).all(|w| w[0] <= w[1]),
            "track 0 is out of order: {times:?}"
        );
        assert!(
            times.iter().any(|t| (t - 5.0).abs() < 0.05),
            "the dragged key should be at 5.0: {times:?}"
        );
    }

    /// Undo restores the timeline as it was before the edit; redo re-applies.
    #[test]
    fn undo_restores_the_previous_timeline_and_redo_reapplies() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        let before = h.panel.key_time_for_test(2, 0);

        let from = h.panel.last_key_positions[&(2, 0)];
        let to = egui::pos2(h.panel.time_to_x(4.0), from.y);
        h.press(from);
        h.drag_to(to);
        h.release(to);
        h.draw();
        let after = h.panel.key_time_for_test(2, 0);
        assert!((after - before).abs() > 0.5, "the edit must have happened");

        h.panel.undo();
        assert!((h.panel.key_time_for_test(2, 0) - before).abs() < 1e-6);

        h.panel.redo();
        assert!((h.panel.key_time_for_test(2, 0) - after).abs() < 1e-6);
    }

    /// Undo with nothing to undo is a no-op, not a panic.
    #[test]
    fn undo_with_an_empty_stack_does_nothing() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        let before = h.panel.key_times_for_test(0);

        h.panel.undo();
        h.panel.redo();

        assert_eq!(h.panel.key_times_for_test(0), before);
    }

    /// A drag spanning many frames is one edit, so one Undo returns the key to
    /// where it started -- not to some point midway through the drag.
    ///
    /// The single-frame undo test above cannot see this: with a push on every
    /// frame it still passes, because its drag has only one frame to push on.
    #[test]
    fn a_multi_frame_drag_is_a_single_undo_step() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();
        let before = h.panel.key_time_for_test(2, 0);

        let from = h.panel.last_key_positions[&(2, 0)];
        h.press(from);
        for t in [1.5_f32, 2.5, 3.5, 4.5] {
            let x = h.panel.time_to_x(t);
            h.drag_to(egui::pos2(x, from.y));
        }
        let end = h.panel.time_to_x(4.5);
        h.release(egui::pos2(end, from.y));
        h.draw();
        assert!(
            (h.panel.key_time_for_test(2, 0) - before).abs() > 0.5,
            "the drag must have moved the key"
        );

        h.panel.undo();

        assert!(
            (h.panel.key_time_for_test(2, 0) - before).abs() < 1e-6,
            "one undo must return the key to where the drag started, got {} \
             (a push-per-frame stack would land it midway)",
            h.panel.key_time_for_test(2, 0)
        );
        assert_eq!(
            h.panel.undo_depth(),
            0,
            "a drag is one edit, so it leaves exactly one entry behind"
        );
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

    /// The central claim, and a round trip rather than a one-way check: press
    /// where the panel says 2.5s is, and the playhead must read 2.5s back.
    ///
    /// A one-way assertion ("the playhead moved right") passes for any
    /// mapping that is merely monotonic, including one off by a constant or
    /// scaled wrongly -- which is exactly how a time axis goes wrong.
    #[test]
    fn pressing_at_a_time_puts_the_playhead_at_that_time() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let target = 2.5;
        let x = h.panel.time_to_x(target);
        let y = h.panel.last_lane_rects[0].center().y;
        h.press(egui::pos2(x, y));
        h.drag_to(egui::pos2(x, y));
        h.release(egui::pos2(x, y));

        assert!(
            (h.panel.time() - target).abs() < 0.05,
            "pressed at time_to_x({target}) but the playhead reads {}",
            h.panel.time()
        );
    }

    /// Paired with the above: a different press must land somewhere
    /// different, or a panel that hardcodes 2.5 passes.
    #[test]
    fn a_different_press_gives_a_different_time() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let y = h.panel.last_lane_rects[0].center().y;
        let x1 = h.panel.time_to_x(1.0);
        h.press(egui::pos2(x1, y));
        h.drag_to(egui::pos2(x1, y));
        h.release(egui::pos2(x1, y));
        let first = h.panel.time();

        let x2 = h.panel.time_to_x(4.0);
        h.press(egui::pos2(x2, y));
        h.drag_to(egui::pos2(x2, y));
        h.release(egui::pos2(x2, y));
        let second = h.panel.time();

        assert!(
            (first - 1.0).abs() < 0.05 && (second - 4.0).abs() < 0.05,
            "expected 1.0 then 4.0, got {first} then {second}"
        );
    }

    /// Dragging past the right edge parks the playhead at the duration
    /// rather than running off the timeline.
    #[test]
    fn dragging_past_the_end_clamps_to_the_duration() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let y = h.panel.last_lane_rects[0].center().y;
        let start = h.panel.time_to_x(3.0);
        h.press(egui::pos2(start, y));
        h.drag_to(egui::pos2(h.screen_rect.right() + 500.0, y));
        h.release(egui::pos2(h.screen_rect.right() + 500.0, y));

        assert!(
            (h.panel.time() - 6.0).abs() < 1e-3,
            "expected the 6.0s duration, got {}",
            h.panel.time()
        );
    }

    /// And past the left edge clamps to zero -- the other side, because a
    /// clamp implemented on one end only passes the test above.
    #[test]
    fn dragging_before_the_start_clamps_to_zero() {
        let mut h = Harness::new(four_track_timeline());
        h.settle();

        let y = h.panel.last_lane_rects[0].center().y;
        let start = h.panel.time_to_x(3.0);
        h.press(egui::pos2(start, y));
        h.drag_to(egui::pos2(h.screen_rect.left() - 500.0, y));
        h.release(egui::pos2(h.screen_rect.left() - 500.0, y));

        assert!(
            h.panel.time().abs() < 1e-3,
            "expected 0.0, got {}",
            h.panel.time()
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
