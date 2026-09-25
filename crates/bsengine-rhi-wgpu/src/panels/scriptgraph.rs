//! Script graph node editor: build a `bsengine_visualscript::ScriptGraph`
//! visually and compile it to the JavaScript the runtime loads.
//!
//! The same shape as `shadergraph.rs`, and on purpose: a toolbar, a canvas
//! painted with `egui::Painter` in panel-local coordinates, ports hit-tested
//! against the geometry the user is looking at, and every accept/refuse
//! decision taken by reading the compiler's own port tables. Two things a
//! script graph has that a shader graph does not, and this panel adds: nodes
//! with **several output ports** (a `Branch` has two flows, `Vec3Split` three
//! values), so outputs are laid out in rows like inputs; and **inline
//! parameters** an author has to be able to edit -- a literal's value, a key
//! name, a variable name -- so the selected node gets an editor row, and the
//! graph's variables a list of their own.
//!
//! Unreal's Blueprint editor and Unity's graph editor both draw exec wires and
//! data wires on one canvas, and both let a flow output connect to exactly
//! one node; a second connection replaces the first here for the same reason
//! the compiler refuses two (`GraphError::AmbiguousFlow`).

use bsengine_core::{EditorPanel, EditorPanelContext};
use bsengine_visualscript::{
    compile, CompareOp, Edge, GraphError, GraphNode, NodeKind, PortType, ScriptGraph, Value,
    ValueType, Variable, OPS,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Width of a node box, in panel-local pixels. Wider than the shader graph's:
/// call nodes carry a parameter name per row on the left and a result on the
/// right, and `setSaveField(entity, key, value)` needs the room.
const NODE_WIDTH: f32 = 184.0;
/// Height of a node's title bar.
const HEADER_HEIGHT: f32 = 22.0;
/// Vertical pitch of one port row inside a node body.
const PORT_ROW_HEIGHT: f32 = 18.0;
/// Radius of the circle drawn for a port.
const PORT_RADIUS: f32 = 5.0;
/// How close to a port's centre a press must land to grab it. Under half a
/// row, so it can never reach the port on the row below.
const PORT_HIT_RADIUS: f32 = 8.0;

/// A node being moved by the pointer; see `shadergraph.rs`'s `NodeDrag` for
/// why the grab offset is captured once rather than accumulated.
#[derive(Clone, Copy)]
struct NodeDrag {
    id: u32,
    grab_offset: egui::Vec2,
}

/// Where every node box and every port circle sits on screen this frame.
#[derive(Default)]
struct Layout {
    nodes: HashMap<u32, egui::Rect>,
    /// `(node id, port name)` to the centre of that port's circle. A node's
    /// input and output names never collide (asserted in the tests), so one
    /// map keyed by name serves both sides.
    ports: HashMap<(u32, String), egui::Pos2>,
}

/// Editor panel for authoring script graphs.
#[derive(Default)]
pub struct ScriptGraphPanel {
    /// The graph being edited.
    pub graph: ScriptGraph,
    /// Where each port was drawn this frame, in **screen** coordinates. Public
    /// for the reason `ShaderGraphPanel::last_port_positions` is: the headless
    /// tests drive interactions at the coordinates the panel actually used.
    pub last_port_positions: HashMap<(u32, String), egui::Pos2>,
    dragging_from: Option<(u32, String)>,
    dragging_node: Option<NodeDrag>,
    /// The node the user last clicked: what **Delete Node** removes and what
    /// the parameter editor row edits.
    selected_node: Option<u32>,
    /// Last compile error, shown beside the node it names.
    last_error: Option<GraphError>,
    /// The file this graph was opened from and is saved back to.
    pub path: Option<PathBuf>,
    /// The path text field's contents.
    path_buffer: String,
    /// The JavaScript the last **successful** compile produced. Public so a
    /// test can assert it is byte-identical to `compile(&graph)`.
    pub last_js: Option<String>,
    /// What the last open, save or compile did.
    status: Option<String>,
    /// The parameter editor's text for the selected node, kept separately so
    /// a half-typed number does not become a `Literal(Text)` on every
    /// keystroke; applied when it parses (see [`apply_parameter`]).
    param_buffer: String,
    /// Which node `param_buffer` was filled for, so a change of selection
    /// refills it rather than carrying one node's text to another.
    param_for: Option<u32>,
}

impl ScriptGraphPanel {
    /// An editor opened on `graph`, with no file behind it.
    pub fn new(graph: ScriptGraph) -> Self {
        Self {
            graph,
            ..Default::default()
        }
    }

    /// An editor opened on the graph stored at `path`.
    ///
    /// # Errors
    ///
    /// A human-readable message if the file cannot be read or is not a valid
    /// `ScriptGraph`; the panel shows it and stays on whatever it had.
    pub fn from_path(path: impl Into<PathBuf>) -> Result<Self, String> {
        let mut panel = Self::default();
        panel.open(path)?;
        Ok(panel)
    }

    /// Replaces the graph being edited with the one stored at `path`,
    /// leaving the panel untouched on failure.
    ///
    /// # Errors
    ///
    /// As [`ScriptGraphPanel::from_path`].
    pub fn open(&mut self, path: impl Into<PathBuf>) -> Result<(), String> {
        let path = path.into();
        let text =
            std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let graph: ScriptGraph =
            ron::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        self.graph = graph;
        self.path_buffer = path.display().to_string();
        self.path = Some(path);
        self.last_error = None;
        self.last_js = None;
        self.selected_node = None;
        self.dragging_from = None;
        self.dragging_node = None;
        self.param_for = None;
        Ok(())
    }

    /// Writes the graph back to the file it was opened from, pretty-printed
    /// like every committed RON asset.
    ///
    /// # Errors
    ///
    /// A message if no file is open or the write fails.
    pub fn save(&self) -> Result<(), String> {
        let path = self
            .path
            .as_ref()
            .ok_or_else(|| "no graph file is open to save to".to_string())?;
        let text = ron::ser::to_string_pretty(&self.graph, ron::ser::PrettyConfig::default())
            .map_err(|e| format!("{}: {e}", path.display()))?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Compiles the graph, remembering the outcome for the canvas to show.
    /// The only place the panel calls [`bsengine_visualscript::compile`].
    ///
    /// # Errors
    ///
    /// Passes [`GraphError`] through; it lands in `last_error` and is drawn
    /// beside the node it names.
    pub fn compile_graph(&mut self) -> Result<String, GraphError> {
        let result = compile(&self.graph);
        match &result {
            Ok(js) => {
                self.last_js = Some(js.clone());
                self.last_error = None;
            }
            Err(error) => {
                self.last_error = Some(error.clone());
                self.last_js = None;
            }
        }
        result
    }

    /// Where the generated script for a graph file goes: the whole
    /// `.scriptgraph.ron` suffix is replaced, so `bob.scriptgraph.ron`
    /// becomes `bob.js` -- the name a scene's `script:` points at.
    fn js_path(graph: &Path) -> PathBuf {
        let name = graph
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("script");
        let base = name
            .strip_suffix(".scriptgraph.ron")
            .unwrap_or_else(|| name.rsplit_once('.').map_or(name, |(stem, _)| stem));
        graph.with_file_name(format!("{base}.js"))
    }

    /// Compiles and, when a file is open, writes the `.js` beside it.
    fn compile_and_write(&mut self) {
        match self.compile_graph() {
            Ok(js) => {
                let Some(path) = self.path.clone() else {
                    self.status = Some(
                        "compiled, but no file is open to write the script beside".to_string(),
                    );
                    return;
                };
                let out = Self::js_path(&path);
                self.status = Some(match std::fs::write(&out, js) {
                    Ok(()) => format!("wrote {}", out.display()),
                    Err(e) => format!("{}: {e}", out.display()),
                });
            }
            Err(_) => self.status = Some("the graph has an error".to_string()),
        }
    }

    /// The node's box: as many rows as the longer of its two port columns.
    fn node_rect(canvas: egui::Rect, node: &GraphNode) -> egui::Rect {
        let rows = node
            .kind
            .input_ports()
            .len()
            .max(node.kind.output_ports().len())
            .max(1) as f32;
        egui::Rect::from_min_size(
            canvas.min + egui::vec2(node.position[0], node.position[1]),
            egui::vec2(NODE_WIDTH, HEADER_HEIGHT + rows * PORT_ROW_HEIGHT),
        )
    }

    fn row_y(rect: egui::Rect, index: usize) -> f32 {
        rect.top() + HEADER_HEIGHT + (index as f32 + 0.5) * PORT_ROW_HEIGHT
    }

    fn input_port_pos(rect: egui::Rect, index: usize) -> egui::Pos2 {
        egui::pos2(rect.left(), Self::row_y(rect, index))
    }

    fn output_port_pos(rect: egui::Rect, index: usize) -> egui::Pos2 {
        egui::pos2(rect.right(), Self::row_y(rect, index))
    }

    /// Lays every node and port out in `canvas`; a pure function of the
    /// graph and the canvas rect.
    fn layout_of(canvas: egui::Rect, graph: &ScriptGraph) -> Layout {
        let mut layout = Layout::default();
        for node in &graph.nodes {
            let rect = Self::node_rect(canvas, node);
            layout.nodes.entry(node.id).or_insert(rect);
            for (i, port) in node.kind.input_ports().iter().enumerate() {
                layout
                    .ports
                    .entry((node.id, port.name.to_string()))
                    .or_insert_with(|| Self::input_port_pos(rect, i));
            }
            for (i, port) in node.kind.output_ports().iter().enumerate() {
                layout
                    .ports
                    .entry((node.id, port.name.to_string()))
                    .or_insert_with(|| Self::output_port_pos(rect, i));
            }
        }
        layout
    }

    /// The nearest port within [`PORT_HIT_RADIUS`] of `pos`, ties broken by
    /// key so the same press always grabs the same port.
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

    /// The node whose box contains `pos`, topmost first.
    fn node_at(&self, layout: &Layout, pos: egui::Pos2) -> Option<u32> {
        self.graph
            .nodes
            .iter()
            .rev()
            .find(|node| layout.nodes.get(&node.id).is_some_and(|r| r.contains(pos)))
            .map(|node| node.id)
    }

    fn node(&self, id: u32) -> Option<&GraphNode> {
        self.graph.nodes.iter().find(|n| n.id == id)
    }

    /// The type a `Data(None)` port resolves to: the named variable's, from
    /// its initial value, or `None` if the graph does not declare it (the
    /// compiler then reports `UnknownVariable`; the editor does not refuse
    /// the connection, so an author can wire first and declare after).
    fn variable_type(&self, kind: &NodeKind) -> Option<ValueType> {
        let (NodeKind::GetVar(name) | NodeKind::SetVar(name)) = kind else {
            return None;
        };
        self.graph
            .variables
            .iter()
            .find(|v| &v.name == name)
            .map(|v| match v.initial {
                Value::Number(_) => ValueType::Number,
                Value::Bool(_) => ValueType::Bool,
                Value::Text(_) => ValueType::Text,
            })
    }

    /// Connects two ports if -- and only if -- [`bsengine_visualscript::compile`]
    /// would accept the result, reporting whether it did.
    ///
    /// Direction comes from the port tables: the end that is one of its
    /// node's output ports is the source. Flow to flow connects; data to
    /// data connects when the input accepts the output's type; flow to data
    /// is refused, as the compiler refuses it. A new connection replaces the
    /// old one on the same input, and a new flow replaces the old one on the
    /// same flow output, so the canvas never shows a wire the compiler would
    /// ignore or reject.
    fn try_connect(&mut self, a: (u32, String), b: (u32, String)) -> bool {
        let is_output = |this: &Self, end: &(u32, String)| {
            this.node(end.0)
                .is_some_and(|n| n.kind.output_ports().iter().any(|p| p.name == end.1))
        };
        let is_input = |this: &Self, end: &(u32, String)| {
            this.node(end.0)
                .is_some_and(|n| n.kind.input_ports().iter().any(|p| p.name == end.1))
        };
        let (from, to) = match (
            is_output(self, &a) && is_input(self, &b),
            is_output(self, &b) && is_input(self, &a),
        ) {
            (true, _) => (a, b),
            (false, true) => (b, a),
            _ => return false,
        };

        let accepted = {
            let source = self.node(from.0).expect("checked above");
            let dest = self.node(to.0).expect("checked above");
            let src_port = source
                .kind
                .output_ports()
                .into_iter()
                .find(|p| p.name == from.1)
                .expect("checked above");
            let dst_port = dest
                .kind
                .input_ports()
                .into_iter()
                .find(|p| p.name == to.1)
                .expect("checked above");
            match (src_port.ty, dst_port.ty) {
                (PortType::Flow, PortType::Flow) => true,
                (PortType::Data(produced), PortType::Data(expected)) => {
                    let produced = produced.or_else(|| self.variable_type(&source.kind));
                    let expected = expected.or_else(|| self.variable_type(&dest.kind));
                    match (expected, produced) {
                        (Some(expected), Some(produced)) => expected.accepts(produced),
                        _ => true,
                    }
                }
                _ => false,
            }
        };
        if !accepted {
            return false;
        }

        let flow = self.node(from.0).is_some_and(|n| {
            n.kind
                .output_ports()
                .iter()
                .any(|p| p.name == from.1 && p.ty == PortType::Flow)
        });
        if flow {
            self.graph.edges.retain(|e| e.from != from);
        }
        self.graph.edges.retain(|e| e.to != to);
        self.graph.edges.push(Edge { from, to });
        true
    }

    /// Removes a node and every connection that mentions it.
    fn delete_node(&mut self, id: u32) {
        self.graph.nodes.retain(|n| n.id != id);
        self.graph.edges.retain(|e| e.from.0 != id && e.to.0 != id);
        if self.selected_node == Some(id) {
            self.selected_node = None;
        }
    }

    /// An id no node in the graph is using: one past the highest.
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

    /// A canvas spot no existing node overlaps, walked on a coarse grid.
    fn free_position(&self) -> [f32; 2] {
        const COLUMN: f32 = NODE_WIDTH + 40.0;
        const ROW: f32 = HEADER_HEIGHT + 4.0 * PORT_ROW_HEIGHT + 24.0;
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
        [20.0, 20.0]
    }

    /// A name for a new `GetVar`/`SetVar`: the first declared variable's, so
    /// the node is valid on arrival when there is one.
    fn default_variable_name(&self) -> String {
        self.graph
            .variables
            .first()
            .map_or_else(|| "var".to_string(), |v| v.name.clone())
    }
}

/// The node kinds the **Add Node** menu offers, grouped as the menu shows
/// them. Every kind is here -- a kind missing from the menu would be
/// reachable only by hand-editing the RON, which is what this panel exists
/// to replace. `Call` is one entry per engine function.
fn addable_kinds(panel: &ScriptGraphPanel) -> Vec<(&'static str, Vec<NodeKind>)> {
    let var = panel.default_variable_name();
    vec![
        (
            "Events",
            vec![
                NodeKind::OnStart,
                NodeKind::OnUpdate,
                NodeKind::OnKeyPressed("Space".to_string()),
                NodeKind::OnCollision,
            ],
        ),
        ("Flow", vec![NodeKind::Branch, NodeKind::Sequence]),
        (
            "Values",
            vec![
                NodeKind::Literal(Value::Number(0.0)),
                NodeKind::Literal(Value::Text(String::new())),
                NodeKind::Literal(Value::Bool(true)),
                NodeKind::SelfEntity,
                NodeKind::GetVar(var.clone()),
                NodeKind::SetVar(var),
            ],
        ),
        (
            "Math",
            vec![
                NodeKind::Add,
                NodeKind::Subtract,
                NodeKind::Multiply,
                NodeKind::Divide,
                NodeKind::Compare(CompareOp::Less),
                NodeKind::Not,
                NodeKind::And,
                NodeKind::Or,
                NodeKind::Vec3Make,
                NodeKind::Vec3Split,
            ],
        ),
        (
            "Calls",
            OPS.iter()
                .map(|o| NodeKind::Call(o.name.to_string()))
                .collect(),
        ),
    ]
}

/// The id of the **Add Node** popup; a process-global constant for the
/// reason `shadergraph.rs` gives, and distinct from that panel's.
fn add_menu_id() -> egui::Id {
    egui::Id::new("scriptgraph_add_node_popup")
}

fn compare_symbol(op: CompareOp) -> &'static str {
    match op {
        CompareOp::Less => "<",
        CompareOp::LessOrEqual => "<=",
        CompareOp::Greater => ">",
        CompareOp::GreaterOrEqual => ">=",
        CompareOp::Equal => "==",
        CompareOp::NotEqual => "!=",
    }
}

/// The label shown in a node's title bar; inline parameters are part of it,
/// so two literals never look alike.
fn node_title(kind: &NodeKind) -> String {
    match kind {
        NodeKind::OnStart => "On Start".to_string(),
        NodeKind::OnUpdate => "On Update".to_string(),
        NodeKind::OnKeyPressed(key) => format!("On Key \"{key}\""),
        NodeKind::OnCollision => "On Collision".to_string(),
        NodeKind::Branch => "Branch".to_string(),
        NodeKind::Sequence => "Sequence".to_string(),
        NodeKind::Literal(Value::Number(n)) => format!("Literal {n}"),
        NodeKind::Literal(Value::Bool(b)) => format!("Literal {b}"),
        NodeKind::Literal(Value::Text(s)) => format!("Literal \"{s}\""),
        NodeKind::SelfEntity => "Self".to_string(),
        NodeKind::GetVar(name) => format!("Get {name}"),
        NodeKind::SetVar(name) => format!("Set {name}"),
        NodeKind::Call(name) => name.clone(),
        NodeKind::Add => "Add".to_string(),
        NodeKind::Subtract => "Subtract".to_string(),
        NodeKind::Multiply => "Multiply".to_string(),
        NodeKind::Divide => "Divide".to_string(),
        NodeKind::Compare(op) => format!("Compare {}", compare_symbol(*op)),
        NodeKind::Not => "Not".to_string(),
        NodeKind::And => "And".to_string(),
        NodeKind::Or => "Or".to_string(),
        NodeKind::Vec3Make => "Make Vec3".to_string(),
        NodeKind::Vec3Split => "Split Vec3".to_string(),
    }
}

/// The menu label for a kind: the title, except that a literal's default
/// value would read as a stray number in a menu.
fn menu_label(kind: &NodeKind) -> String {
    match kind {
        NodeKind::Literal(Value::Number(_)) => "Literal Number".to_string(),
        NodeKind::Literal(Value::Text(_)) => "Literal Text".to_string(),
        NodeKind::Literal(Value::Bool(_)) => "Literal Bool".to_string(),
        NodeKind::OnKeyPressed(_) => "On Key Pressed".to_string(),
        NodeKind::GetVar(_) => "Get Variable".to_string(),
        NodeKind::SetVar(_) => "Set Variable".to_string(),
        NodeKind::Compare(_) => "Compare".to_string(),
        other => node_title(other),
    }
}

/// The colour a port circle is drawn in. Flow ports are white, as Blueprint
/// draws exec pins; a `Data(None)` port takes its variable's colour when the
/// variable is declared and neutral grey otherwise.
fn port_color(ty: PortType, resolved: Option<ValueType>) -> egui::Color32 {
    match ty {
        PortType::Flow => egui::Color32::from_rgb(0xf0, 0xf0, 0xf0),
        PortType::Data(t) => match t.or(resolved) {
            Some(ValueType::Number) => egui::Color32::from_rgb(0x9d, 0xd1, 0xff),
            Some(ValueType::Bool) => egui::Color32::from_rgb(0xe8, 0x80, 0x80),
            Some(ValueType::Text) => egui::Color32::from_rgb(0xd8, 0xa8, 0xf0),
            Some(ValueType::Entity) => egui::Color32::from_rgb(0xff, 0xc0, 0x77),
            Some(ValueType::Vec3) => egui::Color32::from_rgb(0xf0, 0xe0, 0x60),
            None => egui::Color32::from_rgb(0xb0, 0xb0, 0xb0),
        },
    }
}

/// The node a compile error should be reported next to, when it names one.
/// `NoEvent` and `BadVariableName` are properties of the whole graph.
fn error_node(error: &GraphError) -> Option<u32> {
    match error {
        GraphError::NoEvent | GraphError::BadVariableName(_) => None,
        GraphError::UnknownNode(id) => Some(*id),
        GraphError::UnknownVariable { node, .. }
        | GraphError::UnknownOp { node, .. }
        | GraphError::MissingInput { node, .. }
        | GraphError::TypeMismatch { node, .. }
        | GraphError::AmbiguousFlow { node, .. } => Some(*node),
        // The node the walk was on when it found itself again: the last one.
        GraphError::Cycle(ids) | GraphError::FlowCycle(ids) => ids.last().copied(),
    }
}

/// A connection wire, as `shadergraph.rs` draws one.
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

/// A variable's initial value, as the variables list shows and edits it.
fn value_text(value: &Value) -> String {
    match value {
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => s.clone(),
    }
}

/// A variable's initial value from what was typed: `true`/`false` are a
/// `Bool`, anything that parses as a number is a `Number`, the rest is
/// `Text`. What the list's type column then says is what the compiler will
/// type the variable as.
pub fn parse_value(text: &str) -> Value {
    match text.trim() {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        t => t
            .parse::<f64>()
            .map_or_else(|_| Value::Text(text.to_string()), Value::Number),
    }
}

/// The editable text of a node's inline parameter, or `None` for a kind
/// that has none.
fn parameter_text(kind: &NodeKind) -> Option<String> {
    match kind {
        NodeKind::OnKeyPressed(key) => Some(key.clone()),
        NodeKind::Literal(value) => Some(value_text(value)),
        NodeKind::GetVar(name) | NodeKind::SetVar(name) | NodeKind::Call(name) => {
            Some(name.clone())
        }
        _ => None,
    }
}

/// Applies edited parameter text to a node's kind. A literal keeps its
/// type: a `Literal(Number)` given text that does not parse is left as it
/// was and `false` is returned, so a half-typed `-` never turns the node
/// into a `Literal(Text)` mid-keystroke. A `Call` renamed to a function the
/// compiler does not know is applied anyway -- the compiler reports it
/// beside the node, which is where the author is looking.
pub fn apply_parameter(kind: &mut NodeKind, text: &str) -> bool {
    match kind {
        NodeKind::OnKeyPressed(key) => {
            *key = text.to_string();
            true
        }
        NodeKind::Literal(Value::Number(n)) => match text.trim().parse::<f64>() {
            Ok(v) => {
                *n = v;
                true
            }
            Err(_) => false,
        },
        NodeKind::Literal(Value::Bool(b)) => match text.trim() {
            "true" => {
                *b = true;
                true
            }
            "false" => {
                *b = false;
                true
            }
            _ => false,
        },
        NodeKind::Literal(Value::Text(s)) => {
            *s = text.to_string();
            true
        }
        NodeKind::GetVar(name) | NodeKind::SetVar(name) | NodeKind::Call(name) => {
            *name = text.to_string();
            true
        }
        _ => false,
    }
}

impl EditorPanel for ScriptGraphPanel {
    fn id(&self) -> &str {
        "scriptgraph"
    }

    fn title(&self) -> String {
        "Script Graph".to_string()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _ctx: &mut EditorPanelContext) {
        let mut add_kind: Option<NodeKind> = None;
        let mut delete_selected = false;
        let mut add_button: Option<egui::Response> = None;

        ui.horizontal(|ui| {
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

        let mut open_clicked = false;
        let mut save_clicked = false;
        let mut compile_clicked = false;
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut self.path_buffer)
                    .desired_width(300.0)
                    .hint_text("assets/scripts/name.scriptgraph.ron"),
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

        // The selected node's inline parameter, when it has one.
        if let Some(id) = self.selected_node {
            if let Some(kind) = self.node(id).map(|n| n.kind.clone()) {
                if let Some(text) = parameter_text(&kind) {
                    if self.param_for != Some(id) {
                        self.param_buffer = text;
                        self.param_for = Some(id);
                    }
                    let mut compare: Option<CompareOp> = None;
                    let mut edited = false;
                    ui.horizontal(|ui| {
                        ui.label(format!("Node {id}:"));
                        edited = ui
                            .add(
                                egui::TextEdit::singleline(&mut self.param_buffer)
                                    .desired_width(220.0),
                            )
                            .changed();
                    });
                    if let NodeKind::Compare(op) = kind {
                        let mut chosen = op;
                        egui::ComboBox::from_id_salt(("scriptgraph_compare", id))
                            .selected_text(compare_symbol(chosen))
                            .show_ui(ui, |ui| {
                                for candidate in [
                                    CompareOp::Less,
                                    CompareOp::LessOrEqual,
                                    CompareOp::Greater,
                                    CompareOp::GreaterOrEqual,
                                    CompareOp::Equal,
                                    CompareOp::NotEqual,
                                ] {
                                    ui.selectable_value(
                                        &mut chosen,
                                        candidate,
                                        compare_symbol(candidate),
                                    );
                                }
                            });
                        if chosen != op {
                            compare = Some(chosen);
                        }
                    }
                    if edited {
                        let text = self.param_buffer.clone();
                        if let Some(node) = self.graph.nodes.iter_mut().find(|n| n.id == id) {
                            apply_parameter(&mut node.kind, &text);
                        }
                    }
                    if let Some(op) = compare {
                        if let Some(node) = self.graph.nodes.iter_mut().find(|n| n.id == id) {
                            node.kind = NodeKind::Compare(op);
                        }
                    }
                }
            }
        }

        // The graph's variables: name, initial value and the type that
        // implies, which is the type every Get/Set of it will carry.
        let mut remove_variable: Option<usize> = None;
        let mut add_variable = false;
        egui::CollapsingHeader::new(format!("Variables ({})", self.graph.variables.len()))
            .id_salt("scriptgraph_variables")
            .show(ui, |ui| {
                for (i, v) in self.graph.variables.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(egui::TextEdit::singleline(&mut v.name).desired_width(120.0));
                        let mut text = value_text(&v.initial);
                        if ui
                            .add(egui::TextEdit::singleline(&mut text).desired_width(120.0))
                            .changed()
                        {
                            v.initial = parse_value(&text);
                        }
                        let ty = match v.initial {
                            Value::Number(_) => "Number",
                            Value::Bool(_) => "Bool",
                            Value::Text(_) => "Text",
                        };
                        ui.label(egui::RichText::new(ty).weak().small());
                        if ui.small_button(egui_phosphor::regular::TRASH).clicked() {
                            remove_variable = Some(i);
                        }
                    });
                }
                if ui.button("Add Variable").clicked() {
                    add_variable = true;
                }
            });
        if let Some(i) = remove_variable {
            self.graph.variables.remove(i);
        }
        if add_variable {
            let n = self.graph.variables.len() + 1;
            self.graph.variables.push(Variable {
                name: format!("var{n}"),
                initial: Value::Number(0.0),
            });
        }

        if let Some(button) = &add_button {
            let groups = addable_kinds(self);
            egui::popup::popup_below_widget(
                ui,
                add_menu_id(),
                button,
                egui::popup::PopupCloseBehavior::CloseOnClickOutside,
                |ui| {
                    ui.set_min_width(180.0);
                    egui::ScrollArea::vertical()
                        .max_height(360.0)
                        .show(ui, |ui| {
                            for (group, kinds) in groups {
                                ui.label(egui::RichText::new(group).strong());
                                for kind in kinds {
                                    if ui.button(menu_label(&kind)).clicked() {
                                        add_kind = Some(kind);
                                        ui.memory_mut(|m| m.close_popup());
                                    }
                                }
                            }
                        });
                },
            );
        }
        ui.separator();

        if let Some(kind) = add_kind {
            let id = self.next_node_id();
            let position = self.free_position();
            self.graph.nodes.push(GraphNode { id, kind, position });
            self.selected_node = Some(id);
        }
        if delete_selected {
            if let Some(id) = self.selected_node {
                self.delete_node(id);
            }
        }

        let (canvas, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        let layout = Self::layout_of(canvas, &self.graph);

        if response.drag_started() {
            let press = ui
                .ctx()
                .input(|i| i.pointer.press_origin())
                .or_else(|| response.interact_pointer_pos());
            if let Some(press) = press {
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
            self.selected_node = response
                .interact_pointer_pos()
                .and_then(|pos| self.node_at(&layout, pos));
        }

        let painter = ui.painter_at(canvas);
        let visuals = ui.visuals().clone();
        painter.rect_filled(canvas, egui::Rounding::same(0.0), visuals.extreme_bg_color);
        let label_font = egui::FontId::proportional(12.0);
        let title_font = egui::FontId::proportional(13.0);

        let Layout {
            nodes: node_rects,
            ports,
        } = Self::layout_of(canvas, &self.graph);
        self.last_port_positions = ports;

        // Wires under the nodes: flow wires thick and light, data wires thin.
        for edge in &self.graph.edges {
            let (Some(from), Some(to)) = (
                self.last_port_positions.get(&edge.from),
                self.last_port_positions.get(&edge.to),
            ) else {
                continue;
            };
            let is_flow = self.node(edge.from.0).is_some_and(|n| {
                n.kind
                    .output_ports()
                    .iter()
                    .any(|p| p.name == edge.from.1 && p.ty == PortType::Flow)
            });
            let stroke = if is_flow {
                egui::Stroke::new(3.0_f32, visuals.strong_text_color())
            } else {
                egui::Stroke::new(2.0_f32, visuals.weak_text_color())
            };
            painter.add(wire(*from, *to, stroke));
        }
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

            let resolved = self.variable_type(&node.kind);
            for (i, port) in node.kind.input_ports().iter().enumerate() {
                let centre = Self::input_port_pos(rect, i);
                painter.circle_filled(centre, PORT_RADIUS, port_color(port.ty, resolved));
                painter.text(
                    centre + egui::vec2(PORT_RADIUS + 4.0, 0.0),
                    egui::Align2::LEFT_CENTER,
                    port.name,
                    label_font.clone(),
                    visuals.text_color(),
                );
            }
            for (i, port) in node.kind.output_ports().iter().enumerate() {
                let centre = Self::output_port_pos(rect, i);
                painter.circle_filled(centre, PORT_RADIUS, port_color(port.ty, resolved));
                painter.text(
                    centre - egui::vec2(PORT_RADIUS + 4.0, 0.0),
                    egui::Align2::RIGHT_CENTER,
                    port.name,
                    label_font.clone(),
                    visuals.text_color(),
                );
            }
        }

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
                "빈 스크립트 그래프 (empty script graph)",
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

    /// The demo graph shipped with mini-arena, the only authored
    /// `.scriptgraph.ron` there is.
    const DEMO_GRAPH_ASSET: &str = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../games/mini-arena/assets/scripts/bob.scriptgraph.ron"
    );

    /// A copy of the shipped demo graph in a scratch directory: Compile
    /// writes a `.js` beside it and Save rewrites it.
    fn demo_graph_copy(test_name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("bse_scriptgraph_{test_name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("the scratch directory must be creatable");
        let path = dir.join("bob.scriptgraph.ron");
        std::fs::copy(DEMO_GRAPH_ASSET, &path)
            .unwrap_or_else(|e| panic!("the shipped demo graph must be readable: {e}"));
        path
    }

    fn node(id: u32, kind: NodeKind, position: [f32; 2]) -> GraphNode {
        GraphNode { id, kind, position }
    }

    fn edge(from: (u32, &str), to: (u32, &str)) -> Edge {
        Edge {
            from: (from.0, from.1.to_string()),
            to: (to.0, to.1.to_string()),
        }
    }

    fn call(name: &str) -> NodeKind {
        NodeKind::Call(name.to_string())
    }

    fn position_of(graph: &ScriptGraph, id: u32) -> [f32; 2] {
        graph
            .nodes
            .iter()
            .find(|n| n.id == id)
            .unwrap_or_else(|| panic!("node {id} must be in the graph"))
            .position
    }

    /// `OnUpdate -> Branch(isKeyDown("Space")) -> addPosition(self,
    /// vec3(0, speed, 0))`: an event, a flow node with two outputs, a pure
    /// call, literals, a variable and an impure call, laid out across the
    /// canvas so no two nodes overlap.
    fn demo_graph() -> ScriptGraph {
        ScriptGraph {
            nodes: vec![
                node(0, NodeKind::OnUpdate, [20.0, 30.0]),
                node(1, NodeKind::Branch, [260.0, 30.0]),
                node(2, call("isKeyDown"), [20.0, 150.0]),
                node(
                    3,
                    NodeKind::Literal(Value::Text("Space".to_string())),
                    [20.0, 250.0],
                ),
                node(4, call("addPosition"), [520.0, 30.0]),
                node(5, NodeKind::SelfEntity, [260.0, 170.0]),
                node(6, NodeKind::Vec3Make, [260.0, 270.0]),
                node(7, NodeKind::Literal(Value::Number(0.0)), [20.0, 350.0]),
                node(8, NodeKind::GetVar("speed".to_string()), [20.0, 450.0]),
            ],
            edges: vec![
                edge((0, "then"), (1, "exec")),
                edge((2, "out"), (1, "condition")),
                edge((3, "out"), (2, "key")),
                edge((1, "true"), (4, "exec")),
                edge((5, "out"), (4, "entity")),
                edge((6, "out"), (4, "delta")),
                edge((7, "out"), (6, "x")),
                edge((8, "out"), (6, "y")),
                edge((7, "out"), (6, "z")),
            ],
            variables: vec![Variable {
                name: "speed".to_string(),
                initial: Value::Number(2.0),
            }],
        }
    }

    /// Headless multi-frame harness, the shape `shadergraph.rs` uses; see
    /// its `Harness` for why one `Context` spans the whole test and why the
    /// screen rect is fixed.
    struct Harness {
        egui_ctx: egui::Context,
        screen_rect: egui::Rect,
        insp: InspectorState,
        entities_snapshot: Vec<InspectorEntityInfo>,
        panel: ScriptGraphPanel,
    }

    impl Harness {
        fn new(graph: ScriptGraph) -> Self {
            let egui_ctx = egui::Context::default();
            egui_ctx.set_fonts(egui::FontDefinitions::empty());
            Self {
                egui_ctx,
                screen_rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 800.0)),
                insp: InspectorState::default(),
                entities_snapshot: Vec::new(),
                panel: ScriptGraphPanel::new(graph),
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

        fn settle(&mut self) -> egui::FullOutput {
            self.draw();
            self.draw()
        }

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

        fn port(&self, node: u32, name: &str) -> egui::Pos2 {
            *self
                .panel
                .last_port_positions
                .get(&(node, name.to_string()))
                .unwrap_or_else(|| panic!("the panel must have laid out port {node}.{name}"))
        }

        /// The centre of a node's title bar, derived from its first output
        /// port's recorded position and the layout constants.
        fn node_grab_point(&self, id: u32) -> egui::Pos2 {
            let kind = &self
                .panel
                .node(id)
                .unwrap_or_else(|| panic!("node {id} must exist"))
                .kind;
            let first_out = kind.output_ports()[0].name;
            let out = self.port(id, first_out);
            egui::pos2(
                out.x - NODE_WIDTH * 0.5,
                out.y - PORT_ROW_HEIGHT * 0.5 - HEADER_HEIGHT * 0.5,
            )
        }
    }

    fn collect_rendered_texts_with_pos(
        shapes: &[egui::epaint::ClippedShape],
    ) -> Vec<(String, egui::Pos2)> {
        fn walk(shape: &egui::Shape, out: &mut Vec<(String, egui::Pos2)>) {
            match shape {
                egui::Shape::Text(t) => out.push((t.galley.text().to_string(), t.pos)),
                egui::Shape::Vec(nested) => nested.iter().for_each(|s| walk(s, out)),
                _ => {}
            }
        }
        let mut out = Vec::new();
        for clipped in shapes {
            walk(&clipped.shape, &mut out);
        }
        out
    }

    fn text_pos(output: &egui::FullOutput, predicate: impl Fn(&str) -> bool) -> Option<egui::Pos2> {
        collect_rendered_texts_with_pos(&output.shapes)
            .into_iter()
            .find(|(text, _)| predicate(text))
            .map(|(_, pos)| pos)
    }

    /// One port map keyed by name serves inputs and outputs only if no kind
    /// names a port on both sides. Every kind and every call is checked, so
    /// a new op with a parameter called `out` or `then` fails here rather
    /// than drawing two ports at one key.
    #[test]
    fn no_node_kind_names_an_input_and_an_output_alike() {
        let mut kinds: Vec<NodeKind> = addable_kinds(&ScriptGraphPanel::default())
            .into_iter()
            .flat_map(|(_, kinds)| kinds)
            .collect();
        kinds.push(NodeKind::Call("playSound".to_string()));
        for kind in kinds {
            for i in kind.input_ports() {
                assert!(
                    !kind.output_ports().iter().any(|o| o.name == i.name),
                    "{kind:?} has port {:?} on both sides",
                    i.name
                );
            }
        }
    }

    #[test]
    fn merely_rendering_the_panel_leaves_the_graph_untouched() {
        let mut h = Harness::new(demo_graph());
        let before = demo_graph();
        for _ in 0..3 {
            let output = h.draw();
            assert!(!output.shapes.is_empty(), "the panel must have drawn");
        }
        assert_eq!(h.panel.graph, before);
    }

    /// Every input and every output port is recorded, no two at one spot,
    /// all on screen. The count is spelled out so a port-table change cannot
    /// quietly change what this covers.
    #[test]
    fn every_port_of_every_node_gets_a_recorded_position() {
        let mut h = Harness::new(demo_graph());
        h.draw();
        let panel = &h.panel;
        let mut expected = 0usize;
        for node in &panel.graph.nodes {
            for port in node
                .kind
                .input_ports()
                .iter()
                .chain(node.kind.output_ports().iter())
            {
                let key = (node.id, port.name.to_string());
                assert!(
                    panel.last_port_positions.contains_key(&key),
                    "port {key:?} was drawn but not recorded"
                );
                expected += 1;
            }
        }
        assert_eq!(panel.last_port_positions.len(), expected);
        // OnUpdate 1 out; Branch 2 in + 2 out; isKeyDown 1 in + 1 out;
        // Literal 1 out; addPosition 3 in + 1 out; Self 1 out; Make Vec3
        // 3 in + 1 out; Literal 1 out; GetVar 1 out.
        assert_eq!(expected, 1 + 4 + 2 + 1 + 4 + 1 + 4 + 1 + 1);

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
        for (key, pos) in &panel.last_port_positions {
            assert!(
                pos.x.is_finite() && pos.y.is_finite() && pos.x >= 0.0 && pos.y >= 0.0,
                "port {key:?} was recorded off-screen at {pos:?}"
            );
        }
    }

    /// Inputs down the left edge, outputs down the right, one row each,
    /// and a node with more outputs than inputs is as tall as its outputs.
    #[test]
    fn ports_are_laid_out_in_rows_on_their_own_edge() {
        let mut h = Harness::new(ScriptGraph {
            nodes: vec![
                node(1, NodeKind::Branch, [100.0, 50.0]),
                node(2, NodeKind::Vec3Split, [400.0, 50.0]),
            ],
            ..Default::default()
        });
        h.draw();
        let exec = h.port(1, "exec");
        let condition = h.port(1, "condition");
        let t = h.port(1, "true");
        let f = h.port(1, "false");
        assert_eq!(exec.x, condition.x, "both inputs on the left edge");
        assert_eq!(t.x, f.x, "both outputs on the right edge");
        assert_eq!(t.x - exec.x, NODE_WIDTH);
        assert_eq!(condition.y - exec.y, PORT_ROW_HEIGHT);
        assert_eq!(f.y - t.y, PORT_ROW_HEIGHT);
        assert_eq!(t.y, exec.y, "the first output shares the first input's row");

        let v = h.port(2, "v");
        let z = h.port(2, "z");
        assert_eq!(z.y - v.y, 2.0 * PORT_ROW_HEIGHT, "three output rows");
    }

    #[test]
    fn dragging_a_node_moves_it() {
        let mut h = Harness::new(demo_graph());
        h.settle();
        let before = position_of(&h.panel.graph, 1);
        let grab = h.node_grab_point(1);
        let delta = egui::vec2(37.0, -21.0);
        h.press(grab);
        h.drag_to(grab + delta);
        assert_eq!(
            position_of(&h.panel.graph, 1),
            [before[0] + delta.x, before[1] + delta.y]
        );
        h.release(grab + delta);
        assert!(h.panel.dragging_node.is_none());
        assert_eq!(
            h.panel.graph.edges,
            demo_graph().edges,
            "a move must not rewire"
        );
        for n in &demo_graph().nodes {
            if n.id != 1 {
                assert_eq!(position_of(&h.panel.graph, n.id), n.position);
            }
        }
    }

    /// A flow wire and a data wire, each dragged between compatible ports,
    /// each adding exactly one edge -- and the result compiles.
    #[test]
    fn dragging_between_compatible_ports_creates_flow_and_data_edges() {
        let mut graph = demo_graph();
        graph.edges.retain(|e| e.to != (4, "exec".to_string()));
        graph.edges.retain(|e| e.to != (6, "y".to_string()));
        let mut h = Harness::new(graph);
        h.settle();
        let before = h.panel.graph.edges.len();

        let from = h.port(1, "true");
        let to = h.port(4, "exec");
        h.press(from);
        h.drag_to(to);
        h.release(to);
        assert!(
            h.panel
                .graph
                .edges
                .contains(&edge((1, "true"), (4, "exec"))),
            "the flow edge must be the one dragged: {:?}",
            h.panel.graph.edges
        );

        // Dragged input-to-output this time: direction comes from the ports.
        let from = h.port(6, "y");
        let to = h.port(8, "out");
        h.press(from);
        h.drag_to(to);
        h.release(to);
        assert!(
            h.panel.graph.edges.contains(&edge((8, "out"), (6, "y"))),
            "the data edge must run from the output to the input whichever end was grabbed: {:?}",
            h.panel.graph.edges
        );
        assert_eq!(h.panel.graph.edges.len(), before + 2);
        assert!(h.panel.dragging_from.is_none());
        assert!(
            compile(&h.panel.graph).is_ok(),
            "the completed graph must compile: {:?}",
            compile(&h.panel.graph)
        );
    }

    /// The refusals, each the compiler's own verdict: a `Text` literal on a
    /// `Bool` port, a flow output on a data input, and a data output on a
    /// flow input.
    #[test]
    fn incompatible_connections_are_refused() {
        let mut h = Harness::new(demo_graph());
        h.settle();
        let before = h.panel.graph.edges.clone();

        for (a, b) in [
            ((3, "out"), (1, "condition")),
            ((0, "then"), (6, "x")),
            ((7, "out"), (4, "exec")),
        ] {
            let from = h.port(a.0, a.1);
            let to = h.port(b.0, b.1);
            h.press(from);
            h.drag_to(to);
            h.release(to);
            assert_eq!(
                h.panel.graph.edges, before,
                "{a:?} -> {b:?} must be refused and leave the edges unchanged"
            );
            assert!(h.panel.dragging_from.is_none());
        }

        let mut would_have = demo_graph();
        would_have
            .edges
            .retain(|e| e.to != (1, "condition".to_string()));
        would_have.edges.push(edge((3, "out"), (1, "condition")));
        assert!(
            matches!(
                compile(&would_have),
                Err(GraphError::TypeMismatch { node: 1, .. })
            ),
            "the panel must be refusing exactly what the compiler refuses: {:?}",
            compile(&would_have)
        );
    }

    /// A flow output can continue in one place: connecting `true` to a
    /// second node replaces the first connection rather than adding one the
    /// compiler would reject as `AmbiguousFlow`.
    #[test]
    fn a_second_flow_from_one_output_replaces_the_first() {
        let mut graph = demo_graph();
        graph.nodes.push(node(9, call("quit"), [520.0, 250.0]));
        let mut h = Harness::new(graph);
        h.settle();

        let from = h.port(1, "true");
        let to = h.port(9, "exec");
        h.press(from);
        h.drag_to(to);
        h.release(to);

        let from_true: Vec<&Edge> = h
            .panel
            .graph
            .edges
            .iter()
            .filter(|e| e.from == (1, "true".to_string()))
            .collect();
        assert_eq!(from_true, vec![&edge((1, "true"), (9, "exec"))]);
        assert!(
            !matches!(
                compile(&h.panel.graph),
                Err(GraphError::AmbiguousFlow { .. })
            ),
            "{:?}",
            compile(&h.panel.graph)
        );
    }

    #[test]
    fn a_connection_released_over_empty_canvas_is_cancelled() {
        let mut graph = demo_graph();
        graph.edges.retain(|e| e.to != (4, "exec".to_string()));
        let mut h = Harness::new(graph);
        h.settle();
        let before = h.panel.graph.edges.clone();
        let from = h.port(1, "true");
        let empty = h.port(4, "exec") - egui::vec2(PORT_HIT_RADIUS * 1.5, 0.0);
        h.press(from);
        h.drag_to(empty);
        h.release(empty);
        assert_eq!(h.panel.graph.edges, before);
        assert!(h.panel.dragging_from.is_none());
    }

    #[test]
    fn adding_a_node_from_the_menu_appends_it() {
        let mut h = Harness::new(demo_graph());
        let closed = h.settle();
        let add_button =
            text_pos(&closed, |t| t.contains("Add Node")).expect("the Add Node button must render");
        h.click(add_button);
        let open = h.draw();
        // "Sequence" is a kind the demo graph does not contain, so the only
        // "Sequence" on screen is the menu row.
        let row = text_pos(&open, |t| t == "Sequence").expect("the menu must offer every kind");
        h.click(row);

        assert_eq!(h.panel.graph.nodes.len(), demo_graph().nodes.len() + 1);
        let added = h.panel.graph.nodes.last().expect("a node was just added");
        assert_eq!(added.kind, NodeKind::Sequence);
        assert_eq!(
            h.panel
                .graph
                .nodes
                .iter()
                .filter(|n| n.id == added.id)
                .count(),
            1
        );
        assert!(demo_graph()
            .nodes
            .iter()
            .all(|existing| existing.position != added.position));
        assert_eq!(h.panel.graph.edges, demo_graph().edges);
    }

    /// The menu offers every kind and every engine function. The rows past
    /// the scroll area's first screen are not painted until scrolled to
    /// (egui culls widgets outside the clip rect), so the full list is
    /// asserted on the menu's *contents* and the rendered check covers the
    /// groups and the rows that are on screen.
    #[test]
    fn the_menu_offers_every_kind_and_every_call() {
        let panel = ScriptGraphPanel::default();
        let groups = addable_kinds(&panel);
        let calls: Vec<String> = groups
            .iter()
            .find(|(name, _)| *name == "Calls")
            .map(|(_, kinds)| kinds.iter().map(node_title).collect())
            .expect("a Calls group");
        let expected: Vec<String> = OPS.iter().map(|o| o.name.to_string()).collect();
        assert_eq!(
            calls, expected,
            "one menu row per engine function, in table order"
        );
        let all: Vec<&NodeKind> = groups.iter().flat_map(|(_, k)| k).collect();
        for kind in [
            NodeKind::OnStart,
            NodeKind::OnUpdate,
            NodeKind::OnCollision,
            NodeKind::Branch,
            NodeKind::Sequence,
            NodeKind::SelfEntity,
            NodeKind::Add,
            NodeKind::Subtract,
            NodeKind::Multiply,
            NodeKind::Divide,
            NodeKind::Not,
            NodeKind::And,
            NodeKind::Or,
            NodeKind::Vec3Make,
            NodeKind::Vec3Split,
        ] {
            assert!(all.contains(&&kind), "{kind:?} must be in the menu");
        }
        assert!(all.iter().any(|k| matches!(k, NodeKind::OnKeyPressed(_))));
        assert!(all
            .iter()
            .any(|k| matches!(k, NodeKind::Literal(Value::Number(_)))));
        assert!(all
            .iter()
            .any(|k| matches!(k, NodeKind::Literal(Value::Text(_)))));
        assert!(all
            .iter()
            .any(|k| matches!(k, NodeKind::Literal(Value::Bool(_)))));
        assert!(all.iter().any(|k| matches!(k, NodeKind::GetVar(_))));
        assert!(all.iter().any(|k| matches!(k, NodeKind::SetVar(_))));
        assert!(all.iter().any(|k| matches!(k, NodeKind::Compare(_))));

        let mut h = Harness::new(ScriptGraph::default());
        let closed = h.settle();
        let add_button =
            text_pos(&closed, |t| t.contains("Add Node")).expect("the Add Node button must render");
        h.click(add_button);
        let open = h.draw();
        let texts: Vec<String> = collect_rendered_texts_with_pos(&open.shapes)
            .into_iter()
            .map(|(t, _)| t)
            .collect();
        for expected in [
            "Events",
            "On Key Pressed",
            "Flow",
            "Values",
            "Literal Text",
            "Math",
        ] {
            assert!(
                texts.iter().any(|t| t == expected),
                "{expected:?} must be on the menu's first screen; got {texts:?}"
            );
        }
    }

    #[test]
    fn deleting_a_node_also_removes_its_edges() {
        let mut h = Harness::new(demo_graph());
        h.settle();
        // The Branch has an edge in on each of its inputs and one out.
        let grab = h.node_grab_point(1);
        h.click(grab);
        assert_eq!(h.panel.selected_node, Some(1));
        let selected = h.draw();
        let delete_button = text_pos(&selected, |t| t.contains("Delete Node"))
            .expect("the Delete Node button must render");
        h.click(delete_button);

        assert!(!h.panel.graph.nodes.iter().any(|n| n.id == 1));
        assert!(!h
            .panel
            .graph
            .edges
            .iter()
            .any(|e| e.from.0 == 1 || e.to.0 == 1));
        assert_eq!(h.panel.graph.edges.len(), demo_graph().edges.len() - 3);
        assert_eq!(h.panel.selected_node, None);
        assert!(!matches!(
            compile(&h.panel.graph),
            Err(GraphError::UnknownNode(_))
        ));
    }

    /// The panel is not a second code path to JavaScript, and what reaches
    /// disk is the script under the name a scene's `script:` would point
    /// at: `bob.js`, not `bob.scriptgraph.js`.
    #[test]
    fn compiling_from_the_panel_matches_calling_compile_directly() {
        let path = demo_graph_copy("panel_compile");
        let mut h = Harness::new(ScriptGraph::default());
        h.panel
            .open(&path)
            .unwrap_or_else(|e| panic!("the shipped demo graph must open: {e}"));
        let settled = h.settle();
        let compile_button =
            text_pos(&settled, |t| t.contains("Compile")).expect("the Compile button must render");
        h.click(compile_button);

        let direct = compile(&h.panel.graph).expect("the demo graph must compile");
        assert_eq!(h.panel.last_js.as_deref(), Some(direct.as_str()));
        assert_eq!(h.panel.last_error, None);
        let js_path = path.with_file_name("bob.js");
        let written = std::fs::read_to_string(&js_path)
            .unwrap_or_else(|e| panic!("Compile must write {}: {e}", js_path.display()));
        assert_eq!(written, direct);
        assert!(
            written.starts_with(bsengine_visualscript::compile::HEADER),
            "the written script carries the generated-file header"
        );
    }

    /// A compile error is a value drawn beside the node it names.
    #[test]
    fn a_compile_error_is_surfaced_beside_the_node_it_blames() {
        let mut graph = demo_graph();
        graph.edges.retain(|e| e.to != (6, "y".to_string()));
        let mut h = Harness::new(graph);
        let settled = h.settle();
        let compile_button =
            text_pos(&settled, |t| t.contains("Compile")).expect("the Compile button must render");
        h.click(compile_button);

        let error = h
            .panel
            .last_error
            .clone()
            .expect("a missing input is an error");
        assert_eq!(
            error,
            GraphError::MissingInput {
                node: 6,
                port: "y".to_string()
            }
        );
        assert_eq!(h.panel.last_js, None);

        let after = h.draw();
        let texts = collect_rendered_texts_with_pos(&after.shapes);
        let message = error.to_string();
        let (_, at) = texts
            .iter()
            .find(|(text, _)| *text == message)
            .unwrap_or_else(|| panic!("the error message must be drawn; got {texts:?}"));
        let out = h.port(6, "out");
        assert_eq!(
            *at,
            egui::pos2(out.x + 12.0, out.y - HEADER_HEIGHT - PORT_ROW_HEIGHT * 0.5),
            "the message must sit beside node 6"
        );
    }

    #[test]
    fn the_demo_graph_loads_and_saves_without_changing_meaning() {
        let path = demo_graph_copy("panel_roundtrip");
        let mut h = Harness::new(ScriptGraph::default());
        h.panel.path_buffer = path.display().to_string();
        let settled = h.settle();
        let open_button =
            text_pos(&settled, |t| t.contains("Open")).expect("the Open button must render");
        h.click(open_button);

        let loaded = h.panel.graph.clone();
        assert!(
            loaded.nodes.len() > 10,
            "the demo graph's nodes must survive being opened"
        );
        assert_eq!(h.panel.path.as_deref(), Some(path.as_path()));

        h.panel.graph.nodes[0].position = [123.5, -47.25];
        let edited = h.panel.graph.clone();
        let drawn = h.draw();
        let save_button =
            text_pos(&drawn, |t| t.contains("Save")).expect("the Save button must render");
        h.click(save_button);

        let reloaded = ScriptGraphPanel::from_path(&path)
            .unwrap_or_else(|e| panic!("the file Save just wrote must reopen: {e}"));
        assert_eq!(reloaded.graph, edited);
        assert_eq!(reloaded.graph.nodes[0].position, [123.5, -47.25]);
        assert_eq!(compile(&reloaded.graph), compile(&loaded));
        assert!(compile(&reloaded.graph).is_ok());
    }

    /// Selecting a node with a parameter shows it in the editor row, and
    /// clicking Add Variable declares one the compiler will accept.
    #[test]
    fn the_parameter_row_and_variables_list_edit_the_graph() {
        let mut h = Harness::new(demo_graph());
        h.settle();
        let grab = h.node_grab_point(3);
        h.click(grab);
        assert_eq!(h.panel.selected_node, Some(3));
        let drawn = h.draw();
        assert_eq!(
            h.panel.param_buffer, "Space",
            "the literal's text fills the row"
        );
        assert!(text_pos(&drawn, |t| t == "Node 3:").is_some());

        // The variables list is collapsed by default; open it, then add.
        let header = text_pos(&drawn, |t| t.starts_with("Variables ("))
            .expect("the variables header must render");
        h.click(header);
        let open = h.draw();
        let add =
            text_pos(&open, |t| t == "Add Variable").expect("the button must render once open");
        h.click(add);
        assert_eq!(h.panel.graph.variables.len(), 2);
        assert_eq!(h.panel.graph.variables[1].name, "var2");
        assert!(
            compile(&h.panel.graph).is_ok(),
            "a fresh variable must not break the graph: {:?}",
            compile(&h.panel.graph)
        );
    }

    /// The pure halves of the editors: typed text becomes the value the
    /// compiler will see, and a literal keeps its type through a half-typed
    /// number.
    #[test]
    fn parameter_and_value_parsing_keep_types_honest() {
        assert_eq!(parse_value("2.5"), Value::Number(2.5));
        assert_eq!(parse_value("true"), Value::Bool(true));
        assert_eq!(parse_value("hello"), Value::Text("hello".to_string()));

        let mut n = NodeKind::Literal(Value::Number(1.0));
        assert!(
            !apply_parameter(&mut n, "-"),
            "a half-typed number is not applied"
        );
        assert_eq!(n, NodeKind::Literal(Value::Number(1.0)));
        assert!(apply_parameter(&mut n, "-3"));
        assert_eq!(n, NodeKind::Literal(Value::Number(-3.0)));

        let mut k = NodeKind::OnKeyPressed("Space".to_string());
        assert!(apply_parameter(&mut k, "Escape"));
        assert_eq!(k, NodeKind::OnKeyPressed("Escape".to_string()));

        let mut c = NodeKind::Call("log".to_string());
        assert!(apply_parameter(&mut c, "quit"));
        assert_eq!(c, NodeKind::Call("quit".to_string()));

        assert!(
            !apply_parameter(&mut NodeKind::Branch, "x"),
            "no parameter, nothing applied"
        );
        assert_eq!(parameter_text(&NodeKind::Branch), None);
    }

    /// A variable's declared type decides what its ports accept: a `Text`
    /// literal cannot be set into a `Number` variable, but a `Number` can.
    #[test]
    fn variable_ports_take_the_variables_type() {
        let mut graph = demo_graph();
        graph.nodes.push(node(
            9,
            NodeKind::SetVar("speed".to_string()),
            [520.0, 250.0],
        ));
        let mut h = Harness::new(graph);
        h.settle();

        let from = h.port(3, "out");
        let to = h.port(9, "value");
        h.press(from);
        h.drag_to(to);
        h.release(to);
        assert!(
            !h.panel
                .graph
                .edges
                .iter()
                .any(|e| e.to == (9, "value".to_string())),
            "Text into a Number variable is refused"
        );

        let from = h.port(7, "out");
        h.press(from);
        h.drag_to(to);
        h.release(to);
        assert!(
            h.panel
                .graph
                .edges
                .contains(&edge((7, "out"), (9, "value"))),
            "Number into a Number variable connects"
        );
    }
}
