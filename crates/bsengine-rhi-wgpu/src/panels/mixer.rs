//! Mixer panel: the bus tree, with a volume fader on each bus.
//!
//! The buses themselves shipped in #1837-#1840 — layout, DSP chains, occlusion
//! and runtime parameters — but nothing in the editor showed them, so the only
//! way to hear what a mix change did was to edit `buses.ron` and restart. Unity,
//! Unreal and Godot all put a mixer in the editor for the same reason: a mix is
//! tuned by ear, and that means adjusting it while it plays.
//!
//! This panel reads a snapshot and writes requests; it never touches the mixer
//! directly. See [`bsengine_core::mixer_state`] for why the traffic runs that
//! way in both directions.

use bsengine_core::{EditorPanel, EditorPanelContext, MixerBus, MixerShared};

/// Decibel range of a fader.
///
/// -60 reads as silence and +6 leaves a little headroom to push a bus, which is
/// the range Unity's mixer and Godot's audio buses both default to. A fader
/// that ran to -inf would spend most of its travel in values nobody can hear
/// apart.
const MIN_DB: f32 = -60.0;
/// Top of the fader range, in decibels.
const MAX_DB: f32 = 6.0;

/// Shows every audio bus as a tree with a volume fader.
pub struct MixerPanel {
    shared: MixerShared,
}

impl MixerPanel {
    /// Wraps the shared mixer handle published by `AudioPlugin`.
    pub fn new(shared: MixerShared) -> Self {
        Self { shared }
    }

    /// Bus names in tree order: each parent immediately followed by its
    /// children, with depth for indenting.
    ///
    /// Separate from drawing, and takes a plain slice, so the ordering can be
    /// tested without an egui context — the part that can actually be wrong is
    /// the traversal, not the widgets.
    pub fn tree_order(buses: &[MixerBus]) -> Vec<(usize, usize)> {
        let mut out = Vec::new();
        // Roots first, in published order, then each one's subtree.
        let roots: Vec<usize> = buses
            .iter()
            .enumerate()
            .filter(|(_, b)| {
                b.parent.is_none() || !buses.iter().any(|p| Some(&p.name) == b.parent.as_ref())
            })
            .map(|(i, _)| i)
            .collect();
        for root in roots {
            Self::walk(buses, root, 0, &mut out);
        }
        // A cycle in the bus file would leave members unvisited; showing them
        // flat beats dropping them from a panel whose whole job is to show
        // what exists.
        for i in 0..buses.len() {
            if !out.iter().any(|(j, _)| *j == i) {
                out.push((i, 0));
            }
        }
        out
    }

    fn walk(buses: &[MixerBus], index: usize, depth: usize, out: &mut Vec<(usize, usize)>) {
        if out.iter().any(|(i, _)| *i == index) || depth > buses.len() {
            return;
        }
        out.push((index, depth));
        for (child, bus) in buses.iter().enumerate() {
            if bus.parent.as_deref() == Some(buses[index].name.as_str()) {
                Self::walk(buses, child, depth + 1, out);
            }
        }
    }
}

impl EditorPanel for MixerPanel {
    fn id(&self) -> &str {
        "mixer"
    }

    fn title(&self) -> String {
        "Mixer".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        let buses = self.shared.buses();
        if buses.is_empty() {
            // The ordinary state for a project with no `assets/audio/buses.ron`,
            // not a failure — said plainly rather than shown as an empty grid.
            ui.label("No audio buses.");
            ui.label(
                egui::RichText::new("Add assets/audio/buses.ron to define a mix.")
                    .weak()
                    .small(),
            );
            return;
        }

        egui::ScrollArea::vertical().show(ui, |ui| {
            for (index, depth) in Self::tree_order(&buses) {
                let bus = &buses[index];
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 14.0);
                    ui.label(egui::RichText::new(&bus.name).strong());
                    if bus.effects > 0 {
                        ui.label(
                            egui::RichText::new(format!("{} fx", bus.effects))
                                .weak()
                                .small(),
                        );
                    }
                });
                ui.horizontal(|ui| {
                    ui.add_space(depth as f32 * 14.0);
                    let mut db = bus.volume_db;
                    let slider = ui.add(
                        egui::Slider::new(&mut db, MIN_DB..=MAX_DB)
                            .suffix(" dB")
                            .fixed_decimals(1),
                    );
                    // Requested only when the value actually moved. Pushing
                    // every frame would queue a request per frame for a bus
                    // nobody is touching, and the audio side would spend its
                    // time re-applying values that never changed.
                    if slider.changed() {
                        self.shared.request_volume(&bus.name, db);
                    }
                });
            }
        });
    }
}

