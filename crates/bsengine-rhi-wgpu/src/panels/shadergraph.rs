//! Shader graph node editor: build a `bsengine_shadergraph::ShaderGraph`
//! visually and compile it to WGSL. See
//! `docs/superpowers/specs/2026-09-01-shadergraph-ui-design.md`.
//!
//! Nodes are drawn with `egui::Painter` into the panel's own coordinate
//! space rather than each being an `egui::Area`, so a future pan/zoom is a
//! single coordinate transform instead of a change to every hit-test. Pan and
//! zoom themselves are deliberately out of scope for item 50.

use bsengine_core::{EditorPanel, EditorPanelContext};
use bsengine_shadergraph::{GraphError, NodeKind, ShaderGraph, ValueType};
use std::collections::HashMap;

/// The name of the single output port every node kind has.
const OUTPUT_PORT: &str = "out";

/// Width of a node box, in panel-local pixels.
const NODE_WIDTH: f32 = 148.0;
/// Height of a node's title bar.
const HEADER_HEIGHT: f32 = 22.0;
/// Vertical pitch of one port row inside a node body.
const PORT_ROW_HEIGHT: f32 = 18.0;
/// Radius of the circle drawn for a port.
const PORT_RADIUS: f32 = 5.0;

/// Editor panel for authoring shader graphs.
///
/// Holds the graph being edited and, as a side effect of drawing it, the
/// screen position of every port it laid out. Task 3 turns those positions
/// into drag targets; this version only renders.
#[derive(Default)]
pub struct ShaderGraphPanel {
    /// The graph being edited.
    pub graph: ShaderGraph,
    /// Where each port was drawn this frame, in **screen** coordinates,
    /// keyed by `(node id, port name)`. An output port's name is `"out"`.
    ///
    /// Cleared and rebuilt every frame, so it always describes the layout
    /// the user is actually looking at.
    ///
    /// Public because the headless panel tests drive interactions at the
    /// coordinates the panel actually used, rather than hardcoding pixel
    /// positions -- a rule this project adopted after those positions had to
    /// be re-measured by hand three times in PR #1727. The usual helper for
    /// that walks `Shape::Text` galleys, which cannot work here: a port is a
    /// `Shape::Circle` and a node body is a `Shape::Rect`, neither of which
    /// carries any text to find. Recording the geometry satisfies the rule
    /// by construction instead.
    pub last_port_positions: HashMap<(u32, String), egui::Pos2>,
    /// In-progress connection drag: the source `(node id, port name)`.
    ///
    /// Rendered as a wire following the pointer. Task 3 is what sets it.
    dragging_from: Option<(u32, String)>,
    /// Last compile error, shown inline beside the offending node.
    ///
    /// Task 4 is what sets it; sub-step 1/2 made errors values carrying node
    /// ids specifically so this could point at the node responsible.
    last_error: Option<GraphError>,
}

impl ShaderGraphPanel {
    /// An editor opened on `graph`.
    pub fn new(graph: ShaderGraph) -> Self {
        Self {
            graph,
            ..Default::default()
        }
    }

    /// The node's box in screen coordinates, given the canvas it sits in.
    ///
    /// `node.position` is panel-local, so the canvas origin is the only
    /// thing standing between the model and the screen -- which is what
    /// would make a pan/zoom a change to this function alone.
    fn node_rect(canvas: egui::Rect, node: &bsengine_shadergraph::GraphNode) -> egui::Rect {
        // Every node has one output row, so a node with no inputs is still a
        // row tall rather than collapsing to its title bar.
        let rows = node.kind.input_ports().len().max(1) as f32;
        egui::Rect::from_min_size(
            canvas.min + egui::vec2(node.position[0], node.position[1]),
            egui::vec2(NODE_WIDTH, HEADER_HEIGHT + rows * PORT_ROW_HEIGHT),
        )
    }

