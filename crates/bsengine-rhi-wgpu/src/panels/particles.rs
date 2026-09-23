//! Particles panel: every emitter in the scene with its live count, the
//! buttons that act on an effect, and the editor's preview controls.
//!
//! The runtime shipped in item 28 and its parameters have been editable in
//! the Inspector through reflection since, so what was missing is what the
//! reference engines put *beside* the parameters: Unity's Particle Effect
//! overlay in the Scene view, Niagara's preview toolbar, Godot's editor-time
//! emission with its Restart. All three converge on the same four things --
//! play/pause the preview, a playback speed, restart the effect, and how many
//! particles are alive -- and that is this panel. It edits no parameters
//! itself; clicking an emitter selects its entity, and the Inspector does the
//! rest, as it does for every other component.
//!
//! Reads `InspectorEntityInfo::particles` (filled by `bsengine-editor` from the
//! live emitters) and writes `InspectorState::particle_preview` plus
//! `InspectorCmd`s; it never touches an emitter directly.

use bsengine_core::{
    EditorPanel, EditorPanelContext, EditorPlayState, InspectorCmd, InspectorEntityInfo,
    InspectorState, ParticleSnapshot,
};

/// Slowest preview speed the slider offers: a tenth of real time is slow
/// enough to see one particle's arc, and slower than that the effect reads
/// as frozen.
const MIN_SPEED: f32 = 0.1;
/// Fastest preview speed the slider offers.
const MAX_SPEED: f32 = 4.0;

/// Lists every emitter with its live count and the preview controls.
#[derive(Default)]
pub struct ParticlePanel;

/// The `[id] name` label the Hierarchy and Inspector both use for an
/// entity, so the same emitter reads the same in all three places.
fn entity_label(info: &InspectorEntityInfo) -> String {
    format!(
        "[{}] {}",
        info.id,
        info.name.as_deref().unwrap_or("(unnamed)")
    )
}

/// The emitters in the snapshot, in snapshot order.
fn emitters(entities: &[InspectorEntityInfo]) -> Vec<(u64, String, ParticleSnapshot)> {
    entities
        .iter()
        .filter_map(|e| e.particles.map(|p| (e.id, entity_label(e), p)))
        .collect()
}

impl EditorPanel for ParticlePanel {
    fn id(&self) -> &str {
        "particles"
    }

