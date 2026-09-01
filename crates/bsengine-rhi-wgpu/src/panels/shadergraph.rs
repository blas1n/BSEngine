//! Shader graph node editor: build a `bsengine_shadergraph::ShaderGraph`
//! visually and compile it to WGSL. See
//! `docs/superpowers/specs/2026-09-01-shadergraph-ui-design.md`.
//!
//! Nodes are drawn with `egui::Painter` into the panel's own coordinate
//! space rather than each being an `egui::Area`, so a future pan/zoom is a
//! single coordinate transform instead of a change to every hit-test. Pan and
//! zoom themselves are deliberately out of scope for item 50.

use bsengine_core::{EditorPanel, EditorPanelContext};
use bsengine_shadergraph::{
    compile, Edge, GraphError, GraphNode, NodeKind, ShaderGraph, ValueType,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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
/// How close to a port's centre a press must land to grab it.
///
/// Wider than [`PORT_RADIUS`], because a five-pixel target is a miserable one
/// to hit; still under half of [`PORT_ROW_HEIGHT`], so it can never reach the
/// port on the row below and connect the wrong one.
const PORT_HIT_RADIUS: f32 = 8.0;

/// A node being moved by the pointer.
#[derive(Clone, Copy)]
struct NodeDrag {
    /// Which node is being moved.
    id: u32,
    /// The node origin's offset from the pointer, captured once when the drag
    /// began.
    ///
    /// Captured rather than accumulated per frame: summing `drag_delta()`
    /// drifts if a frame is dropped, and it makes the node jump by whatever
    /// the pointer travelled between the press and the first frame egui calls
    /// a drag. With the offset fixed, the node's position is a pure function
    /// of where the pointer is now, so it can never desynchronise.
    grab_offset: egui::Vec2,
}

/// Where every node box and every port circle sits on screen this frame.
///
/// Computed as a whole before anything is hit-tested or drawn, so wires can
/// be painted underneath the nodes and so a press is tested against exactly
/// the geometry the user was looking at.
#[derive(Default)]
struct Layout {
    /// Node id to the node's box.
    nodes: HashMap<u32, egui::Rect>,
    /// `(node id, port name)` to the centre of that port's circle.
    ports: HashMap<(u32, String), egui::Pos2>,
}

/// Editor panel for authoring shader graphs.
///
/// Holds the graph being edited and, as a side effect of drawing it, the
/// screen position of every port it laid out -- which is also what the
/// pointer is hit-tested against, so what the user sees and what a press
/// grabs cannot disagree.
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
    /// In-progress connection drag: the port the wire is being pulled from.
    ///
    /// Rendered as a wire following the pointer, and cleared on release
    /// whether or not the release landed on a compatible port.
    dragging_from: Option<(u32, String)>,
    /// In-progress node move.
    dragging_node: Option<NodeDrag>,
    /// The node the user last clicked; the one **Delete Node** removes.
    ///
    /// Clicking empty canvas clears it, so the delete button is only ever
    /// enabled while something is actually pointed at.
    selected_node: Option<u32>,
    /// Last compile error, shown inline beside the offending node.
    ///
    /// Sub-step 1/2 made every failure a value carrying the node ids it
    /// blames, specifically so the editor could put the message next to the
    /// node responsible instead of in a log the author never reads.
    last_error: Option<GraphError>,
    /// The file this graph was opened from and is saved back to.
    ///
    /// `None` for a graph that only exists in the panel: **Save** is disabled
    /// until there is somewhere to save it to, rather than inventing a path.
    pub path: Option<PathBuf>,
    /// The path text field's contents, which is what **Open** reads.
    ///
    /// Separate from [`ShaderGraphPanel::path`] so a half-typed path never
    /// looks like the open file.
    path_buffer: String,
    /// The WGSL the last **successful** compile produced.
    ///
    /// Public because it is the panel's real output: a test can assert it is
    /// byte-identical to `compile(&graph)`, which is what keeps the panel
    /// from quietly becoming a second, divergent code path to WGSL.
    pub last_wgsl: Option<String>,
    /// What the last open, save or compile did, shown in the toolbar.
    status: Option<String>,
}

impl ShaderGraphPanel {
    /// An editor opened on `graph`, with no file behind it.
    pub fn new(graph: ShaderGraph) -> Self {
        Self {
            graph,
            ..Default::default()
        }
    }

    /// An editor opened on the graph stored at `path`.
    ///
    /// # Errors
    ///
    /// Returns a human-readable message if the file cannot be read or is not
    /// a valid `ShaderGraph`. Both are ordinary authoring mistakes -- a typo
    /// in a path, a hand-edited RON file -- so neither panics; the panel
    /// shows the message and stays open on whatever it already had.
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, String> {
        let mut panel = Self::default();
        panel.open(path)?;
        Ok(panel)
    }

    /// Replaces the graph being edited with the one stored at `path`.
    ///
    /// On failure the panel is left exactly as it was: a mistyped path must
    /// not cost the author the graph they had open.
    ///
    /// # Errors
    ///
    /// As [`ShaderGraphPanel::from_path`].
    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let graph: ShaderGraph =
            ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;