    /// The centre of the `index`-th input port on the left edge of `rect`.
    fn input_port_pos(rect: egui::Rect, index: usize) -> egui::Pos2 {
        egui::pos2(
            rect.left(),
            rect.top() + HEADER_HEIGHT + (index as f32 + 0.5) * PORT_ROW_HEIGHT,
        )
    }

    /// The centre of the output port on the right edge of `rect`.
    fn output_port_pos(rect: egui::Rect) -> egui::Pos2 {
        egui::pos2(
            rect.right(),
            rect.top() + HEADER_HEIGHT + PORT_ROW_HEIGHT * 0.5,
        )
    }
}

/// The label shown in a node's title bar.
///
/// Inline parameters are part of the title rather than an edit field: a
/// `Constant` whose value is invisible is indistinguishable from every other
/// `Constant` on the canvas.
fn node_title(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Uv => "UV".to_string(),
        NodeKind::Time => "Time".to_string(),
        NodeKind::Constant(v) => format!("Constant {v}"),
        NodeKind::ConstantVec3([x, y, z]) => format!("Vec3 {x}, {y}, {z}"),
        NodeKind::TextureSample => "Texture Sample".to_string(),
        NodeKind::Add => "Add".to_string(),
        NodeKind::Multiply => "Multiply".to_string(),
        NodeKind::Sin => "Sin".to_string(),
        NodeKind::Lerp => "Lerp".to_string(),
        NodeKind::Step => "Step".to_string(),
        NodeKind::Fract => "Fract".to_string(),
        NodeKind::Output => "Output".to_string(),
    }
}

/// The colour a port circle is drawn in, by the type it carries.
///
/// `None` is a port that accepts (or produces) whatever it is given -- the
/// polymorphic kinds -- and is drawn neutrally rather than being guessed at,
/// so the canvas never claims a type the compiler has not decided.
fn port_color(ty: Option<ValueType>) -> egui::Color32 {
    match ty {
        Some(ValueType::F32) => egui::Color32::from_rgb(0x9d, 0xd1, 0xff),
        Some(ValueType::Vec2) => egui::Color32::from_rgb(0x8c, 0xe0, 0xa8),
        Some(ValueType::Vec3) => egui::Color32::from_rgb(0xff, 0xc0, 0x77),
        None => egui::Color32::from_rgb(0xb0, 0xb0, 0xb0),
    }
}

/// The node a compile error should be reported next to, when it names one.
///
/// `NoOutput` and `MultipleOutputs` are properties of the whole graph, so
/// they have no node to sit beside and are shown against the canvas instead.
fn error_node(error: &GraphError) -> Option<u32> {
    match error {
        GraphError::NoOutput | GraphError::MultipleOutputs => None,
        // The first stuck node; the rest are stuck waiting on it.
        GraphError::Cycle(ids) => ids.first().copied(),
        GraphError::MissingInput { node, .. } | GraphError::TypeMismatch { node, .. } => {
            Some(*node)
        }
        GraphError::UnknownNode(id) => Some(*id),
    }
}

/// A connection wire: a cubic bezier leaving the source port horizontally
/// and arriving at the destination the same way, so two ports on the same
/// row still read as a curve rather than as a straight line through the
/// nodes between them.
fn wire(from: egui::Pos2, to: egui::Pos2, stroke: egui::Stroke) -> egui::Shape {
    let reach = ((to.x - from.x).abs() * 0.5).max(30.0);
    egui::Shape::CubicBezier(egui::epaint::CubicBezierShape::from_points_stroke(
        [
            from,
            from + egui::vec2(reach, 0.0),
            to - egui::vec2(reach, 0.0),
            to,
        ],
        false,
        egui::Color32::TRANSPARENT,
        stroke,
    ))
}

impl EditorPanel for ShaderGraphPanel {
    fn id(&self) -> &str {
        "shadergraph"
    }