    fn title(&self) -> String {
        "Particles".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        let entities = ctx.entities_snapshot;
        let insp: &mut InspectorState = &mut *ctx.insp;

        ui.horizontal(|ui| {
            ui.label(egui::RichText::new("Preview").strong());
            let toggle = if insp.particle_preview.paused {
                "Play"
            } else {
                "Pause"
            };
            if ui.button(toggle).clicked() {
                insp.particle_preview.paused = !insp.particle_preview.paused;
            }
            ui.add(
                egui::Slider::new(&mut insp.particle_preview.speed, MIN_SPEED..=MAX_SPEED)
                    .fixed_decimals(2)
                    .suffix("x"),
            );
            if ui.button("Restart All").clicked() {
                insp.cmd_queue
                    .push(InspectorCmd::ParticleRestart { id: None });
            }
        });
        // Said here rather than discovered by pressing Pause and seeing
        // nothing happen: the controls are a *preview*, and a running game
        // ignores them by design (see `ParticlePreview`).
        if !(insp.editor_mode && insp.play_state == EditorPlayState::Stopped) {
            ui.label(
                egui::RichText::new("Preview controls apply while the game is stopped.")
                    .weak()
                    .small(),
            );
        }
        ui.separator();

        let rows = emitters(entities);
        if rows.is_empty() {
            // The ordinary state for a scene without effects, not a failure.
            ui.label("No particle emitters in the scene.");
            ui.label(
                egui::RichText::new("Add a ParticleEmitter component to an entity.")
                    .weak()
                    .small(),
            );
            return;
        }
        let alive: usize = rows.iter().map(|(_, _, p)| p.alive).sum();
        ui.label(
            egui::RichText::new(format!(
                "{alive} alive across {} emitter{}",
                rows.len(),
                if rows.len() == 1 { "" } else { "s" }
            ))
            .weak()
            .small(),
        );

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (id, label, particles) in rows {
                ui.horizontal(|ui| {
                    let selected = insp.selected_id == Some(id);
                    if ui.selectable_label(selected, &label).clicked() {
                        insp.selected_id = Some(id);
                        insp.sync_selection();
                    }
                    ui.label(format!("{} alive", particles.alive));
                    if particles.rate > 0.0 {
                        ui.label(
                            egui::RichText::new(format!("{:.1}/s", particles.rate))
                                .weak()
                                .small(),
                        );
                    } else {
                        ui.label(
                            egui::RichText::new(format!("burst {}", particles.burst_count))
                                .weak()
                                .small(),
                        );
                    }
                    if !particles.enabled {
                        ui.label(egui::RichText::new("off").weak().small());
                    }
                    if ui.button("Burst").clicked() {
                        insp.cmd_queue.push(InspectorCmd::ParticleBurst { id });
                    }
                    if ui.button("Restart").clicked() {
                        insp.cmd_queue
                            .push(InspectorCmd::ParticleRestart { id: Some(id) });
                    }
                });
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless frame harness: the panel, the inspector state it edits,
    /// and the entity snapshot it reads. Mirrors `inspector.rs`'s
    /// `PickerHarness` in shape and in its one rule: click positions come
    /// from where a label rendered, never from a hand-measured pixel.
    struct Harness {
        egui_ctx: egui::Context,
        insp: InspectorState,
        entities: Vec<InspectorEntityInfo>,
        panel: ParticlePanel,
    }

    impl Harness {
        fn new(entities: Vec<InspectorEntityInfo>) -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            let mut insp = InspectorState::editor();
            insp.entities = entities.clone();
            Self {
                egui_ctx,
                insp,
                entities,
                panel: ParticlePanel,
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(600.0, 400.0));
            self.egui_ctx.run(
                egui::RawInput {
                    screen_rect: Some(screen_rect),
                    events,
                    ..Default::default()
                },
                |egui_ctx| {
                    egui::CentralPanel::default().show(egui_ctx, |ui| {
                        let mut ctx = EditorPanelContext {
                            insp: &mut self.insp,
                            entities_snapshot: &self.entities,
                            cursor_pos: (0.0, 0.0),
                            type_registry: None,
                        };
                        self.panel.ui(ui, &mut ctx);
                    });
                },
            )
        }

        fn draw(&mut self) -> egui::FullOutput {
            self.frame(Vec::new())
        }

        /// Draws once so the widgets exist, then clicks where `label`
        /// rendered on that frame (egui hit-tests the previous frame's
        /// rects). Returns the frame the click was delivered to.
        fn click(&mut self, label: &str) -> egui::FullOutput {
            let frame = self.draw();
            let pos = text_positions(&frame.shapes)
                .into_iter()
                .find(|(text, _)| text == label)
                .map(|(_, pos)| pos)
                .unwrap_or_else(|| panic!("{label:?} must render before it can be clicked"));
            self.frame(vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: egui::Modifiers::default(),
                },
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: egui::Modifiers::default(),
                },
            ])
        }
    }

    /// Every text egui drew, with where it was drawn. `Shape::Vec` nests
    /// and is walked; the clip rect is ignored on purpose (see
    /// `inspector.rs`'s helper of the same purpose).
    fn text_positions(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Pos2)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos)),
                egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn texts(frame: &egui::FullOutput) -> Vec<String> {
        text_positions(&frame.shapes)
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    fn emitter(id: u64, name: &str, alive: usize, rate: f32) -> InspectorEntityInfo {
        InspectorEntityInfo {
            id,
            name: Some(name.to_string()),
            particles: Some(ParticleSnapshot {
                alive,
                rate,
                burst_count: 24,
                enabled: true,
            }),
            ..Default::default()
        }
    }

    fn plain(id: u64, name: &str) -> InspectorEntityInfo {
        InspectorEntityInfo {
            id,
            name: Some(name.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn a_scene_without_emitters_says_so_rather_than_drawing_nothing() {
        let mut h = Harness::new(vec![plain(1, "Camera"), plain(2, "Sun")]);
        let drawn = texts(&h.draw());
        assert!(
            drawn
                .iter()
                .any(|t| t == "No particle emitters in the scene."),
            "got {drawn:?}"
        );
        assert!(
            !drawn.iter().any(|t| t.contains("Camera")),
            "entities without an emitter are not rows: {drawn:?}"
        );
    }

    /// Only the entities with an emitter are rows, each under the same
    /// `[id] name` the Hierarchy shows, with this frame's alive count.
    #[test]
    fn each_emitter_is_a_row_with_its_alive_count_and_nothing_else_is() {
        let mut h = Harness::new(vec![
            plain(1, "Camera"),
            emitter(4, "Sparks", 37, 0.0),
            emitter(9, "Smoke", 120, 15.0),
        ]);
        let drawn = texts(&h.draw());
        for expected in [
            "[4] Sparks",
            "37 alive",
            "burst 24",
            "[9] Smoke",
            "120 alive",
            "15.0/s",
        ] {
            assert!(
                drawn.iter().any(|t| t == expected),
                "{expected:?} missing from {drawn:?}"
            );
        }
        assert!(
            drawn.iter().any(|t| t == "157 alive across 2 emitters"),
            "the total must be summed over the rows: {drawn:?}"
        );
        assert!(!drawn.iter().any(|t| t.contains("Camera")), "{drawn:?}");
    }

    #[test]
    fn burst_and_restart_queue_commands_for_that_emitter() {
        let mut h = Harness::new(vec![emitter(4, "Sparks", 0, 0.0)]);
        h.click("Burst");
        h.click("Restart");
        assert_eq!(h.insp.cmd_queue.len(), 2, "one command per click");
        assert!(matches!(
            h.insp.cmd_queue[0],
            InspectorCmd::ParticleBurst { id: 4 }
        ));
        assert!(matches!(
            h.insp.cmd_queue[1],
            InspectorCmd::ParticleRestart { id: Some(4) }
        ));
    }

    /// The preview controls: Pause flips the flag and relabels itself Play,
    /// and Restart All asks for every emitter (`None`), not the selected one.
    #[test]
    fn pause_toggles_the_preview_and_restart_all_names_no_emitter() {
        let mut h = Harness::new(vec![emitter(4, "Sparks", 0, 0.0)]);
        assert!(
            !h.insp.particle_preview.paused,
            "premise: not paused to begin with"
        );
        h.click("Pause");
        assert!(
            h.insp.particle_preview.paused,
            "Pause must pause the preview"
        );
        // The frame that took the click had already drawn the button as
        // "Pause"; the relabel shows on the next one.
        assert!(
            texts(&h.draw()).iter().any(|t| t == "Play"),
            "and the button must now offer Play"
        );
        h.click("Play");
        assert!(!h.insp.particle_preview.paused, "Play must resume it");

        h.click("Restart All");
        assert!(matches!(
            h.insp.cmd_queue.as_slice(),
            [InspectorCmd::ParticleRestart { id: None }]
        ));
    }

    #[test]
    fn clicking_an_emitter_selects_its_entity() {
        let mut h = Harness::new(vec![
            emitter(4, "Sparks", 0, 0.0),
            emitter(9, "Smoke", 0, 1.0),
        ]);
        h.click("[9] Smoke");
        assert_eq!(h.insp.selected_id, Some(9));
        assert!(
            h.insp.cmd_queue.is_empty(),
            "selection is state, not a command"
        );
    }

    /// The note that the controls only apply to a stopped game: shown while
    /// playing, absent while stopped, so a paused preview that changes
    /// nothing in a running game is explained rather than mysterious.
    #[test]
    fn the_preview_note_appears_only_while_the_game_runs() {
        let mut h = Harness::new(vec![emitter(4, "Sparks", 0, 0.0)]);
        let note = "Preview controls apply while the game is stopped.";
        assert!(
            !texts(&h.draw()).iter().any(|t| t == note),
            "stopped: no note"
        );
        h.insp.play_state = EditorPlayState::Playing;
        assert!(
            texts(&h.draw()).iter().any(|t| t == note),
            "playing: the note"
        );
    }
}