        self.graph = graph;
        self.path_buffer = path.display().to_string();
        self.path = Some(path);
        // Everything below is about the *previous* graph, and carrying any of
        // it over would leave the panel pointing at nodes that no longer
        // exist -- an error message blaming an id from a different file.
        self.last_error = None;
        self.last_wgsl = None;
        self.selected_node = None;
        self.dragging_from = None;
        self.dragging_node = None;
        Ok(())
    }

    /// Writes the graph back to the file it was opened from.
    ///
    /// # Errors
    ///
    /// Returns a message if no file is open or the write fails.
    pub fn save(&self) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| "no graph file is open to save to".to_string())?;
        // Pretty, like the shipped demo graph and like `graph.rs`'s own
        // round-trip test writes them: a graph saved as one long line is
        // unreviewable in a diff, and these files are committed assets.
        let text = ron::ser::to_string_pretty(&self.graph, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Compiles the graph, remembering the outcome for the canvas to show.
    ///
    /// This is the only place the editor calls
    /// [`bsengine_shadergraph::compile`], so the shader the panel produces is
    /// the shader the compiler produces -- there is no second path for the
    /// two to diverge along.
    ///
    /// # Errors
    ///
    /// Passes [`GraphError`] straight through. Half-built graphs are the
    /// normal case while authoring, so a failure here is data, not an
    /// exception: it lands in `last_error` and is drawn beside the node it
    /// names.
    pub fn compile_graph(&mut self) -> Result<String, GraphError> {
        let result = compile(&self.graph);
        match &result {
            Ok(wgsl) => {
                self.last_wgsl = Some(wgsl.clone());
                self.last_error = None;
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                // Dropped rather than kept: leaving the previous success
                // behind would let the panel report WGSL that the graph on
                // screen does not produce.
                self.last_wgsl = None;
            }
        }
        result
    }

    /// Where the generated shader for a graph file goes.
    ///
    /// The whole `.shadergraph.ron` suffix is stripped, not just the last
    /// extension: `Path::set_extension` would turn `scroll.shadergraph.ron`
    /// into `scroll.shadergraph.wgsl`, which reads like a graph file and
    /// would be a confusing thing to point a `CustomShader` at.
    fn wgsl_path(graph: &Path) -> PathBuf {
        let name = graph
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("shader");
        let base = name
            .strip_suffix(".shadergraph.ron")
            .unwrap_or_else(|| name.rsplit_once('.').map_or(name, |(stem, _)| stem));
        graph.with_file_name(format!("{base}.wgsl"))
    }

    /// Compiles and, when a file is open, writes the `.wgsl` beside it.
    ///
    /// The written shader is exactly what `CustomShader.path` already knows
    /// how to load, which is what makes the graph route an addition to the
    /// hand-written WGSL route rather than a replacement for it.
    fn compile_and_write(&mut self) {
        match self.compile_graph() {
            Ok(wgsl) => {
                let Some(path) = self.path.clone() else {
                    self.status = Some(
                        "compiled, but no file is open to write the shader beside".to_string(),
                    );
                    return;
                };
                let out = Self::wgsl_path(&path);
                self.status = Some(match std::fs::write(&out, wgsl) {
                    Ok(()) => format!("wrote {}", out.display()),
                    Err(e) => format!("{}: {e}", out.display()),
                });
            }
            // The message itself is drawn on the canvas beside the offending
            // node, so the toolbar only needs to say that it failed.
            Err(_) => self.status = Some("the graph has an error".to_string()),
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

    /// Lays every node and port out in `canvas`.
    ///
    /// A pure function of the graph and the canvas rect, so it can be called
    /// twice in a frame -- once before the pointer is hit-tested, against the
    /// geometry the user pressed on, and once after any edit, to draw and to
    /// record what is now on screen.
    fn layout_of(canvas: egui::Rect, graph: &ShaderGraph) -> Layout {
        let mut layout = Layout::default();
        for node in &graph.nodes {
            let rect = Self::node_rect(canvas, node);
            // A duplicated id is authoring corruption the compiler already
            // tolerates by letting the first definition win; do the same
            // here so the two never disagree about which node is which.
            layout.nodes.entry(node.id).or_insert(rect);
            for (i, port) in node.kind.input_ports().iter().enumerate() {
                layout
                    .ports
                    .entry((node.id, port.name.to_string()))
                    .or_insert_with(|| Self::input_port_pos(rect, i));
            }
            layout
                .ports
                .entry((node.id, OUTPUT_PORT.to_string()))
                .or_insert_with(|| Self::output_port_pos(rect));
        }
        layout
    }

    /// The port `pos` grabs, if any.
    ///
    /// The *nearest* port within [`PORT_HIT_RADIUS`], with the key breaking a
    /// tie. Two nodes can be dragged so their ports overlap, and picking
    /// whichever a `HashMap` happened to yield first would make the same
    /// press connect different things on different runs.
    fn port_at(layout: &Layout, pos: egui::Pos2) -> Option<(u32, String)> {
        layout
            .ports
            .iter()
            .filter(|(_, centre)| centre.distance(pos) <= PORT_HIT_RADIUS)
            .min_by(|(a_key, a), (b_key, b)| {
                a.distance(pos)
                    .total_cmp(&b.distance(pos))
                    .then_with(|| a_key.cmp(b_key))
            })
            .map(|(key, _)| key.clone())
    }

    /// The node whose box contains `pos`, if any.
    ///
    /// Searched in reverse graph order, because that is the order the nodes
    /// are painted in: where two boxes overlap, the press goes to the one
    /// drawn on top, which is the one the user can see.
    fn node_at(&self, layout: &Layout, pos: egui::Pos2) -> Option<u32> {
        self.graph
            .nodes
            .iter()
            .rev()
            .find(|node| layout.nodes.get(&node.id).is_some_and(|r| r.contains(pos)))
            .map(|node| node.id)
    }

    /// Connects two ports if -- and only if -- [`bsengine_shadergraph::compile`]
    /// would accept the result, reporting whether it did.
    ///
    /// The compatibility test reads [`NodeKind::input_ports`] and
    /// [`NodeKind::output_type`], which *are* the compiler's own table rather
    /// than a copy of it. That is the whole point of exposing them: a
    /// duplicated table here would drift, and the editor would start making
    /// connections the compiler then rejects -- a `TypeMismatch` reported far
    /// from the drag that caused it.
    fn try_connect(&mut self, a: (u32, String), b: (u32, String)) -> bool {
        // Exactly one end must be an output. A drag from one input to another
        // (or between two outputs) names no direction, so there is nothing to
        // build; dragging either way round between an output and an input
        // works, since neither direction is more natural than the other.
        let (from, to) = match (a.1 == OUTPUT_PORT, b.1 == OUTPUT_PORT) {
            (true, false) => (a, b),
            (false, true) => (b, a),
            _ => return false,
        };

        let accepted = {
            let Some(source) = self.graph.nodes.iter().find(|n| n.id == from.0) else {
                return false;
            };
            let Some(dest) = self.graph.nodes.iter().find(|n| n.id == to.0) else {
                return false;
            };
            let Some(port) = dest.kind.input_ports().iter().find(|p| p.name == to.1) else {
                return false;
            };
            // `None` on either side is a polymorphic port whose type the
            // compiler works out from what it is given. Refusing those here
            // would make the editor reject connections `compile` accepts,
            // which is exactly the drift this shared table exists to prevent.
            match (port.expected, source.kind.output_type()) {
                (Some(expected), Some(produced)) => expected == produced,
                _ => true,
            }
        };
        if !accepted {
            return false;
        }

        // Replace rather than append: `compile` lets the *first* edge into a
        // port win, so appending a second would leave the user looking at a
        // wire the generated shader silently ignores.
        self.graph.edges.retain(|e| e.to != to);
        self.graph.edges.push(Edge { from, to });
        true
    }

    /// Removes a node and every connection that mentions it.
    ///
    /// The edges are not optional housekeeping: an edge left pointing at a
    /// deleted node makes the next compile fail with
    /// [`GraphError::UnknownNode`], reported against an id that is no longer
    /// on the canvas and nowhere near the deletion that caused it.
    fn delete_node(&mut self, id: u32) {
        self.graph.nodes.retain(|n| n.id != id);
        self.graph.edges.retain(|e| e.from.0 != id && e.to.0 != id);
        if self.selected_node == Some(id) {
            self.selected_node = None;
        }
    }

    /// An id no node in the graph is using.
    ///
    /// One past the highest, so ids stay ascending and the generated
    /// `n{id}` bindings stay readable; the scan afterwards is what keeps the
    /// answer correct rather than merely usual, since a hand-authored graph
    /// can contain any ids at all.
    fn next_node_id(&self) -> u32 {
        let mut id = self
            .graph
            .nodes
            .iter()
            .map(|n| n.id)
            .max()
            .map_or(0, |m| m.wrapping_add(1));
        while self.graph.nodes.iter().any(|n| n.id == id) {
            id = id.wrapping_add(1);
        }
        id
    }

    /// A canvas spot no existing node overlaps.
    ///
    /// A node dropped on top of another looks exactly like nothing happened,
    /// so the menu walks a coarse grid -- down a column, then on to the next
    /// -- and takes the first free cell. Deterministic, so adding the same
    /// nodes in the same order always produces the same layout.
    fn free_position(&self) -> [f32; 2] {
        /// Horizontal pitch: a node plus a gutter wide enough for its wires.
        const COLUMN: f32 = NODE_WIDTH + 40.0;
        /// Vertical pitch: tall enough for the tallest node kind (three input
        /// rows) plus a gap.
        const ROW: f32 = HEADER_HEIGHT + 3.0 * PORT_ROW_HEIGHT + 24.0;

        for column in 0..32 {
            for row in 0..16 {
                let candidate = [20.0 + column as f32 * COLUMN, 20.0 + row as f32 * ROW];
                let taken = self.graph.nodes.iter().any(|n| {
                    (n.position[0] - candidate[0]).abs() < NODE_WIDTH
                        && (n.position[1] - candidate[1]).abs() < ROW
                });
                if !taken {
                    return candidate;
                }
            }
        }
        // Five hundred occupied cells is far past any graph this compiler is
        // meant for; stacking at the origin beats refusing to add a node.
        [20.0, 20.0]
    }
}

/// The node kinds the **Add Node** menu offers, in the order it lists them.
///
/// All twelve, sources first: a menu that offered a subset would make the
/// remaining kinds reachable only by hand-editing the RON, which is exactly
/// the authoring route this panel exists to replace. The parameterised kinds
/// arrive with neutral defaults -- a constant of zero and an opaque white --
/// since a node has to exist before its parameters can be edited.
fn addable_kinds() -> [NodeKind; 12] {
    [
        NodeKind::Uv,
        NodeKind::Time,
        NodeKind::Constant(0.0),
        NodeKind::ConstantVec3([1.0, 1.0, 1.0]),
        NodeKind::TextureSample,
        NodeKind::Add,
        NodeKind::Multiply,
        NodeKind::Sin,
        NodeKind::Lerp,
        NodeKind::Step,
        NodeKind::Fract,
        NodeKind::Output,
    ]
}

/// The id of the **Add Node** popup.
///
/// A process-global constant rather than `ui.make_persistent_id`, which mixes
/// in the enclosing `Ui`'s own id: the button is built inside a
/// `ui.horizontal` child `Ui` while the popup is opened on the parent, so a
/// `Ui`-relative id would differ between the two and the popup would never
/// open. `dock.rs` builds exactly one `ShaderGraphPanel` (an `or_insert_with`
/// keyed by panel id), so no second panel can claim the same constant.
fn add_menu_id() -> egui::Id {
    egui::Id::new("shadergraph_add_node_popup")
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
        // The toolbar first, so the canvas below it gets whatever is left.
        let mut add_kind: Option<NodeKind> = None;
        let mut delete_selected = false;
        let mut add_button: Option<egui::Response> = None;

        ui.horizontal(|ui| {
            // Explicit size rather than a shrink-wrapped `ui.button`,
            // mirroring hierarchy.rs's Create Terrain button and for its
            // reason: `popup_below_widget` derives the popup's width from
            // this response's rect and `debug_assert!`s it non-negative,
            // which a near-zero shrink-wrapped width can violate under the
            // headless empty-font `Context` these panels are tested with.
            let button_width = ui.available_width().clamp(48.0, 160.0);
            let response = ui.add_sized(
                [button_width, ui.spacing().interact_size.y],
                egui::Button::new(format!("{} Add Node", egui_phosphor::regular::PLUS)),
            );
            if response.clicked() {
                ui.memory_mut(|m| m.toggle_popup(add_menu_id()));
            }
            add_button = Some(response);

            if ui
                .add_enabled(
                    self.selected_node.is_some(),
                    egui::Button::new(format!("{} Delete Node", egui_phosphor::regular::TRASH)),
                )
                .clicked()
            {
                delete_selected = true;
            }
        });

        // Row two: the file the graph lives in, and what to do with it.
        let mut open_clicked = false;
        let mut save_clicked = false;
        let mut compile_clicked = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.path_buffer)
                    .desired_width(300.0)
                    .hint_text("assets/shaders/name.shadergraph.ron"),
            );
            open_clicked = ui
                .button(format!("{} Open", egui_phosphor::regular::FOLDER_OPEN))
                .clicked();
            save_clicked = ui
                .add_enabled(
                    self.path.is_some(),
                    egui::Button::new(format!("{} Save", egui_phosphor::regular::FLOPPY_DISK)),
                )
                .clicked();
            compile_clicked = ui
                .button(format!("{} Compile", egui_phosphor::regular::PLAY))
                .clicked();
        });

        if open_clicked {
            let path = self.path_buffer.trim().to_string();
            if let Err(e) = self.open(path) {
                self.status = Some(e);
            } else {
                self.status = self
                    .path
                    .as_ref()
                    .map(|p| format!("opened {}", p.display()));
            }
        }
        if save_clicked {
            self.status = Some(match self.save() {
                Ok(()) => match &self.path {
                    Some(p) => format!("saved {}", p.display()),
                    None => "saved".to_string(),
                },
                Err(e) => e,
            });
        }
        if compile_clicked {
            self.compile_and_write();
        }
        if let Some(status) = &self.status {
            ui.label(status.as_str());
        }

        if let Some(button) = &add_button {
            egui::popup::popup_below_widget(
                ui,
                add_menu_id(),
                button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(160.0);
                    for kind in addable_kinds() {
                        if ui.button(node_title(&kind)).clicked() {
                            add_kind = Some(kind);
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                },
            );
        }
        ui.separator();

        if let Some(kind) = add_kind {
            let id = self.next_node_id();
            let position = self.free_position();
            self.graph.nodes.push(GraphNode { id, kind, position });
            // Selecting it means the next Delete undoes an accidental add
            // without hunting for the node first.
            self.selected_node = Some(id);
        }
        if delete_selected {
            if let Some(id) = self.selected_node {
                self.delete_node(id);
            }
        }

        // `click_and_drag` rather than `hover`: the response is what the node
        // and connection drags below read.
        let (canvas, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());

        // Hit-testing happens against the layout as it was *before* this
        // frame's edits, which is the geometry that was on screen when the
        // press landed.
        let layout = Self::layout_of(canvas, &self.graph);

        if response.drag_started() {
            // The press origin, not `interact_pointer_pos()`: egui reports a
            // drag as started on the frame the pointer first *moved* while
            // held, and by then `interact_pointer_pos()` has already advanced
            // to where it moved to. Grabbing at that point would miss a port
            // whenever the first frame of movement exceeded PORT_HIT_RADIUS.
            let press = ui
                .ctx()
                .input(|i| i.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos());
            if let Some(press) = press {
                // Ports before node bodies: a port circle straddles the box's
                // edge, so the two overlap and the smaller, more specific
                // target has to win or ports would be unreachable.
                if let Some(port) = Self::port_at(&layout, press) {
                    self.dragging_from = Some(port);
                } else if let Some(id) = self.node_at(&layout, press) {
                    let origin = layout.nodes[&id].min;
                    self.dragging_node = Some(NodeDrag {
                        id,
                        grab_offset: origin - press,
                    });
                    self.selected_node = Some(id);
                }
            }
        }

        if let Some(drag) = self.dragging_node {
            if response.dragged() {
                if let Some(pointer) = response.interact_pointer_pos() {
                    if let Some(node) = self.graph.nodes.iter_mut().find(|n| n.id == drag.id) {
                        let origin = pointer + drag.grab_offset;
                        node.position = [origin.x - canvas.min.x, origin.y - canvas.min.y];
                    }
                }
            } else if response.drag_stopped() {
                self.dragging_node = None;
            }
        }

        if self.dragging_from.is_some() && response.drag_stopped() {
            // Taken unconditionally: a wire released anywhere other than on a
            // compatible port is simply abandoned. Leaving it set would keep
            // a wire trailing the pointer for the rest of the session.
            if let Some(source) = self.dragging_from.take() {
                if let Some(target) = response
                    .interact_pointer_pos()
                    .and_then(|pos| Self::port_at(&layout, pos))
                {
                    self.try_connect(source, target);
                }
            }
        }

        if response.clicked() {
            // A click on empty canvas clears the selection, which is what
            // disables Delete again.
            self.selected_node = response
                .interact_pointer_pos()
                .and_then(|pos| self.node_at(&layout, pos));
        }

        let painter = ui.painter_at(canvas);
        let visuals = ui.visuals().clone();
        painter.rect_filled(canvas, egui::Rounding::same(0.0), visuals.extreme_bg_color);

        let label_font = egui::FontId::proportional(12.0);
        let title_font = egui::FontId::proportional(13.0);

        // Pass 1: geometry, recomputed after this frame's edits so that what
        // is recorded is what is about to be painted.
        let Layout {
            nodes: node_rects,
            ports,
        } = Self::layout_of(canvas, &self.graph);
        self.last_port_positions = ports;

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
            // The selected node is outlined in the selection colour: Delete
            // acts on it, so which node that is must never be a guess.
            let outline = if self.selected_node == Some(node.id) {
                visuals.selection.stroke
            } else {
                visuals.widgets.inactive.fg_stroke
            };
            painter.rect_stroke(rect, rounding, outline);
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

    /// The demo graph shipped with mini-arena -- the only real authored
    /// `.shadergraph.ron` there is, and the file a serialisation asymmetry
    /// would actually corrupt.
    const DEMO_GRAPH_ASSET: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../games/mini-arena/assets/shaders/scroll.shadergraph.ron"
    );

    /// Copies the shipped demo graph into a fresh scratch directory and
    /// returns the copy's path.
    ///
    /// A copy, never the original: **Compile** writes a `.wgsl` beside the
    /// graph and **Save** rewrites the graph itself, so these tests would
    /// otherwise dirty a committed asset.
    fn demo_graph_copy(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bse_shadergraph_{test_name}"));
        // Removed first so a previous run's `.wgsl` cannot be mistaken for
        // one this run wrote.
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch directory must be creatable");
        let path = dir.join("scroll.shadergraph.ron");
        std::fs::copy(DEMO_GRAPH_ASSET, &path)
            .unwrap_or_else(|e| panic!("the shipped demo graph must be readable: {e}"));
        path
    }

    fn node(id: u32, kind: NodeKind, position: [f32; 2]) -> GraphNode {
        GraphNode { id, kind, position }
    }

    /// Where node `id` currently sits, per the graph itself.
    fn position_of(graph: &ShaderGraph, id: u32) -> [f32; 2] {
        graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id} must be in the graph"))
            .position
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

    /// Headless multi-frame harness for `ShaderGraphPanel::ui`, following
    /// `viewport.rs`'s `ViewportHarness` (itself following `hierarchy.rs`):
    /// one `egui::Context` with `FontDefinitions::empty()`, driven frame by
    /// frame through `ctx.run`.
    ///
    /// One `Context` for the whole test, not one per frame, because every
    /// interaction here spans frames -- egui hit-tests a press against the
    /// *previous* frame's widget rects, and reports a drag as started only on
    /// the frame after the press.
    ///
    /// The screen rect is given explicitly rather than left to
    /// `RawInput::default()`, so the canvas -- and therefore every recorded
    /// port position -- is the same size on every machine.
    struct Harness {
        egui_ctx: egui::Context,
        screen_rect: egui::Rect,
        insp: InspectorState,
        entities_snapshot: Vec<InspectorEntityInfo>,
        panel: ShaderGraphPanel,
    }

    impl Harness {
        fn new(graph: ShaderGraph) -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            Self {
                egui_ctx,
                screen_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 700.0)),
                insp: InspectorState::default(),
                entities_snapshot: Vec::new(),
                panel: ShaderGraphPanel::new(graph),
            }
        }

        /// Runs one frame with `events` delivered to it.
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

        /// Runs one frame with no input.
        fn draw(&mut self) -> egui::FullOutput {
            self.frame(Vec::new())
        }

        /// Runs two input-less frames and returns the second.
        ///
        /// Two, not one: a press is hit-tested against the previous frame's
        /// widget rects, so the frame that first lays the canvas out cannot
        /// also receive a press against it. Returning the second frame means
        /// any position read from it is one an immediately following press
        /// will actually hit.
        fn settle(&mut self) -> egui::FullOutput {
            self.draw();
            self.draw()
        }

        /// A frame delivering a full primary-button click at `pos`.
        ///
        /// `pos` must be a position the panel reported -- from
        /// [`Harness::port`], [`Harness::node_grab_point`] or
        /// [`text_pos`] -- and be used exactly as returned. With
        /// `FontDefinitions::empty()` every galley has `row_height: 0.0`, so
        /// a widget's text lands on the vertical centre of its padded rect
        /// and adding a half-row offset to "reach the centre" lands outside
        /// the widget (`inspector.rs:529`).
        fn click(&mut self, pos: egui::Pos2) -> egui::FullOutput {
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

        /// A frame pressing the primary button at `pos`, deliberately not
        /// combined with any subsequent move. egui's
        /// `InputState::is_decidedly_dragging` requires `!any_pressed()`, so
        /// a press and a move sent in one `RawInput` never report
        /// `dragged() == true`: the press must land on its own frame first.
        /// See `viewport.rs`'s `ViewportHarness::press` for the full note.
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

        /// A frame moving the pointer to `pos` with the button still held
        /// from an earlier [`Harness::press`]. This is the frame on which
        /// `drag_started()` and `dragged()` first turn true.
        fn drag_to(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerMoved(pos)])
        }

        /// A frame releasing the primary button at `pos`.
        fn release(&mut self, pos: egui::Pos2) -> egui::FullOutput {
            self.frame(vec![egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::default(),
            }])
        }

        /// Where the panel recorded a port last frame.
        ///
        /// Every coordinate these tests press at comes from here or from
        /// [`Harness::node_grab_point`], never from a measured constant --
        /// the rule this project adopted after PR #1727 had to re-measure
        /// hand-written coordinates three times.
        fn port(&self, node: u32, name: &str) -> egui::Pos2 {
            *self
                .panel
                .last_port_positions
                .get(&(node, name.to_string()))
                .unwrap_or_else(|| panic!("the panel must have laid out port {node}.{name}"))
        }

        /// The centre of a node's title bar: a point inside its box that is
        /// nowhere near a port, so pressing it grabs the node itself.
        ///
        /// Derived from the recorded output-port position and the panel's own
        /// layout constants rather than measured: the output circle sits on
        /// the right edge on the first port row, so the box reaches
        /// `NODE_WIDTH` to its left and the title bar is half a port row plus
        /// half a header above it. A layout change moves this with it.
        fn node_grab_point(&self, id: u32) -> egui::Pos2 {
            let out = self.port(id, OUTPUT_PORT);
            egui::pos2(
                out.x - NODE_WIDTH * 0.5,
                out.y - PORT_ROW_HEIGHT * 0.5 - HEADER_HEIGHT * 0.5,
            )
        }
    }

    /// Every literal string egui rendered as text in one frame, paired with
    /// where it was drawn. Ported from `viewport.rs`'s identical helper
    /// (itself from `inspector.rs`) -- see that copy for the full rationale.
    /// `egui_phosphor` icons are ordinary unicode text, so a button labelled
    /// with an icon and words is found by the words.
    fn collect_rendered_texts_with_pos(
        shapes: &[egui::epaint::ClippedShape],
    ) -> Vec<(String, egui::Pos2)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(text_shape) => {
                    out.push((text_shape.galley.text().to_string(), text_shape.pos))
                }
                egui::Shape::Vec(nested) => {
                    for s in nested {
                        walk(s, out);
                    }
                }
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    /// Where the first text matching `predicate` was drawn in `output`.
    fn text_pos(output: &egui::FullOutput, predicate: impl Fn(&str) -> bool) -> Option<egui::Pos2> {
        collect_rendered_texts_with_pos(&output.shapes)
            .into_iter()
            .find(|(text, _)| predicate(text))
            .map(|(_, pos)| pos)
    }

    #[test]
    fn merely_rendering_the_panel_leaves_the_graph_untouched() {
        // The opposite-direction guard: a panel that spuriously edited on
        // every frame would still pass any "it draws" assertion. Several
        // frames, because egui settles layout over more than one and a
        // second-frame-only mutation would be the easiest kind to miss.
        let mut h = Harness::new(demo_graph());
        let before = demo_graph();

        for _ in 0..3 {
            let output = h.draw();
            // Without this the test would pass just as happily on a `ui()`
            // that returned immediately, which is not "leaves the graph
            // untouched" in any useful sense.
            assert!(
                !output.shapes.is_empty(),
                "the panel must actually have drawn something"
            );
        }

        assert_eq!(
            h.panel.graph, before,
            "rendering must not add, remove, move or rewire anything"
        );
    }

    #[test]
    fn every_port_gets_a_recorded_position() {
        // Ports the panel did not record cannot be clicked by Task 3's
        // tests, so a missing entry would silently make those tests
        // untestable rather than failing. The count assertion is what makes
        // this a real check: without it, recording one port would pass.
        let mut h = Harness::new(demo_graph());
        h.draw();
        let panel = &h.panel;

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
        let mut h = Harness::new(ShaderGraph {
            nodes: vec![node(7, NodeKind::Output, [100.0, 50.0])],
            edges: vec![],
        });
        h.draw();

        let colour = h.port(7, "color");
        let alpha = h.port(7, "alpha");
        let out = h.port(7, OUTPUT_PORT);

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

    #[test]
    fn dragging_a_node_moves_it() {
        let mut h = Harness::new(demo_graph());
        h.settle();

        // Node 3 is the Multiply in the middle of the graph -- surrounded on
        // both sides, so a drag that grabbed the wrong thing has plenty of
        // wrong things available to grab.
        let before = position_of(&h.panel.graph, 3);
        let grab = h.node_grab_point(3);
        let delta = egui::vec2(37.0, -21.0);

        h.press(grab);
        h.drag_to(grab + delta);

        assert_eq!(
            position_of(&h.panel.graph, 3),
            [before[0] + delta.x, before[1] + delta.y],
            "the node must move by exactly the drag delta"
        );

        h.release(grab + delta);
        assert_eq!(
            position_of(&h.panel.graph, 3),
            [before[0] + delta.x, before[1] + delta.y],
            "releasing must leave the node where the drag put it"
        );
        assert!(
            h.panel.dragging_node.is_none(),
            "the drag must end on release, or the node follows the pointer forever"
        );

        // Moving is moving: nothing else about the graph may change. Without
        // this, a "drag" that rewired or duplicated as a side effect passes.
        let expected = demo_graph();
        assert_eq!(
            h.panel.graph.edges, expected.edges,
            "a move must not rewire"
        );
        assert_eq!(h.panel.graph.nodes.len(), expected.nodes.len());
        for node in &expected.nodes {
            if node.id != 3 {
                assert_eq!(
                    position_of(&h.panel.graph, node.id),
                    node.position,
                    "dragging node 3 must not move node {}",
                    node.id
                );
            }
        }
    }

    #[test]
    fn dragging_between_compatible_ports_creates_an_edge() {
        // The main assertion: the panel actually edits. It is made on
        // `graph.edges`, never on pixels -- a panel that renders beautifully
        // and mutates nothing would pass any drawing check.
        //
        // The demo graph minus its last connection: node 6 (TextureSample,
        // vec3 out) has to be wired into node 7 (Output, `color`, vec3 in).
        let mut graph = demo_graph();
        graph.edges.retain(|e| e.to != (7, "color".to_string()));
        let mut h = Harness::new(graph);
        h.settle();

        let before = h.panel.graph.edges.len();
        let from = h.port(6, OUTPUT_PORT);
        let to = h.port(7, "color");

        h.press(from);
        h.drag_to(to);
        h.release(to);

        assert_eq!(
            h.panel.graph.edges.len(),
            before + 1,
            "dragging output to input must add exactly one edge"
        );
        assert!(
            h.panel.graph.edges.contains(&edge(6, 7, "color")),
            "the edge must be the one dragged, from 6's out to 7's color: {:?}",
            h.panel.graph.edges
        );
        assert!(
            h.panel.dragging_from.is_none(),
            "the wire must stop following the pointer once it lands"
        );
        // The strongest statement available that the edge is real and right:
        // the graph the drag completed is one the compiler accepts.
        assert!(
            compile(&h.panel.graph).is_ok(),
            "the completed graph must compile: {:?}",
            compile(&h.panel.graph)
        );
    }

    #[test]
    fn a_type_mismatched_connection_is_refused() {
        // MUTATION TEST for the type guard. Node 6 (TextureSample) produces a
        // vec3; node 7's `alpha` port takes an f32. Without this test the
        // suite proves only that connections *can* be made, and would pass
        // just as happily on an implementation that accepts everything.
        let mut h = Harness::new(demo_graph());
        h.settle();

        let before = h.panel.graph.edges.clone();
        let from = h.port(6, OUTPUT_PORT);
        let to = h.port(7, "alpha");

        h.press(from);
        h.drag_to(to);
        h.release(to);

        assert_eq!(
            h.panel.graph.edges, before,
            "a vec3 output dropped on an f32 input must leave the edges UNCHANGED"
        );
        assert!(
            h.panel.dragging_from.is_none(),
            "a refused connection must still end the drag"
        );

        // And the refusal must be the compiler's own verdict, not a second
        // opinion: had the edge been made, `compile` would have rejected it.
        let mut would_have = demo_graph();
        would_have.edges.push(edge(6, 7, "alpha"));
        assert_eq!(
            compile(&would_have),
            Err(bsengine_shadergraph::GraphError::TypeMismatch {
                node: 7,
                port: "alpha".to_string(),
                expected: "f32".to_string(),
                found: "vec3<f32>".to_string(),
            }),
            "the panel must be refusing exactly what the compiler refuses"
        );
    }

    #[test]
    fn a_connection_released_over_empty_canvas_is_cancelled() {
        // The second mutation test on the same guard, from the other side: an
        // implementation that connected on release without checking *what*
        // was under the pointer passes the compatible-ports test.
        //
        // The graph is the demo one minus its last connection, exactly as in
        // `dragging_between_compatible_ports_creates_an_edge` -- so the drag
        // below is a connection that *would* be accepted if it landed, and
        // any spurious edge shows up as a length change rather than only as a
        // reordering of an already-present one.
        let mut graph = demo_graph();
        graph.edges.retain(|e| e.to != (7, "color".to_string()));
        let mut h = Harness::new(graph);
        h.settle();

        let before = h.panel.graph.edges.clone();
        let from = h.port(6, OUTPUT_PORT);

        // A *near miss*, not a point in the far corner: half a hit radius
        // beyond node 7's `color` port, which is the one port this drag would
        // legitimately connect to. Input ports sit on their node's left edge,
        // so a point to the left of one is outside that node's box, and node
        // 6's box ends well before it -- empty canvas by construction.
        //
        // The far corner would have been the weaker choice. Released there,
        // even an implementation that ignored the hit radius entirely and
        // snapped to the nearest port would have found node 6's own output
        // and refused on direction alone, so the test would have passed
        // without the guard it exists to pin. Here the nearest port is a
        // compatible input, and only the radius stops the connection.
        let empty = h.port(7, "color") - egui::vec2(PORT_HIT_RADIUS * 1.5, 0.0);
        assert!(
            h.screen_rect.contains(empty),
            "the empty point must still be on the canvas: {empty:?}"
        );

        h.press(from);
        h.drag_to(empty);
        h.release(empty);

        assert_eq!(
            h.panel.graph.edges, before,
            "a wire dropped on empty canvas must leave the edges unchanged"
        );
        assert!(
            h.panel.dragging_from.is_none(),
            "the cancelled wire must not keep trailing the pointer"
        );
    }

    #[test]
    fn adding_a_node_from_the_menu_appends_it() {
        let mut h = Harness::new(demo_graph());
        let closed = h.settle();

        let add_button =
            text_pos(&closed, |t| t.contains("Add Node")).expect("the Add Node button must render");
        h.click(add_button);
        // The settle frame is mandatory: a popup's first frame sizes its
        // `Area` from a placeholder and paints none of its rows. See
        // `inspector.rs`'s `open_picker` for the full note.
        let open = h.draw();

        // `Sin` is deliberately a kind the demo graph does not contain, so
        // the only "Sin" on screen is the menu row -- unlike, say, "Add",
        // which is also node 4's title.
        let sin_row = text_pos(&open, |t| t == "Sin").expect("the menu must offer every node kind");
        h.click(sin_row);

        assert_eq!(
            h.panel.graph.nodes.len(),
            demo_graph().nodes.len() + 1,
            "choosing a kind must append exactly one node"
        );
        let added = h.panel.graph.nodes.last().expect("a node was just added");
        assert_eq!(
            added.kind,
            NodeKind::Sin,
            "the kind chosen is the kind added"
        );
        assert_eq!(
            h.panel
                .graph
                .nodes
                .iter()
                .filter(|n| n.id == added.id)
                .count(),
            1,
            "the new node's id must be unused, or the compiler drops it as a duplicate"
        );
        assert!(
            demo_graph()
                .nodes
                .iter()
                .all(|existing| existing.position != added.position),
            "a node dropped on top of another looks like nothing happened"
        );
        assert_eq!(
            h.panel.graph.edges,
            demo_graph().edges,
            "adding a node must not rewire anything"
        );
    }

    #[test]
    fn deleting_a_node_also_removes_its_edges() {
        // The subtle one. An edge left pointing at a deleted node makes the
        // *next* compile fail with `UnknownNode`, naming an id that is no
        // longer on the canvas -- an error arbitrarily far from the deletion
        // that caused it.
        let mut h = Harness::new(demo_graph());
        h.settle();

        // Node 3 (Multiply) is the one with edges on both sides: two in
        // (1 -> a, 2 -> b) and one out (3 -> 4.b). Deleting a leaf would
        // leave an implementation that only cleans up incoming edges intact.
        let grab = h.node_grab_point(3);
        h.click(grab);
        assert_eq!(
            h.panel.selected_node,
            Some(3),
            "clicking a node must select it, or there is nothing for Delete to act on"
        );

        let selected = h.draw();
        let delete_button = text_pos(&selected, |t| t.contains("Delete Node"))
            .expect("the Delete Node button must render");
        h.click(delete_button);

        assert!(
            !h.panel.graph.nodes.iter().any(|n| n.id == 3),
            "the node must be gone"
        );
        assert!(
            !h.panel
                .graph
                .edges
                .iter()
                .any(|e| e.from.0 == 3 || e.to.0 == 3),
            "every edge touching the deleted node must go with it: {:?}",
            h.panel.graph.edges
        );
        assert_eq!(
            h.panel.graph.edges.len(),
            demo_graph().edges.len() - 3,
            "exactly node 3's three edges, and no others, may be removed"
        );
        assert_eq!(
            h.panel.selected_node, None,
            "the selection must not survive the node it pointed at"
        );

        // The property the edge cleanup exists for, asserted directly: no
        // matter what else is now wrong with this half-graph, no edge names a
        // node that is not there.
        assert!(
            !matches!(
                compile(&h.panel.graph),
                Err(bsengine_shadergraph::GraphError::UnknownNode(_))
            ),
            "the graph must not compile to UnknownNode: {:?}",
            compile(&h.panel.graph)
        );
    }

    #[test]
    fn compiling_from_the_panel_matches_calling_compile_directly() {
        // The panel must not become a second, divergent code path to WGSL.
        let path = demo_graph_copy("panel_compile");
        let mut h = Harness::new(ShaderGraph::default());
        h.panel
            .open(&path)
            .unwrap_or_else(|e| panic!("the shipped demo graph must open: {e}"));
        let settled = h.settle();

        let compile_button =
            text_pos(&settled, |t| t.contains("Compile")).expect("the Compile button must render");
        h.click(compile_button);

        let direct = compile(&h.panel.graph).expect("the demo graph must compile");
        assert_eq!(
            h.panel.last_wgsl.as_deref(),
            Some(direct.as_str()),
            "the panel's WGSL must be byte-identical to compile(&graph)"
        );
        assert_eq!(
            h.panel.last_error, None,
            "a successful compile leaves no error"
        );

        // And what reached disk is that same shader, under the name a
        // `CustomShader` would point at -- `scroll.wgsl`, not
        // `scroll.shadergraph.wgsl`.
        let wgsl_path = path.with_file_name("scroll.wgsl");
        let written = std::fs::read_to_string(&wgsl_path)
            .unwrap_or_else(|e| panic!("Compile must write {}: {e}", wgsl_path.display()));
        assert_eq!(
            written, direct,
            "the file written must be the shader the compiler produced"
        );
    }

    #[test]
    fn a_compile_error_is_surfaced_rather_than_panicking() {
        // Nodes 2 and 3 feed each other and 2 feeds the Output, so the cycle
        // is reachable -- the same shape `compile.rs` uses for its own cycle
        // test. Sub-step 1/2 made errors values carrying node ids
        // *specifically* so the UI could show them beside the offending node;
        // this is the payoff, so it is asserted, not just that no panic
        // happened.
        let cyclic = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([1.0, 1.0, 1.0]), [20.0, 30.0]),
                node(1, NodeKind::Output, [600.0, 30.0]),
                node(2, NodeKind::Add, [220.0, 140.0]),
                node(3, NodeKind::Add, [400.0, 260.0]),
            ],
            edges: vec![
                edge(0, 2, "a"),
                edge(3, 2, "b"),
                edge(0, 3, "a"),
                edge(2, 3, "b"),
                edge(2, 1, "color"),
            ],
        };
        let mut h = Harness::new(cyclic);
        let settled = h.settle();

        let compile_button =
            text_pos(&settled, |t| t.contains("Compile")).expect("the Compile button must render");
        h.click(compile_button);

        let error = h
            .panel
            .last_error
            .clone()
            .expect("a cyclic graph must leave an error behind, not panic");
        assert!(
            matches!(error, GraphError::Cycle(_)),
            "the error must be the cycle itself: {error:?}"
        );
        assert_eq!(
            h.panel.last_wgsl, None,
            "a failed compile must not leave WGSL the graph does not produce"
        );

        // The panel still renders -- the whole point of errors being values.
        let after = h.draw();
        assert!(
            !after.shapes.is_empty(),
            "the panel must keep drawing after a failed compile"
        );

        // And the message is on screen, beside the node the error names.
        let texts = collect_rendered_texts_with_pos(&after.shapes);
        let message = error.to_string();
        let (_, at) = texts
            .iter()
            .find(|(text, _)| *text == message)
            .unwrap_or_else(|| panic!("the error message must be drawn; got {texts:?}"));
        let blamed = error_node(&error).expect("a cycle names the nodes it is stuck on");
        let out = h.port(blamed, OUTPUT_PORT);
        // The panel anchors the message at the blamed node's top-right corner
        // plus a small gap, which the output port's recorded position pins:
        // the corner is `NODE_WIDTH`-free on x and one header plus half a
        // port row above it on y.
        assert_eq!(
            *at,
            egui::pos2(out.x + 12.0, out.y - HEADER_HEIGHT - PORT_ROW_HEIGHT * 0.5),
            "the message must sit beside node {blamed}, the one the error blames"
        );
    }

    #[test]
    fn the_demo_graph_loads_and_saves_without_changing_meaning() {
        // A serialisation asymmetry would silently corrupt an artist's file
        // the first time they pressed Save, so the whole cycle is driven
        // through the toolbar the way a user reaches it.
        let path = demo_graph_copy("panel_roundtrip");

        let mut h = Harness::new(ShaderGraph::default());
        h.panel.path_buffer = path.display().to_string();
        let settled = h.settle();
        let open_button =
            text_pos(&settled, |t| t.contains("Open")).expect("the Open button must render");
        h.click(open_button);

        let loaded = h.panel.graph.clone();
        assert_eq!(
            loaded.nodes.len(),
            8,
            "the demo graph's eight nodes must survive being opened"
        );
        assert_eq!(h.panel.path.as_deref(), Some(path.as_path()));

        // Move a node first: layout is the reason `position` was put in the
        // asset at all, so a round trip that lost it would defeat the point.
        h.panel.graph.nodes[0].position = [123.5, -47.25];
        let edited = h.panel.graph.clone();

        let drawn = h.draw();
        let save_button =
            text_pos(&drawn, |t| t.contains("Save")).expect("the Save button must render");
        h.click(save_button);

        let reloaded = ShaderGraphPanel::from_path(&path)
            .unwrap_or_else(|e| panic!("the file Save just wrote must reopen: {e}"));

        assert_eq!(
            reloaded.graph, edited,
            "a save/reload cycle must not change a single node, edge or position"
        );
        assert_eq!(
            reloaded.graph.nodes[0].position,
            [123.5, -47.25],
            "the layout the author arranged must survive the round trip"
        );
        // "Without changing meaning" said in the terms that actually matter:
        // the shader is byte-identical either side of the round trip.
        assert_eq!(
            compile(&reloaded.graph),
            compile(&loaded),
            "the reloaded graph must compile to exactly the same shader"
        );
        assert!(
            compile(&reloaded.graph).is_ok(),
            "and it must still be a graph that compiles at all"
        );
    }
}