/// Puts the mixer panel in the editor's panel registry.
///
/// A `Startup` system rather than a plugin's `build`, because `EditorPlugin`
/// *inserts* the registry (replacing whatever was there) while it builds, so a
/// panel registered during any other plugin's `build` may or may not survive
/// depending on plugin order. By `Startup` every plugin has built and the
/// registry is final.
///
/// It lives beside the panel rather than in `dock.rs` with the other built-ins
/// for two reasons. `ensure_builtin_panels` is called from inside
/// `render_frame`, which sits at Bevy 0.14's 16-parameter ceiling with a full
/// `ParamSet` — there is no room to thread a `MixerShared` through it. And
/// keeping the wiring here lets the panel's own tests drive the panel this
/// function registered, which is the only way to observe that the panel is
/// bound to the *live* handle rather than a fresh empty one.
///
/// Does nothing when either resource is absent: an app with no editor has no
/// registry, and one with no audio has no mixer to show.
pub fn register_mixer_panel(
    registry: Option<bevy_ecs::prelude::Res<bsengine_core::EditorPanelRegistry>>,
    mixer: Option<bevy_ecs::prelude::Res<MixerShared>>,
) {
    let (Some(registry), Some(mixer)) = (registry, mixer) else {
        return;
    };
    let Ok(mut panels) = registry.0.lock() else {
        return;
    };
    panels
        .entry("mixer".to_string())
        .or_insert_with(|| Box::new(MixerPanel::new(mixer.clone())) as Box<dyn EditorPanel>);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bus(name: &str, parent: Option<&str>) -> MixerBus {
        MixerBus {
            name: name.into(),
            parent: parent.map(str::to_string),
            volume_db: 0.0,
            effects: 0,
        }
    }

    #[test]
    fn a_bus_appears_under_its_parent_and_indented() {
        // Published deliberately out of order: the panel's job is to recover
        // the tree, and a list that happened to arrive parent-first would let a
        // traversal that ignores `parent` look correct.
        let buses = vec![
            bus("ui", Some("sfx")),
            bus("master", None),
            bus("sfx", Some("master")),
        ];
        let order = MixerPanel::tree_order(&buses);
        let named: Vec<(&str, usize)> = order
            .iter()
            .map(|&(i, d)| (buses[i].name.as_str(), d))
            .collect();
        assert_eq!(
            named,
            vec![("master", 0), ("sfx", 1), ("ui", 2)],
            "each bus must follow its parent, one level deeper"
        );
    }

    #[test]
    fn several_roots_are_all_shown() {
        let buses = vec![bus("master", None), bus("voice", None)];
        assert_eq!(MixerPanel::tree_order(&buses).len(), 2);
    }

    #[test]
    fn a_bus_naming_a_missing_parent_is_still_shown_as_a_root() {
        // A typo in `buses.ron` must not make a bus vanish from the one view
        // whose purpose is to show what exists.
        let buses = vec![bus("master", None), bus("orphan", Some("nope"))];
        let order = MixerPanel::tree_order(&buses);
        assert_eq!(order.len(), 2, "{order:?}");
        assert!(
            order
                .iter()
                .any(|&(i, d)| buses[i].name == "orphan" && d == 0),
            "the orphan should appear at the root rather than disappear"
        );
    }

    #[test]
    fn a_cycle_in_the_bus_file_still_lists_every_bus_once() {
        // Two buses naming each other: unguarded this recurses forever, and
        // dropping them would hide the very mistake the panel should reveal.
        let buses = vec![bus("a", Some("b")), bus("b", Some("a"))];
        let order = MixerPanel::tree_order(&buses);
        assert_eq!(order.len(), 2, "every bus exactly once: {order:?}");
        let mut seen: Vec<usize> = order.iter().map(|&(i, _)| i).collect();
        seen.sort_unstable();
        seen.dedup();
        assert_eq!(seen.len(), 2, "no bus listed twice");
    }

    /// Every string the panel drew, collected over two frames.
    ///
    /// Two frames because egui lays a widget out against the sizes it measured
    /// on the previous one, so content can be clipped away on the frame a
    /// context first sees it.
    ///
    /// Takes `&mut dyn EditorPanel` so the registration test can drive the
    /// panel it pulled back out of the registry, where the concrete type is
    /// gone.
    fn drawn_text(panel: &mut dyn EditorPanel) -> Vec<String> {
        let ctx = egui::Context::default();
        let mut insp = bsengine_core::InspectorState::default();
        let mut texts = Vec::new();
        for _ in 0..2 {
            texts.clear();
            let output = ctx.run(
                egui::RawInput {
                    screen_rect: Some(egui::Rect::from_min_size(
                        egui::Pos2::ZERO,
                        egui::vec2(400.0, 400.0),
                    )),
                    ..Default::default()
                },
                |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        let mut panel_ctx = EditorPanelContext {
                            insp: &mut insp,
                            entities_snapshot: &[],
                            cursor_pos: (0.0, 0.0),
                            type_registry: None,
                        };
                        panel.ui(ui, &mut panel_ctx);
                    });
                },
            );
            for clipped in &output.shapes {
                collect_text(&clipped.shape, &mut texts);
            }
        }
        texts
    }

    fn collect_text(shape: &egui::Shape, out: &mut Vec<String>) {
        match shape {
            egui::Shape::Text(text) => out.push(text.galley.text().to_string()),
            egui::Shape::Vec(shapes) => {
                for shape in shapes {
                    collect_text(shape, out);
                }
            }
            _ => {}
        }
    }

    #[test]
    fn an_empty_mixer_says_so_rather_than_drawing_nothing() {
        let mut panel = MixerPanel::new(MixerShared::default());
        let text = drawn_text(&mut panel);
        assert!(
            text.iter().any(|t| t.contains("No audio buses")),
            "an empty mixer should say why it is empty: {text:?}"
        );
    }

    #[test]
    fn every_bus_is_drawn_with_its_own_volume() {
        // Different volumes deliberately: were they equal, a panel that drew
        // the first bus's value on every fader would look correct.
        let shared = MixerShared::default();
        shared.publish(vec![
            MixerBus {
                name: "sfx".into(),
                parent: None,
                volume_db: -6.0,
                effects: 0,
            },
            MixerBus {
                name: "music".into(),
                parent: Some("sfx".into()),
                volume_db: -12.0,
                effects: 0,
            },
        ]);
        let mut panel = MixerPanel::new(shared);
        let text = drawn_text(&mut panel);
        for expected in ["sfx", "music", "-6.0", "-12.0"] {
            assert!(
                text.iter().any(|t| t.contains(expected)),
                "expected {expected:?} among the drawn text: {text:?}"
            );
        }
    }

    #[test]
    fn an_effect_count_is_drawn_only_for_a_bus_that_has_effects() {
        let shared = MixerShared::default();
        shared.publish(vec![
            MixerBus {
                name: "sfx".into(),
                parent: None,
                volume_db: 0.0,
                effects: 2,
            },
            MixerBus {
                name: "ui".into(),
                parent: None,
                volume_db: 0.0,
                effects: 0,
            },
        ]);
        let mut panel = MixerPanel::new(shared);
        let text = drawn_text(&mut panel);
        assert!(
            text.iter().any(|t| t.contains("2 fx")),
            "a bus with a DSP chain should say how long it is: {text:?}"
        );
        assert!(
            !text.iter().any(|t| t.contains("0 fx")),
            "a bus with no effects should say nothing rather than \"0 fx\": {text:?}"
        );
    }

    #[test]
    fn the_registered_panel_reads_the_live_mixer() {
        // Published *after* registration on purpose. A panel handed a snapshot,
        // or one that built a fresh handle of its own, would draw nothing here
        // -- and no count-of-registered-panels assertion would notice, which is
        // exactly how a mixer that shows a permanently empty tree ships green.
        let mut app = bsengine_app::new_app();
        app.insert_resource(bsengine_core::EditorPanelRegistry::default());
        app.init_resource::<MixerShared>();
        app.add_systems(bevy_app::Startup, register_mixer_panel);
        app.update();

        app.world()
            .resource::<MixerShared>()
            .publish(vec![MixerBus {
                name: "late".into(),
                parent: None,
                volume_db: -3.0,
                effects: 0,
            }]);

        let registry = app
            .world()
            .resource::<bsengine_core::EditorPanelRegistry>()
            .0
            .clone();
        let mut panels = registry.lock().unwrap();
        let panel = panels
            .get_mut("mixer")
            .expect("the startup system should have registered a mixer panel");
        let text = drawn_text(panel.as_mut());
        assert!(
            text.iter().any(|t| t.contains("late")),
            "the panel should show what was published after it was built: {text:?}"
        );
    }
}