    fn title(&self) -> String {
        "Shader Graph".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        // `click_and_drag` rather than `hover`: the response is what Task 3's
        // node and connection drags read, and sensing it changes nothing
        // about what is drawn.
        let (canvas, _response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let painter = ui.painter_at(canvas);
        let visuals = ui.visuals().clone();
        painter.rect_filled(canvas, egui::Rounding::same(0.0), visuals.extreme_bg_color);

        let label_font = egui::FontId::proportional(12.0);
        let title_font = egui::FontId::proportional(13.0);

        // Pass 1: geometry only. Every port position is known before any
        // wire is drawn, which is what lets wires go underneath the nodes.
        self.last_port_positions.clear();
        let mut node_rects: HashMap<u32, egui::Rect> = HashMap::new();
        for node in &self.graph.nodes {
            let rect = Self::node_rect(canvas, node);
            // A duplicated id is authoring corruption the compiler already
            // tolerates by letting the first definition win; do the same
            // here so the two never disagree about which node is which.
            node_rects.entry(node.id).or_insert(rect);
            for (i, port) in node.kind.input_ports().iter().enumerate() {
                self.last_port_positions
                    .entry((node.id, port.name.to_string()))
                    .or_insert_with(|| Self::input_port_pos(rect, i));
            }
            self.last_port_positions
                .entry((node.id, OUTPUT_PORT.to_string()))
                .or_insert_with(|| Self::output_port_pos(rect));
        }

        // Pass 2: wires, under the nodes. An edge naming a port that does
        // not exist simply has nothing to draw -- the compiler ignores such
        // an edge too, so silently not drawing it keeps the two consistent.
        let wire_stroke = egui::Stroke::new(2.0_f32, visuals.weak_text_color());
        for edge in &self.graph.edges {
            let (Some(from), Some(to)) = (
                self.last_port_positions.get(&edge.from),
                self.last_port_positions.get(&edge.to),
            ) else {
                continue;
            };
            painter.add(wire(*from, *to, wire_stroke));
        }

        // The connection being dragged right now follows the pointer.
        if let Some(source) = &self.dragging_from {
            if let (Some(from), Some(pointer)) = (
                self.last_port_positions.get(source),
                ui.ctx().pointer_latest_pos(),
            ) {
                painter.add(wire(
                    *from,
                    pointer,
                    egui::Stroke::new(2.0_f32, visuals.strong_text_color()),
                ));
            }
        }

        // Pass 3: the node boxes, their titles, and their ports.
        for node in &self.graph.nodes {
            let Some(&rect) = node_rects.get(&node.id) else {
                continue;
            };
            let rounding = egui::Rounding::same(4.0);
            painter.rect_filled(rect, rounding, visuals.widgets.inactive.bg_fill);
            let header = egui::Rect::from_min_size(
                rect.min,
                egui::vec2(rect.width(), HEADER_HEIGHT.min(rect.height())),
            );
            painter.rect_filled(header, rounding, visuals.widgets.active.bg_fill);
            painter.rect_stroke(rect, rounding, visuals.widgets.inactive.fg_stroke);
            painter.text(
                header.left_center() + egui::vec2(8.0, 0.0),
                egui::Align2::LEFT_CENTER,
                node_title(&node.kind),
                title_font.clone(),
                visuals.strong_text_color(),
            );

            for (i, port) in node.kind.input_ports().iter().enumerate() {
                let centre = Self::input_port_pos(rect, i);
                painter.circle_filled(centre, PORT_RADIUS, port_color(port.expected));
                painter.text(
                    centre + egui::vec2(PORT_RADIUS + 4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    port.name,
                    label_font.clone(),
                    visuals.text_color(),
                );
            }

            let out = Self::output_port_pos(rect);
            painter.circle_filled(out, PORT_RADIUS, port_color(node.kind.output_type()));
            painter.text(
                out - egui::vec2(PORT_RADIUS + 4.0, 0.0),
                egui::Align2::RIGHT_CENTER,
                OUTPUT_PORT,
                label_font.clone(),
                visuals.text_color(),
            );
        }

        // The last compile failure, beside the node it blames.
        if let Some(error) = &self.last_error {
            let anchor = error_node(error)
                .and_then(|id| node_rects.get(&id))
                .map(|rect| rect.right_top() + egui::vec2(12.0, 0.0))
                .unwrap_or_else(|| canvas.left_top() + egui::vec2(8.0, 8.0));
            painter.text(
                anchor,
                egui::Align2::LEFT_TOP,
                error.to_string(),
                label_font.clone(),
                visuals.error_fg_color,
            );
        }

        if self.graph.nodes.is_empty() {
            painter.text(
                canvas.left_top() + egui::vec2(12.0, 12.0),
                egui::Align2::LEFT_TOP,
                "빈 셰이더 그래프 (empty shader graph)",
                label_font,
                visuals.weak_text_color(),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_core::{InspectorEntityInfo, InspectorState};
    use bsengine_shadergraph::{Edge, GraphNode};

    fn node(id: u32, kind: NodeKind, position: [f32; 2]) -> GraphNode {
        GraphNode { id, kind, position }
    }

    fn edge(from: u32, to: u32, port: &str) -> Edge {
        Edge {
            from: (from, OUTPUT_PORT.to_string()),
            to: (to, port.to_string()),
        }
    }

    /// The demo graph's shape, laid out across the canvas: `Uv + (Time *
    /// 0.1)` -> `Fract` -> `TextureSample` -> `Output`. Deliberately covers
    /// a node with no inputs, one with one, one with two, and one with an
    /// optional port, so a port the panel forgets to lay out has somewhere
    /// to hide only if the test graph is poorer than this one.
    fn demo_graph() -> ShaderGraph {
        ShaderGraph {
            nodes: vec![
                node(0, NodeKind::Uv, [20.0, 30.0]),
                node(1, NodeKind::Time, [20.0, 120.0]),
                node(2, NodeKind::Constant(0.1), [20.0, 200.0]),
                node(3, NodeKind::Multiply, [200.0, 150.0]),
                node(4, NodeKind::Add, [380.0, 60.0]),
                node(5, NodeKind::Fract, [560.0, 60.0]),
                node(6, NodeKind::TextureSample, [740.0, 60.0]),
                node(7, NodeKind::Output, [920.0, 60.0]),
            ],
            edges: vec![
                edge(1, 3, "a"),
                edge(2, 3, "b"),
                edge(0, 4, "a"),
                edge(3, 4, "b"),
                edge(4, 5, "x"),
                edge(5, 6, "uv"),
                edge(6, 7, "color"),
            ],
        }
    }

    /// Runs `panel.ui` for one headless frame against a fixed-size screen,
    /// following `inspector.rs`'s idiom: a bare `egui::Context` with
    /// `FontDefinitions::empty()`, driven through `ctx.run`.
    ///
    /// The screen rect is given explicitly rather than left to
    /// `RawInput::default()` so the canvas -- and therefore every recorded
    /// port position -- is the same size on every machine.
    fn run_frame(panel: &mut ShaderGraphPanel) -> egui::FullOutput {
        let ctx = egui::Context::default();
        ctx.set_fonts(egui::FontDefinitions::empty());
        let mut insp = InspectorState::default();
        let entities_snapshot: Vec<InspectorEntityInfo> = Vec::new();
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::Pos2::ZERO,
                egui::vec2(1200.0, 700.0),
            )),
            ..Default::default()
        };
        ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let mut panel_ctx = EditorPanelContext {
                    insp: &mut insp,
                    entities_snapshot: &entities_snapshot,
                    cursor_pos: (0.0, 0.0),
                    type_registry: None,
                };
                panel.ui(ui, &mut panel_ctx);
            });
        })
    }

    #[test]
    fn merely_rendering_the_panel_leaves_the_graph_untouched() {
        // The opposite-direction guard: a panel that spuriously edited on
        // every frame would still pass any "it draws" assertion. Several
        // frames, because egui settles layout over more than one and a
        // second-frame-only mutation would be the easiest kind to miss.
        let mut panel = ShaderGraphPanel::new(demo_graph());
        let before = panel.graph.clone();

        for _ in 0..3 {
            let output = run_frame(&mut panel);
            // Without this the test would pass just as happily on a `ui()`
            // that returned immediately, which is not "leaves the graph
            // untouched" in any useful sense.
            assert!(
                !output.shapes.is_empty(),
                "the panel must actually have drawn something"
            );
        }

        assert_eq!(
            panel.graph, before,
            "rendering must not add, remove, move or rewire anything"
        );
    }

    #[test]
    fn every_port_gets_a_recorded_position() {
        // Ports the panel did not record cannot be clicked by Task 3's
        // tests, so a missing entry would silently make those tests
        // untestable rather than failing. The count assertion is what makes
        // this a real check: without it, recording one port would pass.
        let mut panel = ShaderGraphPanel::new(demo_graph());
        run_frame(&mut panel);

        let mut expected = 0usize;
        for node in &panel.graph.nodes {
            for port in node.kind.input_ports() {
                let key = (node.id, port.name.to_string());
                assert!(
                    panel.last_port_positions.contains_key(&key),
                    "input port {key:?} was drawn but not recorded"
                );
                expected += 1;
            }
            let out = (node.id, OUTPUT_PORT.to_string());
            assert!(
                panel.last_port_positions.contains_key(&out),
                "output port {out:?} was drawn but not recorded"
            );
            expected += 1;
        }
        assert_eq!(
            panel.last_port_positions.len(),
            expected,
            "the panel recorded ports the graph does not have"
        );
        // 8 outputs, one per node, plus 8 inputs: 2 each on Multiply, Add
        // and Output (`color` and the optional `alpha`), 1 each on Fract and
        // TextureSample, and none on Uv/Time/Constant. Spelled out so a
        // change to the port table cannot quietly change what this covers.
        assert_eq!(expected, 8 + 8);

        // Two ports sharing a position would make a Task 3 click land on
        // whichever the hit-test happened to try first.
        let mut seen: Vec<(u32, u32)> = panel
            .last_port_positions
            .values()
            .map(|p| (p.x.to_bits(), p.y.to_bits()))
            .collect();
        seen.sort_unstable();
        let before = seen.len();
        seen.dedup();
        assert_eq!(
            before,
            seen.len(),
            "two ports were laid out on top of each other"
        );

        // Every recorded position must be somewhere a pointer can reach.
        for (key, pos) in &panel.last_port_positions {
            assert!(
                pos.x.is_finite() && pos.y.is_finite() && pos.x >= 0.0 && pos.y >= 0.0,
                "port {key:?} was recorded off-screen at {pos:?}"
            );
        }
    }

    #[test]
    fn a_port_position_is_recorded_where_the_port_is_drawn() {
        // `last_port_positions` is only useful to Task 3 if it agrees with
        // the geometry: a map full of plausible-but-wrong points would pass
        // the test above and make every later interaction miss. Anchor it to
        // the node box the panel derives from `position`.
        let mut panel = ShaderGraphPanel::new(ShaderGraph {
            nodes: vec![node(7, NodeKind::Output, [100.0, 50.0])],
            edges: vec![],
        });
        run_frame(&mut panel);

        let colour = panel.last_port_positions[&(7, "color".to_string())];
        let alpha = panel.last_port_positions[&(7, "alpha".to_string())];
        let out = panel.last_port_positions[&(7, OUTPUT_PORT.to_string())];

        // The canvas starts below the CentralPanel's own margin, so the
        // absolute origin is not asserted -- the relationships are.
        assert_eq!(colour.x, alpha.x, "both inputs sit on the left edge");
        assert!(
            out.x - colour.x == NODE_WIDTH,
            "the output sits one node width to the right: {out:?} vs {colour:?}"
        );
        assert_eq!(
            alpha.y - colour.y,
            PORT_ROW_HEIGHT,
            "the second input sits one row below the first"
        );
        assert_eq!(out.y, colour.y, "the output shares the first input's row");
    }
}
