//! References panel: the project's asset dependency graph, centred on the
//! selected asset.
//!
//! Unreal's Reference Viewer is the model: the asset in the middle, what
//! references it in a column to the left, what it references in a column to
//! the right, lines between, and clicking any node re-centres on it. Godot
//! has no graph but its Dependency Editor's three lists -- owners, dependencies
//! and orphan resources -- are the same facts, and the orphans (assets nothing
//! reaches, which a packaged build leaves out) are listed under the graph
//! here together with references that name a file that does not exist.
//!
//! One level each side rather than a whole-project graph, as Unreal's default
//! depth is: a project's full graph is a hairball, and a question about one
//! asset is answered by its neighbours. Re-centring by click is how an author
//! walks further.
//!
//! Draws `InspectorState::asset_graph`, which `bsengine-editor` fills from the
//! packager's walk when `asset_graph_refresh` is set; this crate cannot walk
//! the project itself (it sits below `bsengine-asset`). Selecting a node goes
//! through `InspectorState::select_asset`, so the Inspector follows the panel
//! the way it follows the Asset Browser.

use bsengine_core::{AssetGraphSnapshot, EditorPanel, EditorPanelContext, InspectorState};

/// The referrer the manifest is recorded as: the one node that is not an
/// asset, so it cannot be selected, only shown.
const MANIFEST: &str = "project.toml";

/// The lines between nodes. Its own colour so a test can count the graph's
/// edges among the frame's line segments without also counting separators.
const EDGE: egui::Color32 = egui::Color32::from_rgb(0x8a, 0x90, 0x9c);

/// Draws the selected asset's neighbourhood in the dependency graph.
#[derive(Default)]
pub struct ReferencesPanel;

impl EditorPanel for ReferencesPanel {
    fn id(&self) -> &str {
        "references"
    }

    fn title(&self) -> String {
        "References".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, ctx: &mut EditorPanelContext) {
        let insp: &mut InspectorState = &mut *ctx.insp;

        ui.horizontal(|ui| {
            if ui.button("Refresh").clicked() {
                insp.asset_graph_refresh = true;
            }
            ui.label(
                egui::RichText::new("The packager's walk from project.toml's entry scene.")
                    .weak()
                    .small(),
            );
        });
        ui.separator();

        let Some(graph) = insp.asset_graph.clone() else {
            // First frame: ask for the walk and say so. The editor fills the
            // snapshot next frame and this branch is never taken again
            // until something drops it.
            insp.asset_graph_refresh = true;
            ui.label("Walking the project...");
            return;
        };
        if let Some(error) = &graph.error {
            ui.colored_label(egui::Color32::from_rgb(230, 90, 90), error);
            return;
        }

        match insp.selected_asset.clone() {
            Some(focus) => draw_neighbourhood(ui, insp, &graph, &focus),
            None => {
                ui.label("Select an asset in the Asset Browser to see its references.");
            }
        }

        ui.separator();
        draw_lists(ui, insp, &graph);
    }
}

/// A node: a clickable label for an asset, a plain one for the manifest.
/// Returns the rect it was drawn in, for the edge lines.
fn node(ui: &mut egui::Ui, insp: &mut InspectorState, path: &str, focused: bool) -> egui::Rect {
    if path == MANIFEST {
        return ui
            .label(egui::RichText::new(path).color(crate::theme::TEXT_MUTED))
            .rect;
    }
    let response = ui.selectable_label(focused, path);
    if response.clicked() {
        insp.select_asset(path);
    }
    response.rect
}

/// The three columns and the lines between them.
fn draw_neighbourhood(
    ui: &mut egui::Ui,
    insp: &mut InspectorState,
    graph: &AssetGraphSnapshot,
    focus: &str,
) {
    let referencers: Vec<String> = graph
        .referencers_of(focus)
        .into_iter()
        .map(str::to_string)
        .collect();
    let dependencies: Vec<String> = graph
        .dependencies_of(focus)
        .into_iter()
        .map(str::to_string)
        .collect();
    let reached = graph.edges.iter().any(|(_, to)| to == focus);
    if !reached {
        ui.colored_label(
            egui::Color32::from_rgb(230, 180, 60),
            "Not reached from the entry scene: a packaged build leaves it out.",
        );
    }

    let mut left = Vec::with_capacity(referencers.len());
    let mut right = Vec::with_capacity(dependencies.len());
    let mut centre = egui::Rect::NOTHING;
    ui.columns(3, |cols| {
        cols[0].label(egui::RichText::new(format!("Used by ({})", referencers.len())).strong());
        for path in &referencers {
            left.push(node(&mut cols[0], insp, path, false));
        }
        cols[1].label(egui::RichText::new("Asset").strong());
        centre = node(&mut cols[1], insp, focus, true);
        cols[2].label(egui::RichText::new(format!("Uses ({})", dependencies.len())).strong());
        for path in &dependencies {
            right.push(node(&mut cols[2], insp, path, false));
        }
    });

    // Painted after the columns so the lines lie over the column
    // backgrounds; each runs from a node's near edge to the centre node's.
    let stroke = egui::Stroke::new(1.0_f32, EDGE);
    let painter = ui.painter();
    for rect in &left {
        painter.line_segment([rect.right_center(), centre.left_center()], stroke);
    }
    for rect in &right {
        painter.line_segment([centre.right_center(), rect.left_center()], stroke);
    }
}

/// The project-wide lists under the graph: what nothing reaches, and what
/// names nothing.
fn draw_lists(ui: &mut egui::Ui, insp: &mut InspectorState, graph: &AssetGraphSnapshot) {
    egui::CollapsingHeader::new(format!("Unreferenced ({})", graph.unreferenced.len()))
        .default_open(true)
        .show(ui, |ui| {
            if graph.unreferenced.is_empty() {
                ui.label(
                    egui::RichText::new("Every asset is reached.")
                        .weak()
                        .small(),
                );
            }
            for path in &graph.unreferenced {
                let focused = insp.selected_asset.as_deref() == Some(path.as_str());
                node(ui, insp, path, focused);
            }
        });
    egui::CollapsingHeader::new(format!("Missing ({})", graph.missing.len()))
        .default_open(true)
        .show(ui, |ui| {
            if graph.missing.is_empty() {
                ui.label(
                    egui::RichText::new("Every reference resolves.")
                        .weak()
                        .small(),
                );
            }
            for (referrer, path) in &graph.missing {
                ui.horizontal(|ui| {
                    node(ui, insp, referrer, false);
                    ui.label(egui::RichText::new("names").weak().small());
                    ui.colored_label(egui::Color32::from_rgb(230, 90, 90), path);
                });
            }
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A headless frame harness, the shape `particles.rs` uses: the panel
    /// and the inspector state it reads and edits. Click positions come
    /// from where a label rendered, never from a hand-measured pixel.
    struct Harness {
        egui_ctx: egui::Context,
        insp: InspectorState,
        panel: ReferencesPanel,
    }

    impl Harness {
        fn new() -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            Self {
                egui_ctx,
                insp: InspectorState::editor(),
                panel: ReferencesPanel,
            }
        }

        fn frame(&mut self, events: Vec<egui::Event>) -> egui::FullOutput {
            let screen_rect = egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(900.0, 500.0));
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
                            entities_snapshot: &[],
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
        /// rendered on that frame.
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

    fn walk(shape: &egui::Shape, texts: &mut Vec<(String, egui::Pos2)>, edges: &mut usize) {
        match shape {
            egui::Shape::Text(t) => texts.push((t.galley.text().to_string(), t.pos)),
            egui::Shape::LineSegment { stroke, .. } if matches!(stroke.color, egui::epaint::ColorMode::Solid(c) if c == EDGE) => {
                *edges += 1
            }
            egui::Shape::Vec(inner) => inner.iter().for_each(|s| walk(s, texts, edges)),
            _ => {}
        }
    }

    fn text_positions(shapes: &[egui::epaint::ClippedShape]) -> Vec<(String, egui::Pos2)> {
        let mut texts = Vec::new();
        let mut edges = 0;
        for clipped in shapes {
            walk(&clipped.shape, &mut texts, &mut edges);
        }
        texts
    }

    fn texts(frame: &egui::FullOutput) -> Vec<String> {
        text_positions(&frame.shapes)
            .into_iter()
            .map(|(t, _)| t)
            .collect()
    }

    /// How many graph edges the frame drew -- the lines in the graph's own
    /// colour, so separators and widget outlines do not count.
    fn edge_lines(frame: &egui::FullOutput) -> usize {
        let mut texts = Vec::new();
        let mut edges = 0;
        for clipped in &frame.shapes {
            walk(&clipped.shape, &mut texts, &mut edges);
        }
        edges
    }

    /// A model two scenes name, one of them reached through a script; a
    /// texture nothing names; a reference to a file that is not there.
    fn graph() -> AssetGraphSnapshot {
        let e = |a: &str, b: &str| (a.to_string(), b.to_string());
        AssetGraphSnapshot {
            edges: vec![
                e("assets/scenes/level2.ron", "assets/models/hero.glb"),
                e("assets/scenes/main.ron", "assets/models/hero.glb"),
                e("assets/scenes/main.ron", "assets/scripts/hero.js"),
                e("assets/scripts/hero.js", "assets/scenes/level2.ron"),
                e(MANIFEST, "assets/scenes/main.ron"),
            ],
            unreferenced: vec!["assets/textures/unused.png".to_string()],
            missing: vec![e("assets/scenes/level2.ron", "assets/textures/gone.png")],
            error: None,
        }
    }

    /// Without a graph the panel asks for one and says it is waiting; with
    /// one and no selection it says what to do. Neither draws edges.
    #[test]
    fn asks_for_the_walk_on_its_first_frame_and_waits() {
        let mut h = Harness::new();
        assert!(!h.insp.asset_graph_refresh, "premise: nothing asked yet");
        let frame = h.draw();
        assert!(
            h.insp.asset_graph_refresh,
            "the first frame requests the walk"
        );
        assert!(texts(&frame).iter().any(|t| t == "Walking the project..."));
        assert_eq!(edge_lines(&frame), 0);

        h.insp.asset_graph_refresh = false;
        h.insp.asset_graph = Some(graph());
        let frame = h.draw();
        assert!(
            !h.insp.asset_graph_refresh,
            "with a graph in hand nothing is requested"
        );
        assert!(texts(&frame)
            .iter()
            .any(|t| t.starts_with("Select an asset in the Asset Browser")));
        assert_eq!(edge_lines(&frame), 0);
    }

    /// The neighbourhood of the selected asset: its referencers on the
    /// left, its dependencies on the right, one line per neighbour, and
    /// nothing from elsewhere in the graph.
    #[test]
    fn draws_the_selected_assets_referencers_and_dependencies_with_a_line_each() {
        let mut h = Harness::new();
        h.insp.asset_graph = Some(graph());
        h.insp.select_asset("assets/scripts/hero.js");
        let frame = h.draw();
        let drawn = texts(&frame);
        for expected in [
            "Used by (1)",
            "assets/scenes/main.ron",
            "Asset",
            "assets/scripts/hero.js",
            "Uses (1)",
            "assets/scenes/level2.ron",
        ] {
            assert!(
                drawn.iter().any(|t| t == expected),
                "{expected:?} missing from {drawn:?}"
            );
        }
        assert!(
            !drawn.iter().any(|t| t == "assets/models/hero.glb"),
            "the model is two steps away and not in the neighbourhood: {drawn:?}"
        );
        assert_eq!(
            edge_lines(&frame),
            2,
            "one line to the referencer, one to the dependency"
        );

        h.insp.select_asset("assets/models/hero.glb");
        let frame = h.draw();
        let drawn = texts(&frame);
        assert!(drawn.iter().any(|t| t == "Used by (2)"), "{drawn:?}");
        assert!(drawn.iter().any(|t| t == "Uses (0)"), "{drawn:?}");
        assert_eq!(edge_lines(&frame), 2, "two referencers, no dependencies");
        assert!(
            !drawn.iter().any(|t| t.starts_with("Not reached")),
            "a reached asset carries no warning"
        );
    }

    /// An asset nothing reaches says so above its (empty) neighbourhood.
    #[test]
    fn an_unreached_asset_is_flagged() {
        let mut h = Harness::new();
        h.insp.asset_graph = Some(graph());
        h.insp.select_asset("assets/textures/unused.png");
        let drawn = texts(&h.draw());
        assert!(
            drawn
                .iter()
                .any(|t| t.starts_with("Not reached from the entry scene")),
            "{drawn:?}"
        );
        assert!(drawn.iter().any(|t| t == "Used by (0)"));
    }

    /// Clicking a neighbour re-centres on it, through the same selection
    /// the Asset Browser uses, so the Inspector follows; the manifest is
    /// not an asset and clicking it changes nothing.
    #[test]
    fn clicking_a_node_selects_that_asset_except_the_manifest() {
        let mut h = Harness::new();
        h.insp.asset_graph = Some(graph());
        h.insp.select_asset("assets/scripts/hero.js");
        h.click("assets/scenes/level2.ron");
        assert_eq!(
            h.insp.selected_asset.as_deref(),
            Some("assets/scenes/level2.ron"),
            "the clicked dependency is now the centre"
        );
        assert!(
            h.insp.asset_references.is_none(),
            "and the Inspector's per-asset snapshot is dropped for re-reading"
        );

        h.insp.select_asset("assets/scenes/main.ron");
        let drawn = texts(&h.draw());
        assert!(drawn.iter().any(|t| t == MANIFEST), "{drawn:?}");
        h.click(MANIFEST);
        assert_eq!(
            h.insp.selected_asset.as_deref(),
            Some("assets/scenes/main.ron"),
            "the manifest cannot be selected"
        );
    }

    /// Refresh asks for a new walk without dropping the graph on screen.
    #[test]
    fn refresh_requests_a_walk_and_keeps_the_current_graph() {
        let mut h = Harness::new();
        h.insp.asset_graph = Some(graph());
        h.click("Refresh");
        assert!(h.insp.asset_graph_refresh);
        assert!(
            h.insp.asset_graph.is_some(),
            "the old graph stays until replaced"
        );
    }

    /// The project-wide lists: the unreached assets, clickable, and every
    /// reference that names nothing, with who made it.
    #[test]
    fn lists_unreferenced_assets_and_missing_references() {
        let mut h = Harness::new();
        h.insp.asset_graph = Some(graph());
        let drawn = texts(&h.draw());
        for expected in [
            "Unreferenced (1)",
            "assets/textures/unused.png",
            "Missing (1)",
            "assets/textures/gone.png",
        ] {
            assert!(
                drawn.iter().any(|t| t == expected),
                "{expected:?} missing from {drawn:?}"
            );
        }
        h.click("assets/textures/unused.png");
        assert_eq!(
            h.insp.selected_asset.as_deref(),
            Some("assets/textures/unused.png"),
            "an unreferenced asset can be selected from the list"
        );
    }

    /// A walk that could not run is shown as its error, and nothing else.
    #[test]
    fn a_failed_walk_shows_its_error_instead_of_a_graph() {
        let mut h = Harness::new();
        h.insp.asset_graph = Some(AssetGraphSnapshot {
            error: Some("no project.toml".to_string()),
            ..Default::default()
        });
        h.insp.select_asset("assets/models/hero.glb");
        let drawn = texts(&h.draw());
        assert!(drawn.iter().any(|t| t == "no project.toml"), "{drawn:?}");
        assert!(!drawn.iter().any(|t| t.starts_with("Unreferenced")));
    }
}
