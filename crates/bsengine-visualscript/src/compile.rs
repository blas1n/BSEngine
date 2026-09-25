//! Graph to JavaScript compilation.
//!
//! [`compile`] validates the graph, then writes one `onUpdate(self)` in which
//! every event's flow appears as straight-line statements: a `Branch` is an
//! `if`, a `Sequence` is its arms one after another, a `SetVar` is an
//! assignment, an impure `Call` is a call. Data ports become expressions,
//! inlined where they are read -- a pure node read twice is evaluated twice,
//! as a Blueprint pure node is -- so the generated file reads top to bottom
//! like the script a person would have written.
//!
//! Nothing here panics on malformed input. Every failure is a [`GraphError`]
//! value naming the node responsible, because the node editor compiles
//! half-built graphs on every edit and shows the result beside that node.

use std::collections::HashMap;
use std::fmt::Write as _;

use crate::graph::{CompareOp, Edge, GraphError, GraphNode, NodeKind, ScriptGraph, Value};
use crate::ops::{op, OpSpec};

/// The type a value carries between data ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueType {
    /// A JavaScript number.
    Number,
    /// `true` or `false`.
    Bool,
    /// A string.
    Text,
    /// An entity's name -- a string too, so a `Text` literal connects here
    /// and an entity name connects to a `Text` port. Kept distinct so the
    /// editor can colour and label it as what it is.
    Entity,
    /// A `Bsengine.Vec3`.
    Vec3,
}

impl ValueType {
    /// How the type is named in the editor and in `GraphError::TypeMismatch`.
    pub fn name(self) -> &'static str {
        match self {
            ValueType::Number => "Number",
            ValueType::Bool => "Bool",
            ValueType::Text => "Text",
            ValueType::Entity => "Entity",
            ValueType::Vec3 => "Vec3",
        }
    }

    /// Whether a port of this type accepts a value of `found`.
    pub fn accepts(self, found: ValueType) -> bool {
        self == found
            || matches!(
                (self, found),
                (ValueType::Entity, ValueType::Text) | (ValueType::Text, ValueType::Entity)
            )
    }

    fn of(value: &Value) -> ValueType {
        match value {
            Value::Number(_) => ValueType::Number,
            Value::Bool(_) => ValueType::Bool,
            Value::Text(_) => ValueType::Text,
        }
    }
}

/// What a port carries: control flow, or a value of a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortType {
    /// A flow port: which node runs next.
    Flow,
    /// A data port. `None` means the type is decided by the graph -- a
    /// variable's port takes the variable's type -- and the compiler checks
    /// it once it knows.
    Data(Option<ValueType>),
}

/// One port of a node kind.
///
/// Public so the node editor lays ports out and accepts or refuses a
/// connection by reading **this** table -- the same one the compiler checks
/// against -- rather than a copy that could drift.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Port {
    /// The name an `Edge` carries to connect here.
    pub name: &'static str,
    /// What it carries.
    pub ty: PortType,
}

const fn flow(name: &'static str) -> Port {
    Port {
        name,
        ty: PortType::Flow,
    }
}

const fn data(name: &'static str, ty: ValueType) -> Port {
    Port {
        name,
        ty: PortType::Data(Some(ty)),
    }
}

const EXEC: Port = flow("exec");
const THEN: Port = flow("then");
const A_B_NUMBER: [Port; 2] = [data("a", ValueType::Number), data("b", ValueType::Number)];
const A_B_BOOL: [Port; 2] = [data("a", ValueType::Bool), data("b", ValueType::Bool)];

impl NodeKind {
    /// This kind's input ports, in display order: the flow input first, then
    /// the data inputs in the order the compiler reads them.
    ///
    /// A `Call` naming a function the compiler does not know has no ports;
    /// [`compile`] reports it as `UnknownOp`.
    pub fn input_ports(&self) -> Vec<Port> {
        match self {
            NodeKind::OnStart
            | NodeKind::OnUpdate
            | NodeKind::OnKeyPressed(_)
            | NodeKind::OnCollision
            | NodeKind::OnInterval(_)
            | NodeKind::Literal(_)
            | NodeKind::SelfEntity
            | NodeKind::GetVar(_) => Vec::new(),
            NodeKind::Branch | NodeKind::WhileLoop => {
                vec![EXEC, data("condition", ValueType::Bool)]
            }
            NodeKind::Sequence => vec![EXEC],
            NodeKind::ForLoop => vec![
                EXEC,
                data("first", ValueType::Number),
                data("last", ValueType::Number),
            ],
            NodeKind::Delay => vec![EXEC, data("frames", ValueType::Number)],
            NodeKind::ToText => vec![data("x", ValueType::Number)],
            NodeKind::Concat => vec![data("a", ValueType::Text), data("b", ValueType::Text)],
            NodeKind::SetVar(_) => vec![
                EXEC,
                Port {
                    name: "value",
                    ty: PortType::Data(None),
                },
            ],
            NodeKind::Call(name) => {
                let Some(spec) = op(name) else {
                    return Vec::new();
                };
                let mut ports = Vec::with_capacity(spec.params.len() + 1);
                if !spec.pure {
                    ports.push(EXEC);
                }
                ports.extend(spec.params.iter().map(|(name, ty)| data(name, *ty)));
                ports
            }
            NodeKind::Add
            | NodeKind::Subtract
            | NodeKind::Multiply
            | NodeKind::Divide
            | NodeKind::Compare(_) => A_B_NUMBER.to_vec(),
            NodeKind::Not => vec![data("x", ValueType::Bool)],
            NodeKind::And | NodeKind::Or => A_B_BOOL.to_vec(),
            NodeKind::Vec3Make => vec![
                data("x", ValueType::Number),
                data("y", ValueType::Number),
                data("z", ValueType::Number),
            ],
            NodeKind::Vec3Split => vec![data("v", ValueType::Vec3)],
        }
    }

    /// This kind's output ports, in display order: flow outputs first.
    pub fn output_ports(&self) -> Vec<Port> {
        match self {
            NodeKind::OnStart
            | NodeKind::OnUpdate
            | NodeKind::OnKeyPressed(_)
            | NodeKind::OnInterval(_) => vec![THEN],
            NodeKind::OnCollision => vec![THEN, data("other", ValueType::Entity)],
            NodeKind::Branch => vec![flow("true"), flow("false")],
            NodeKind::Sequence => vec![flow("then0"), flow("then1"), flow("then2")],
            NodeKind::ForLoop => vec![
                flow("body"),
                flow("completed"),
                data("index", ValueType::Number),
            ],
            NodeKind::WhileLoop => vec![flow("body"), flow("completed")],
            NodeKind::Delay => vec![flow("completed")],
            NodeKind::ToText | NodeKind::Concat => vec![data("out", ValueType::Text)],
            NodeKind::Literal(value) => vec![data("out", ValueType::of(value))],
            NodeKind::SelfEntity => vec![data("out", ValueType::Entity)],
            NodeKind::GetVar(_) => vec![Port {
                name: "out",
                ty: PortType::Data(None),
            }],
            NodeKind::SetVar(_) => vec![THEN],
            NodeKind::Call(name) => {
                let Some(spec) = op(name) else {
                    return Vec::new();
                };
                let mut ports = Vec::with_capacity(2);
                if !spec.pure {
                    ports.push(THEN);
                }
                if let Some(returns) = spec.returns {
                    ports.push(data("out", returns));
                }
                ports
            }
            NodeKind::Add | NodeKind::Subtract | NodeKind::Multiply | NodeKind::Divide => {
                vec![data("out", ValueType::Number)]
            }
            NodeKind::Compare(_) | NodeKind::Not | NodeKind::And | NodeKind::Or => {
                vec![data("out", ValueType::Bool)]
            }
            NodeKind::Vec3Make => vec![data("out", ValueType::Vec3)],
            NodeKind::Vec3Split => vec![
                data("x", ValueType::Number),
                data("y", ValueType::Number),
                data("z", ValueType::Number),
            ],
        }
    }

    fn is_event(&self) -> bool {
        matches!(
            self,
            NodeKind::OnStart
                | NodeKind::OnUpdate
                | NodeKind::OnKeyPressed(_)
                | NodeKind::OnCollision
                | NodeKind::OnInterval(_)
        )
    }
}

/// Whether `port` of a node of this kind is a value that exists only inside
/// the node's own flow -- a collision callback's `other`, a loop body's
/// `index` -- and so may only be read downstream of that flow.
fn flow_scoped(kind: &NodeKind, port: &str) -> bool {
    matches!(
        (kind, port),
        (NodeKind::OnCollision, "other") | (NodeKind::ForLoop, "index")
    )
}

/// The comment every generated file starts with. Public so a tool that
/// writes the file can recognise one it wrote before.
pub const HEADER: &str =
    "// Generated by bsengine-visualscript from a script graph. Edit the graph, not\n\
// this file: the next compile overwrites it. Delete this header to take the\n\
// script over by hand.\n";

/// How many iterations a `WhileLoop` may run in one frame before it is
/// stopped with a logged warning. Blueprint stops a runaway loop at a
/// million; this is lower because a frame here is one V8 tick that every
/// other entity's script waits behind.
pub const LOOP_LIMIT: u32 = 100_000;

const INDENT: &str = "    ";

/// Everything resolved about a graph before any code is written.
struct Compiler<'g> {
    nodes: HashMap<u32, &'g GraphNode>,
    variables: HashMap<&'g str, ValueType>,
    /// `(from node, from port)` -> the node its flow continues at.
    flow: HashMap<(u32, &'g str), u32>,
    /// `(to node, to port)` -> `(from node, from port)`.
    data: HashMap<(u32, &'g str), (u32, &'g str)>,
    /// The nodes whose flow-scoped outputs (see [`flow_scoped`]) the flow
    /// being written is inside of: a collision callback, a loop body.
    scope: Vec<u32>,
    out: String,
}

/// Compiles a graph to a JavaScript script.
///
/// # Errors
///
/// Returns a [`GraphError`] rather than panicking for every malformed graph:
/// no event node, an edge naming a node or a variable or a function that does
/// not exist, a data input left unconnected, a connection between
/// incompatible types, a data cycle, a flow that loops back into itself, or
/// a flow output connected twice. Half-built graphs are the normal case for
/// the node editor, so none of these is exceptional.
pub fn compile(graph: &ScriptGraph) -> Result<String, GraphError> {
    let mut c = Compiler::resolve(graph)?;
    c.emit(graph)?;
    Ok(c.out)
}

impl<'g> Compiler<'g> {
    fn resolve(graph: &'g ScriptGraph) -> Result<Self, GraphError> {
        let mut nodes: HashMap<u32, &GraphNode> = HashMap::new();
        for node in &graph.nodes {
            // A duplicated id is authoring corruption with no error variant
            // of its own; the first definition wins so the result is stable.
            nodes.entry(node.id).or_insert(node);
        }

        let mut variables = HashMap::new();
        for v in &graph.variables {
            if !is_identifier(&v.name) {
                return Err(GraphError::BadVariableName(v.name.clone()));
            }
            variables.insert(v.name.as_str(), ValueType::of(&v.initial));
        }

        for node in nodes.values() {
            match &node.kind {
                NodeKind::Call(name) if op(name).is_none() => {
                    return Err(GraphError::UnknownOp {
                        node: node.id,
                        op: name.clone(),
                    });
                }
                NodeKind::GetVar(name) | NodeKind::SetVar(name)
                    if !variables.contains_key(name.as_str()) =>
                {
                    return Err(GraphError::UnknownVariable {
                        node: node.id,
                        name: name.clone(),
                    });
                }
                _ => {}
            }
        }

        if !nodes.values().any(|n| n.kind.is_event()) {
            return Err(GraphError::NoEvent);
        }

        let mut flow = HashMap::new();
        let mut data = HashMap::new();
        for Edge { from, to } in &graph.edges {
            let src = *nodes.get(&from.0).ok_or(GraphError::UnknownNode(from.0))?;
            let dst = *nodes.get(&to.0).ok_or(GraphError::UnknownNode(to.0))?;
            // An edge naming a port its node does not have is ignored, as the
            // shader graph ignores it: there is no error variant, and the
            // editor never creates one.
            let Some(src_port) = src
                .kind
                .output_ports()
                .into_iter()
                .find(|p| p.name == from.1)
            else {
                continue;
            };
            let Some(dst_port) = dst.kind.input_ports().into_iter().find(|p| p.name == to.1) else {
                continue;
            };
            match (src_port.ty, dst_port.ty) {
                (PortType::Flow, PortType::Flow) => {
                    if flow.insert((src.id, src_port.name), dst.id).is_some() {
                        return Err(GraphError::AmbiguousFlow {
                            node: src.id,
                            port: src_port.name.to_string(),
                        });
                    }
                }
                (PortType::Data(_), PortType::Data(_)) => {
                    // First connection wins, so compilation is deterministic.
                    data.entry((dst.id, dst_port.name))
                        .or_insert((src.id, src_port.name));
                }
                (PortType::Flow, PortType::Data(_)) | (PortType::Data(_), PortType::Flow) => {
                    return Err(GraphError::TypeMismatch {
                        node: dst.id,
                        port: dst_port.name.to_string(),
                        expected: port_type_name(dst_port.ty).to_string(),
                        found: port_type_name(src_port.ty).to_string(),
                    });
                }
            }
        }

        Ok(Self {
            nodes,
            variables,
            flow,
            data,
            scope: Vec::new(),
            out: String::new(),
        })
    }

    fn emit(&mut self, graph: &ScriptGraph) -> Result<(), GraphError> {
        self.out.push_str(HEADER);
        for v in &graph.variables {
            let _ = writeln!(self.out, "let v_{} = {};", v.name, literal(&v.initial));
        }

        let mut ids: Vec<u32> = self.nodes.keys().copied().collect();
        ids.sort_unstable();
        let of_kind =
            |ids: &[u32], nodes: &HashMap<u32, &GraphNode>, f: &dyn Fn(&NodeKind) -> bool| {
                ids.iter()
                    .copied()
                    .filter(|id| f(&nodes[id].kind))
                    .collect::<Vec<u32>>()
            };
        let starts = of_kind(&ids, &self.nodes, &|k| *k == NodeKind::OnStart);
        let collisions = of_kind(&ids, &self.nodes, &|k| *k == NodeKind::OnCollision);
        let updates = of_kind(&ids, &self.nodes, &|k| *k == NodeKind::OnUpdate);
        let keys = of_kind(&ids, &self.nodes, &|k| {
            matches!(k, NodeKind::OnKeyPressed(_))
        });
        let intervals = of_kind(&ids, &self.nodes, &|k| matches!(k, NodeKind::OnInterval(_)));
        // Every impure call with a result gets a slot at the top of the
        // function, so a node downstream of it -- inside a branch arm, or in
        // a collision callback -- can read it without scoping rules deciding
        // whether it may.
        let results: Vec<u32> = ids
            .iter()
            .copied()
            .filter(|id| match &self.nodes[id].kind {
                NodeKind::Call(name) => op(name).is_some_and(|o| !o.pure && o.returns.is_some()),
                _ => false,
            })
            .collect();

        let first_frame = !starts.is_empty() || !collisions.is_empty();
        if first_frame {
            self.out.push_str("let __started = false;\n");
        }
        // One accumulator per interval, at module level so it survives
        // between frames like a variable does.
        for id in &intervals {
            let _ = writeln!(self.out, "let __interval{id} = 0;");
        }
        self.out.push_str("function onUpdate(self) {\n");
        for id in &results {
            let _ = writeln!(self.out, "{INDENT}let n{id};");
        }
        if first_frame {
            let _ = writeln!(self.out, "{INDENT}if (!__started) {{");
            let _ = writeln!(self.out, "{INDENT}{INDENT}__started = true;");
            for id in &starts {
                self.emit_flow(id, "then", 2, &mut Vec::new())?;
            }
            for id in &collisions {
                let _ = writeln!(
                    self.out,
                    "{INDENT}{INDENT}Bsengine.onCollision(self, (other, started) => {{"
                );
                let _ = writeln!(
                    self.out,
                    "{INDENT}{INDENT}{INDENT}if (!started) {{ return; }}"
                );
                self.scope.push(*id);
                self.emit_flow(id, "then", 3, &mut Vec::new())?;
                self.scope.pop();
                let _ = writeln!(self.out, "{INDENT}{INDENT}}});");
            }
            let _ = writeln!(self.out, "{INDENT}}}");
        }
        for id in &updates {
            self.emit_flow(id, "then", 1, &mut Vec::new())?;
        }
        for id in &intervals {
            let NodeKind::OnInterval(seconds) = &self.nodes[id].kind else {
                unreachable!("filtered above");
            };
            let seconds = js_number(*seconds);
            let _ = writeln!(
                self.out,
                "{INDENT}__interval{id} += Bsengine.getDeltaTime();"
            );
            let _ = writeln!(self.out, "{INDENT}if (__interval{id} >= {seconds}) {{");
            let _ = writeln!(self.out, "{INDENT}{INDENT}__interval{id} -= {seconds};");
            self.emit_flow(id, "then", 2, &mut Vec::new())?;
            let _ = writeln!(self.out, "{INDENT}}}");
        }
        for id in &keys {
            let NodeKind::OnKeyPressed(key) = &self.nodes[id].kind else {
                unreachable!("filtered above");
            };
            let _ = writeln!(
                self.out,
                "{INDENT}if (Bsengine.isKeyPressed({})) {{",
                js_string(key)
            );
            self.emit_flow(id, "then", 2, &mut Vec::new())?;
            let _ = writeln!(self.out, "{INDENT}}}");
        }
        self.out.push_str("}\n");
        Ok(())
    }

    /// Writes the statements of the flow leaving `(node, port)`, at `depth`
    /// indents. `stack` is the chain of nodes this flow came through, which
    /// is what turns a flow that leads back into itself into `FlowCycle`
    /// rather than unbounded recursion.
    fn emit_flow(
        &mut self,
        node: &u32,
        port: &'static str,
        depth: usize,
        stack: &mut Vec<u32>,
    ) -> Result<(), GraphError> {
        let Some(next) = self.flow.get(&(*node, port)).copied() else {
            return Ok(());
        };
        if stack.contains(&next) {
            stack.push(next);
            return Err(GraphError::FlowCycle(stack.clone()));
        }
        stack.push(next);
        let pad = INDENT.repeat(depth);
        let kind = self.nodes[&next].kind.clone();
        match &kind {
            NodeKind::Branch => {
                let condition = self.input(next, "condition", ValueType::Bool)?;
                let _ = writeln!(self.out, "{pad}if ({condition}) {{");
                self.emit_flow(&next, "true", depth + 1, stack)?;
                if self.flow.contains_key(&(next, "false")) {
                    let _ = writeln!(self.out, "{pad}}} else {{");
                    self.emit_flow(&next, "false", depth + 1, stack)?;
                }
                let _ = writeln!(self.out, "{pad}}}");
            }
            NodeKind::Sequence => {
                for port in ["then0", "then1", "then2"] {
                    self.emit_flow(&next, port, depth, stack)?;
                }
            }
            NodeKind::SetVar(name) => {
                let ty = self.variables[name.as_str()];
                let value = self.input(next, "value", ty)?;
                let _ = writeln!(self.out, "{pad}v_{name} = {value};");
                self.emit_flow(&next, "then", depth, stack)?;
            }
            NodeKind::Call(name) => {
                let spec = op(name).expect("checked in resolve");
                let call = self.call(next, spec)?;
                if spec.returns.is_some() {
                    let _ = writeln!(self.out, "{pad}n{next} = {call};");
                } else {
                    let _ = writeln!(self.out, "{pad}{call};");
                }
                self.emit_flow(&next, "then", depth, stack)?;
            }
            NodeKind::ForLoop => {
                let first = self.input(next, "first", ValueType::Number)?;
                let last = self.input(next, "last", ValueType::Number)?;
                let _ = writeln!(
                    self.out,
                    "{pad}for (let i{next} = {first}; i{next} <= {last}; i{next}++) {{"
                );
                self.scope.push(next);
                self.emit_flow(&next, "body", depth + 1, stack)?;
                self.scope.pop();
                let _ = writeln!(self.out, "{pad}}}");
                self.emit_flow(&next, "completed", depth, stack)?;
            }
            NodeKind::WhileLoop => {
                let condition = self.input(next, "condition", ValueType::Bool)?;
                let _ = writeln!(self.out, "{pad}let guard{next} = 0;");
                let _ = writeln!(self.out, "{pad}while ({condition}) {{");
                let _ = writeln!(
                    self.out,
                    "{pad}{INDENT}if (++guard{next} > {LOOP_LIMIT}) {{ Bsengine.log(\"script graph: node {next}'s loop ran {LOOP_LIMIT} times in one frame and was stopped\"); break; }}"
                );
                self.emit_flow(&next, "body", depth + 1, stack)?;
                let _ = writeln!(self.out, "{pad}}}");
                self.emit_flow(&next, "completed", depth, stack)?;
            }
            NodeKind::Delay => {
                let frames = self.input(next, "frames", ValueType::Number)?;
                let _ = writeln!(self.out, "{pad}Bsengine.setTimeout(() => {{");
                self.emit_flow(&next, "completed", depth + 1, stack)?;
                let _ = writeln!(self.out, "{pad}}}, {frames});");
            }
            // Only kinds with an `"exec"` input can be a flow's target, and
            // these are all of them.
            _ => unreachable!("a flow edge only ever targets a node with an exec port"),
        }
        stack.pop();
        Ok(())
    }

    /// `Bsengine.<name>(<args>)`, each argument the expression on its port.
    fn call(&self, node: u32, spec: &OpSpec) -> Result<String, GraphError> {
        let mut args = Vec::with_capacity(spec.params.len());
        for (param, ty) in spec.params {
            args.push(self.input(node, param, *ty)?);
        }
        Ok(format!("Bsengine.{}({})", spec.name, args.join(", ")))
    }

    /// The expression connected to `node`'s data input `port`, checked
    /// against `expected`.
    fn input(
        &self,
        node: u32,
        port: &'static str,
        expected: ValueType,
    ) -> Result<String, GraphError> {
        let Some((src, src_port)) = self.data.get(&(node, port)).copied() else {
            return Err(GraphError::MissingInput {
                node,
                port: port.to_string(),
            });
        };
        self.check_scope(node, port, src, src_port)?;
        let (text, found) = self.expr(src, src_port, &mut Vec::new())?;
        if !expected.accepts(found) {
            return Err(GraphError::TypeMismatch {
                node,
                port: port.to_string(),
                expected: expected.name().to_string(),
                found: found.name().to_string(),
            });
        }
        Ok(text)
    }

    /// Refuses a read of a flow-scoped value from outside its flow.
    fn check_scope(
        &self,
        node: u32,
        port: &str,
        src: u32,
        src_port: &str,
    ) -> Result<(), GraphError> {
        if flow_scoped(&self.nodes[&src].kind, src_port) && !self.scope.contains(&src) {
            return Err(GraphError::OutOfScope {
                node,
                port: port.to_string(),
                source: src,
            });
        }
        Ok(())
    }

    /// The expression for `node`'s data output `port`, and its type.
    fn expr(
        &self,
        node: u32,
        port: &'g str,
        stack: &mut Vec<u32>,
    ) -> Result<(String, ValueType), GraphError> {
        if stack.contains(&node) {
            stack.push(node);
            return Err(GraphError::Cycle(stack.clone()));
        }
        stack.push(node);
        let kind = &self.nodes[&node].kind;
        let arg = |name: &'static str,
                   ty: ValueType,
                   stack: &mut Vec<u32>|
         -> Result<String, GraphError> {
            let Some((src, src_port)) = self.data.get(&(node, name)).copied() else {
                return Err(GraphError::MissingInput {
                    node,
                    port: name.to_string(),
                });
            };
            self.check_scope(node, name, src, src_port)?;
            let (text, found) = self.expr(src, src_port, stack)?;
            if !ty.accepts(found) {
                return Err(GraphError::TypeMismatch {
                    node,
                    port: name.to_string(),
                    expected: ty.name().to_string(),
                    found: found.name().to_string(),
                });
            }
            Ok(text)
        };
        let result = match kind {
            NodeKind::Literal(value) => (literal(value), ValueType::of(value)),
            NodeKind::SelfEntity => ("self".to_string(), ValueType::Entity),
            NodeKind::GetVar(name) => (format!("v_{name}"), self.variables[name.as_str()]),
            NodeKind::OnCollision => ("other".to_string(), ValueType::Entity),
            NodeKind::Call(name) => {
                let spec = op(name).expect("checked in resolve");
                let returns = spec
                    .returns
                    .expect("an edge from a result port implies one");
                if spec.pure {
                    let mut args = Vec::with_capacity(spec.params.len());
                    for (param, ty) in spec.params {
                        args.push(arg(param, *ty, stack)?);
                    }
                    (
                        format!("Bsengine.{}({})", spec.name, args.join(", ")),
                        returns,
                    )
                } else {
                    (format!("n{node}"), returns)
                }
            }
            NodeKind::Add | NodeKind::Subtract | NodeKind::Multiply | NodeKind::Divide => {
                let a = arg("a", ValueType::Number, stack)?;
                let b = arg("b", ValueType::Number, stack)?;
                let symbol = match kind {
                    NodeKind::Add => "+",
                    NodeKind::Subtract => "-",
                    NodeKind::Multiply => "*",
                    _ => "/",
                };
                (format!("({a} {symbol} {b})"), ValueType::Number)
            }
            NodeKind::Compare(cmp) => {
                let a = arg("a", ValueType::Number, stack)?;
                let b = arg("b", ValueType::Number, stack)?;
                let symbol = match cmp {
                    CompareOp::Less => "<",
                    CompareOp::LessOrEqual => "<=",
                    CompareOp::Greater => ">",
                    CompareOp::GreaterOrEqual => ">=",
                    CompareOp::Equal => "===",
                    CompareOp::NotEqual => "!==",
                };
                (format!("({a} {symbol} {b})"), ValueType::Bool)
            }
            NodeKind::Not => {
                let x = arg("x", ValueType::Bool, stack)?;
                (format!("(!{x})"), ValueType::Bool)
            }
            NodeKind::And | NodeKind::Or => {
                let a = arg("a", ValueType::Bool, stack)?;
                let b = arg("b", ValueType::Bool, stack)?;
                let symbol = if *kind == NodeKind::And { "&&" } else { "||" };
                (format!("({a} {symbol} {b})"), ValueType::Bool)
            }
            NodeKind::Vec3Make => {
                let x = arg("x", ValueType::Number, stack)?;
                let y = arg("y", ValueType::Number, stack)?;
                let z = arg("z", ValueType::Number, stack)?;
                (format!("Bsengine.vec3({x}, {y}, {z})"), ValueType::Vec3)
            }
            NodeKind::Vec3Split => {
                let v = arg("v", ValueType::Vec3, stack)?;
                (format!("{v}.{port}"), ValueType::Number)
            }
            NodeKind::ForLoop => (format!("i{node}"), ValueType::Number),
            NodeKind::ToText => {
                let x = arg("x", ValueType::Number, stack)?;
                (format!("String({x})"), ValueType::Text)
            }
            NodeKind::Concat => {
                let a = arg("a", ValueType::Text, stack)?;
                let b = arg("b", ValueType::Text, stack)?;
                (format!("({a} + {b})"), ValueType::Text)
            }
            // These have no data outputs, so no data edge can start at them.
            NodeKind::OnStart
            | NodeKind::OnUpdate
            | NodeKind::OnKeyPressed(_)
            | NodeKind::OnInterval(_)
            | NodeKind::Branch
            | NodeKind::Sequence
            | NodeKind::WhileLoop
            | NodeKind::Delay
            | NodeKind::SetVar(_) => {
                unreachable!("a data edge only ever starts at a node with a data output")
            }
        };
        stack.pop();
        Ok(result)
    }
}

fn port_type_name(ty: PortType) -> &'static str {
    match ty {
        PortType::Flow => "a flow",
        PortType::Data(Some(t)) => t.name(),
        PortType::Data(None) => "a value",
    }
}

/// A JavaScript identifier fragment: what `v_<name>` needs `<name>` to be.
fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A `Value` as JavaScript source.
fn literal(value: &Value) -> String {
    match value {
        Value::Number(n) => js_number(*n),
        Value::Bool(b) => b.to_string(),
        Value::Text(s) => js_string(s),
    }
}

/// A number spelled the way a person would: `2`, not `2.0`; `0.1`, not
/// `0.10000000000000001`. Rust's shortest round-trip formatting is the latter
/// guarantee; the integer case is the former.
fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e15 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// A double-quoted JavaScript string literal.
fn js_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph::Variable;

    fn node(id: u32, kind: NodeKind) -> GraphNode {
        GraphNode {
            id,
            kind,
            position: [0.0, 0.0],
        }
    }

    fn edge(from: (u32, &str), to: (u32, &str)) -> Edge {
        Edge {
            from: (from.0, from.1.to_string()),
            to: (to.0, to.1.to_string()),
        }
    }

    fn number(n: f64) -> NodeKind {
        NodeKind::Literal(Value::Number(n))
    }

    fn text(s: &str) -> NodeKind {
        NodeKind::Literal(Value::Text(s.to_string()))
    }

    fn call(name: &str) -> NodeKind {
        NodeKind::Call(name.to_string())
    }

    /// `OnUpdate -> addPosition(self, vec3(0, speed * dt, 0))`: a variable,
    /// a pure call, arithmetic, a vector and an impure call, with the literal
    /// `0` read twice.
    fn bob() -> ScriptGraph {
        ScriptGraph {
            nodes: vec![
                node(0, NodeKind::OnUpdate),
                node(1, call("addPosition")),
                node(2, NodeKind::SelfEntity),
                node(3, NodeKind::Vec3Make),
                node(4, number(0.0)),
                node(5, NodeKind::Multiply),
                node(6, NodeKind::GetVar("speed".to_string())),
                node(7, call("getDeltaTime")),
            ],
            edges: vec![
                edge((0, "then"), (1, "exec")),
                edge((2, "out"), (1, "entity")),
                edge((3, "out"), (1, "delta")),
                edge((4, "out"), (3, "x")),
                edge((5, "out"), (3, "y")),
                edge((4, "out"), (3, "z")),
                edge((6, "out"), (5, "a")),
                edge((7, "out"), (5, "b")),
            ],
            variables: vec![Variable {
                name: "speed".to_string(),
                initial: Value::Number(2.0),
            }],
        }
    }

    fn body(js: &str) -> &str {
        js.strip_prefix(HEADER)
            .expect("every compile starts with the header")
    }

    /// The whole output, pinned: this is the file a person will read, so its
    /// shape is part of the contract, not an implementation detail.
    #[test]
    fn a_flow_compiles_to_a_readable_on_update() {
        let js = compile(&bob()).expect("the demo flow compiles");
        assert_eq!(
            body(&js),
            "let v_speed = 2;\n\
             function onUpdate(self) {\n\
             \x20   Bsengine.addPosition(self, Bsengine.vec3(0, (v_speed * Bsengine.getDeltaTime()), 0));\n\
             }\n"
        );
    }

    /// `Branch` is an `if`/`else`; `Sequence` runs its arms in order; a
    /// `SetVar` assigns. The false arm is only written when connected.
    #[test]
    fn branch_and_sequence_become_if_and_ordered_statements() {
        let graph = ScriptGraph {
            nodes: vec![
                node(0, NodeKind::OnUpdate),
                node(1, NodeKind::Branch),
                node(2, call("isKeyDown")),
                node(3, text("Space")),
                node(4, NodeKind::Sequence),
                node(5, call("log")),
                node(6, text("up")),
                node(7, NodeKind::SetVar("count".to_string())),
                node(8, NodeKind::Add),
                node(9, NodeKind::GetVar("count".to_string())),
                node(10, number(1.0)),
                node(11, call("log")),
                node(12, text("down")),
            ],
            edges: vec![
                edge((0, "then"), (1, "exec")),
                edge((2, "out"), (1, "condition")),
                edge((3, "out"), (2, "key")),
                edge((1, "true"), (4, "exec")),
                edge((4, "then0"), (5, "exec")),
                edge((6, "out"), (5, "message")),
                edge((4, "then2"), (7, "exec")),
                edge((8, "out"), (7, "value")),
                edge((9, "out"), (8, "a")),
                edge((10, "out"), (8, "b")),
                edge((1, "false"), (11, "exec")),
                edge((12, "out"), (11, "message")),
            ],
            variables: vec![Variable {
                name: "count".to_string(),
                initial: Value::Number(0.0),
            }],
        };
        let js = compile(&graph).expect("compiles");
        assert_eq!(
            body(&js),
            "let v_count = 0;\n\
             function onUpdate(self) {\n\
             \x20   if (Bsengine.isKeyDown(\"Space\")) {\n\
             \x20       Bsengine.log(\"up\");\n\
             \x20       v_count = (v_count + 1);\n\
             \x20   } else {\n\
             \x20       Bsengine.log(\"down\");\n\
             \x20   }\n\
             }\n"
        );

        let mut without_else = graph.clone();
        without_else
            .edges
            .retain(|e| e.from != (1, "false".to_string()));
        let js = compile(&without_else).expect("compiles");
        assert!(
            !js.contains("else"),
            "an unconnected false arm is not written: {js}"
        );
    }

    /// `OnStart` runs behind a first-frame guard, `OnCollision` registers
    /// its callback there with `self` in hand, and `OnKeyPressed` is a
    /// per-frame check -- all inside the one `onUpdate` the runtime calls.
    #[test]
    fn every_event_is_dispatched_from_on_update() {
        let graph = ScriptGraph {
            nodes: vec![
                node(0, NodeKind::OnStart),
                node(1, call("log")),
                node(2, text("started")),
                node(3, NodeKind::OnCollision),
                node(4, call("destroy")),
                node(5, NodeKind::OnKeyPressed("Escape".to_string())),
                node(6, call("quit")),
                node(7, call("playSound")),
                node(8, text("assets/sounds/hit.wav")),
            ],
            edges: vec![
                edge((0, "then"), (1, "exec")),
                edge((2, "out"), (1, "message")),
                edge((3, "then"), (4, "exec")),
                edge((3, "other"), (4, "entity")),
                edge((4, "then"), (7, "exec")),
                edge((8, "out"), (7, "path")),
                edge((5, "then"), (6, "exec")),
            ],
            variables: Vec::new(),
        };
        let js = compile(&graph).expect("compiles");
        assert_eq!(
            body(&js),
            "let __started = false;\n\
             function onUpdate(self) {\n\
             \x20   let n7;\n\
             \x20   if (!__started) {\n\
             \x20       __started = true;\n\
             \x20       Bsengine.log(\"started\");\n\
             \x20       Bsengine.onCollision(self, (other, started) => {\n\
             \x20           if (!started) { return; }\n\
             \x20           Bsengine.destroy(other);\n\
             \x20           n7 = Bsengine.playSound(\"assets/sounds/hit.wav\");\n\
             \x20       });\n\
             \x20   }\n\
             \x20   if (Bsengine.isKeyPressed(\"Escape\")) {\n\
             \x20       Bsengine.quit();\n\
             \x20   }\n\
             }\n"
        );
    }

    /// Comparisons, logic and vector splitting, with an entity name given
    /// as a `Text` literal where an `Entity` is expected -- the one coercion
    /// the type check allows.
    #[test]
    fn comparisons_logic_and_vector_parts_compile_to_expressions() {
        let graph = ScriptGraph {
            nodes: vec![
                node(0, NodeKind::OnUpdate),
                node(1, NodeKind::Branch),
                node(2, NodeKind::And),
                node(3, NodeKind::Compare(CompareOp::Greater)),
                node(4, NodeKind::Vec3Split),
                node(5, call("getPosition")),
                node(6, text("Enemy")),
                node(7, number(2.5)),
                node(8, NodeKind::Not),
                node(9, call("isPaused")),
                node(10, call("setVisible")),
                node(11, NodeKind::SelfEntity),
                node(12, NodeKind::Literal(Value::Bool(false))),
            ],
            edges: vec![
                edge((0, "then"), (1, "exec")),
                edge((2, "out"), (1, "condition")),
                edge((3, "out"), (2, "a")),
                edge((8, "out"), (2, "b")),
                edge((4, "y"), (3, "a")),
                edge((7, "out"), (3, "b")),
                edge((5, "out"), (4, "v")),
                edge((6, "out"), (5, "entity")),
                edge((9, "out"), (8, "x")),
                edge((1, "true"), (10, "exec")),
                edge((11, "out"), (10, "entity")),
                edge((12, "out"), (10, "visible")),
            ],
            variables: Vec::new(),
        };
        let js = compile(&graph).expect("compiles");
        assert!(
            js.contains(
                "if (((Bsengine.getPosition(\"Enemy\").y > 2.5) && (!Bsengine.isPaused()))) {"
            ),
            "{js}"
        );
        assert!(js.contains("Bsengine.setVisible(self, false);"), "{js}");
    }

    #[test]
    fn a_graph_without_an_event_does_not_compile() {
        let graph = ScriptGraph {
            nodes: vec![node(0, call("quit"))],
            ..Default::default()
        };
        assert_eq!(compile(&graph), Err(GraphError::NoEvent));
    }

    /// Nodes nothing reaches are not written: a stray literal, a call no
    /// flow leads to. What is written is exactly what runs.
    #[test]
    fn unreached_nodes_are_not_emitted() {
        let mut graph = bob();
        graph.nodes.push(node(20, call("quit")));
        graph.nodes.push(node(21, text("nobody reads this")));
        let js = compile(&graph).expect("compiles");
        assert!(!js.contains("quit"), "{js}");
        assert!(!js.contains("nobody"), "{js}");
    }

    #[test]
    fn every_authoring_mistake_is_a_named_error() {
        let mut g = bob();
        g.edges.push(edge((99, "out"), (3, "x")));
        assert_eq!(compile(&g), Err(GraphError::UnknownNode(99)));

        let mut g = bob();
        g.nodes[7] = node(7, call("fly"));
        assert_eq!(
            compile(&g),
            Err(GraphError::UnknownOp {
                node: 7,
                op: "fly".to_string()
            })
        );

        let mut g = bob();
        g.nodes[6] = node(6, NodeKind::GetVar("velocity".to_string()));
        assert_eq!(
            compile(&g),
            Err(GraphError::UnknownVariable {
                node: 6,
                name: "velocity".to_string()
            })
        );

        let mut g = bob();
        g.variables[0].name = "top speed".to_string();
        assert_eq!(
            compile(&g),
            Err(GraphError::BadVariableName("top speed".to_string()))
        );

        let mut g = bob();
        g.edges.retain(|e| e.to != (3, "y".to_string()));
        assert_eq!(
            compile(&g),
            Err(GraphError::MissingInput {
                node: 3,
                port: "y".to_string()
            })
        );

        // A Bool where a Number is needed.
        let mut g = bob();
        g.nodes[4] = node(4, NodeKind::Literal(Value::Bool(true)));
        assert_eq!(
            compile(&g),
            Err(GraphError::TypeMismatch {
                node: 3,
                port: "x".to_string(),
                expected: "Number".to_string(),
                found: "Bool".to_string()
            })
        );

        // A flow output into a data input.
        let mut g = bob();
        g.edges.push(edge((0, "then"), (5, "a")));
        assert_eq!(
            compile(&g),
            Err(GraphError::TypeMismatch {
                node: 5,
                port: "a".to_string(),
                expected: "Number".to_string(),
                found: "a flow".to_string()
            })
        );

        // A data cycle: the multiply feeds itself through an add.
        let mut g = bob();
        g.nodes.push(node(30, NodeKind::Add));
        g.edges.retain(|e| e.to != (5, "a".to_string()));
        g.edges.push(edge((30, "out"), (5, "a")));
        g.edges.push(edge((5, "out"), (30, "a")));
        g.edges.push(edge((4, "out"), (30, "b")));
        assert!(
            matches!(compile(&g), Err(GraphError::Cycle(ids)) if ids.contains(&5) && ids.contains(&30)),
            "{:?}",
            compile(&g)
        );

        // A flow that loops: the call's `then` leads back to itself.
        let mut g = bob();
        g.edges.push(edge((1, "then"), (1, "exec")));
        assert!(
            matches!(compile(&g), Err(GraphError::FlowCycle(ids)) if ids.contains(&1)),
            "{:?}",
            compile(&g)
        );

        // One flow output connected twice.
        let mut g = bob();
        g.nodes.push(node(40, call("quit")));
        g.edges.push(edge((0, "then"), (40, "exec")));
        assert_eq!(
            compile(&g),
            Err(GraphError::AmbiguousFlow {
                node: 0,
                port: "then".to_string()
            })
        );
    }

    /// The type check on the ports a *flow* node reads -- a Branch's
    /// condition, a SetVar's value, an impure call's arguments -- which is a
    /// different code path from the one an expression's inputs take. A
    /// mutation that disabled only this path survived every other test in
    /// this file; these three are what it fails.
    #[test]
    fn flow_nodes_type_check_their_inputs_too() {
        let mut g = bob();
        g.nodes.push(node(20, NodeKind::Branch));
        g.nodes.push(node(21, number(1.0)));
        g.edges.retain(|e| e.from != (0, "then".to_string()));
        g.edges.push(edge((0, "then"), (20, "exec")));
        g.edges.push(edge((21, "out"), (20, "condition")));
        assert_eq!(
            compile(&g),
            Err(GraphError::TypeMismatch {
                node: 20,
                port: "condition".to_string(),
                expected: "Bool".to_string(),
                found: "Number".to_string()
            })
        );

        let mut g = bob();
        g.nodes
            .push(node(20, NodeKind::SetVar("speed".to_string())));
        g.nodes.push(node(21, text("fast")));
        g.edges.push(edge((1, "then"), (20, "exec")));
        g.edges.push(edge((21, "out"), (20, "value")));
        assert_eq!(
            compile(&g),
            Err(GraphError::TypeMismatch {
                node: 20,
                port: "value".to_string(),
                expected: "Number".to_string(),
                found: "Text".to_string()
            })
        );

        let mut g = bob();
        g.nodes.push(node(20, call("setVisible")));
        g.nodes.push(node(21, number(1.0)));
        g.edges.push(edge((1, "then"), (20, "exec")));
        g.edges.push(edge((2, "out"), (20, "entity")));
        g.edges.push(edge((21, "out"), (20, "visible")));
        assert_eq!(
            compile(&g),
            Err(GraphError::TypeMismatch {
                node: 20,
                port: "visible".to_string(),
                expected: "Bool".to_string(),
                found: "Number".to_string()
            })
        );
    }

    /// `ForLoop` is a `for` whose index is readable in its body; `WhileLoop`
    /// is a `while` with the runaway guard; `Delay` continues inside a
    /// `setTimeout` callback; `OnInterval` is a module-level accumulator on
    /// the frame's delta. `ToText` and `Concat` build the HUD string. The
    /// whole output is pinned, as the other flows are.
    #[test]
    fn loops_delay_and_interval_compile_to_their_statements() {
        let graph = ScriptGraph {
            nodes: vec![
                node(0, NodeKind::OnStart),
                node(1, NodeKind::ForLoop),
                node(2, number(1.0)),
                node(3, number(3.0)),
                node(4, call("setHudText")),
                node(5, text("count")),
                node(6, NodeKind::Concat),
                node(7, text("i=")),
                node(8, NodeKind::ToText),
                node(9, NodeKind::Delay),
                node(10, number(2.0)),
                node(11, call("log")),
                node(12, text("done")),
                node(13, NodeKind::OnInterval(0.5)),
                node(14, NodeKind::WhileLoop),
                node(15, NodeKind::Compare(CompareOp::Less)),
                node(16, NodeKind::GetVar("n".to_string())),
                node(17, number(10.0)),
                node(18, NodeKind::SetVar("n".to_string())),
                node(19, NodeKind::Add),
                node(20, number(1.0)),
                node(21, call("quit")),
            ],
            edges: vec![
                edge((0, "then"), (1, "exec")),
                edge((2, "out"), (1, "first")),
                edge((3, "out"), (1, "last")),
                edge((1, "body"), (4, "exec")),
                edge((5, "out"), (4, "id")),
                edge((6, "out"), (4, "text")),
                edge((7, "out"), (6, "a")),
                edge((8, "out"), (6, "b")),
                edge((1, "index"), (8, "x")),
                edge((1, "completed"), (9, "exec")),
                edge((10, "out"), (9, "frames")),
                edge((9, "completed"), (11, "exec")),
                edge((12, "out"), (11, "message")),
                edge((13, "then"), (14, "exec")),
                edge((15, "out"), (14, "condition")),
                edge((16, "out"), (15, "a")),
                edge((17, "out"), (15, "b")),
                edge((14, "body"), (18, "exec")),
                edge((19, "out"), (18, "value")),
                edge((16, "out"), (19, "a")),
                edge((20, "out"), (19, "b")),
                edge((14, "completed"), (21, "exec")),
            ],
            variables: vec![Variable {
                name: "n".to_string(),
                initial: Value::Number(0.0),
            }],
        };
        let js = compile(&graph).expect("compiles");
        assert_eq!(
            body(&js),
            "let v_n = 0;\n\
             let __started = false;\n\
             let __interval13 = 0;\n\
             function onUpdate(self) {\n\
             \x20   if (!__started) {\n\
             \x20       __started = true;\n\
             \x20       for (let i1 = 1; i1 <= 3; i1++) {\n\
             \x20           Bsengine.setHudText(\"count\", (\"i=\" + String(i1)));\n\
             \x20       }\n\
             \x20       Bsengine.setTimeout(() => {\n\
             \x20           Bsengine.log(\"done\");\n\
             \x20       }, 2);\n\
             \x20   }\n\
             \x20   __interval13 += Bsengine.getDeltaTime();\n\
             \x20   if (__interval13 >= 0.5) {\n\
             \x20       __interval13 -= 0.5;\n\
             \x20       let guard14 = 0;\n\
             \x20       while ((v_n < 10)) {\n\
             \x20           if (++guard14 > 100000) { Bsengine.log(\"script graph: node 14's loop ran 100000 times in one frame and was stopped\"); break; }\n\
             \x20           v_n = (v_n + 1);\n\
             \x20       }\n\
             \x20       Bsengine.quit();\n\
             \x20   }\n\
             }\n"
        );
    }

    /// A loop's `index` and a collision's `other` exist only inside the
    /// `for` / the callback the compiler writes; read from anywhere else
    /// they compile to a name JavaScript would throw on. The compiler
    /// refuses those reads by name, on both input paths (a flow node's
    /// argument and an expression's operand), and allows the same reads
    /// from inside the flow -- including from a loop nested in the callback.
    #[test]
    fn flow_scoped_values_cannot_be_read_outside_their_flow() {
        // The index read by a call *after* the loop completed.
        let mut g = bob();
        g.nodes.push(node(20, NodeKind::ForLoop));
        g.nodes.push(node(21, number(0.0)));
        g.nodes.push(node(22, call("log")));
        g.nodes.push(node(23, NodeKind::ToText));
        g.edges.retain(|e| e.from != (0, "then".to_string()));
        g.edges.push(edge((0, "then"), (20, "exec")));
        g.edges.push(edge((21, "out"), (20, "first")));
        g.edges.push(edge((21, "out"), (20, "last")));
        g.edges.push(edge((20, "completed"), (22, "exec")));
        g.edges.push(edge((23, "out"), (22, "message")));
        g.edges.push(edge((20, "index"), (23, "x")));
        assert_eq!(
            compile(&g),
            Err(GraphError::OutOfScope {
                node: 23,
                port: "x".to_string(),
                source: 20
            })
        );
        // The same read from the body compiles.
        g.edges.retain(|e| e.from != (20, "completed".to_string()));
        g.edges.push(edge((20, "body"), (22, "exec")));
        let js = compile(&g).expect("reading the index inside the body is fine");
        assert!(js.contains("Bsengine.log(String(i20));"), "{js}");

        // A collision's `other` read by the per-frame flow: the error is on
        // the flow node's own argument, the first input path.
        let mut g = bob();
        g.nodes.push(node(20, NodeKind::OnCollision));
        g.nodes.push(node(21, call("destroy")));
        g.edges.push(edge((1, "then"), (21, "exec")));
        g.edges.push(edge((20, "other"), (21, "entity")));
        assert_eq!(
            compile(&g),
            Err(GraphError::OutOfScope {
                node: 21,
                port: "entity".to_string(),
                source: 20
            })
        );

        // ... and through an expression, the second path: `other` as an
        // operand of `distanceTo` feeding the update flow's Branch.
        let mut g = bob();
        g.nodes.push(node(20, NodeKind::OnCollision));
        g.nodes.push(node(21, NodeKind::Branch));
        g.nodes.push(node(22, NodeKind::Compare(CompareOp::Less)));
        g.nodes.push(node(23, call("distanceTo")));
        g.nodes.push(node(24, number(1.0)));
        g.edges.push(edge((1, "then"), (21, "exec")));
        g.edges.push(edge((22, "out"), (21, "condition")));
        g.edges.push(edge((23, "out"), (22, "a")));
        g.edges.push(edge((24, "out"), (22, "b")));
        g.edges.push(edge((2, "out"), (23, "a")));
        g.edges.push(edge((20, "other"), (23, "b")));
        assert_eq!(
            compile(&g),
            Err(GraphError::OutOfScope {
                node: 23,
                port: "b".to_string(),
                source: 20
            })
        );

        // Inside the callback, even from a loop nested in it, `other` is in
        // scope, and so is that loop's index.
        let mut g = bob();
        g.nodes.push(node(20, NodeKind::OnCollision));
        g.nodes.push(node(21, NodeKind::ForLoop));
        g.nodes.push(node(22, number(1.0)));
        g.nodes.push(node(23, call("damageShield")));
        g.edges.push(edge((20, "then"), (21, "exec")));
        g.edges.push(edge((22, "out"), (21, "first")));
        g.edges.push(edge((22, "out"), (21, "last")));
        g.edges.push(edge((21, "body"), (23, "exec")));
        g.edges.push(edge((20, "other"), (23, "entity")));
        g.edges.push(edge((21, "index"), (23, "amount")));
        let js = compile(&g).expect("a nested read inside the callback compiles");
        assert!(js.contains("Bsengine.damageShield(other, i21);"), "{js}");
    }

    /// The port tables the editor draws from agree with what the compiler
    /// does: an impure call has flow ports and a pure one does not, a
    /// variable's ports take the graph's word for their type, and an unknown
    /// call has no ports at all rather than a guess.
    #[test]
    fn port_tables_follow_the_op_and_the_kind() {
        let add = NodeKind::Call("addPosition".to_string());
        assert_eq!(add.input_ports()[0], EXEC);
        assert_eq!(add.input_ports()[1], data("entity", ValueType::Entity));
        assert_eq!(add.output_ports(), vec![THEN]);

        let dt = NodeKind::Call("getDeltaTime".to_string());
        assert!(dt.input_ports().is_empty());
        assert_eq!(dt.output_ports(), vec![data("out", ValueType::Number)]);

        let sound = NodeKind::Call("playSound".to_string());
        assert_eq!(
            sound.output_ports(),
            vec![THEN, data("out", ValueType::Number)]
        );

        assert!(NodeKind::Call("fly".to_string()).input_ports().is_empty());
        assert_eq!(
            NodeKind::GetVar("x".to_string()).output_ports()[0].ty,
            PortType::Data(None)
        );
        assert_eq!(
            NodeKind::OnCollision.output_ports(),
            vec![THEN, data("other", ValueType::Entity)]
        );
        assert_eq!(NodeKind::Vec3Split.output_ports().len(), 3);
        assert_eq!(
            NodeKind::ForLoop.output_ports(),
            vec![
                flow("body"),
                flow("completed"),
                data("index", ValueType::Number)
            ]
        );
        assert_eq!(NodeKind::ForLoop.input_ports().len(), 3);
        assert_eq!(
            NodeKind::WhileLoop.input_ports()[1].ty,
            PortType::Data(Some(ValueType::Bool))
        );
        assert_eq!(NodeKind::Delay.output_ports(), vec![flow("completed")]);
        assert_eq!(NodeKind::OnInterval(1.0).output_ports(), vec![THEN]);
        assert!(NodeKind::OnInterval(1.0).input_ports().is_empty());
        assert_eq!(
            NodeKind::Concat.output_ports(),
            vec![data("out", ValueType::Text)]
        );
    }

    #[test]
    fn literals_are_spelled_as_a_person_would_write_them() {
        assert_eq!(js_number(2.0), "2");
        assert_eq!(js_number(-1.0), "-1");
        assert_eq!(js_number(0.1), "0.1");
        assert_eq!(js_number(2.5), "2.5");
        assert_eq!(js_string("a\"b\\c\nd"), "\"a\\\"b\\\\c\\nd\"");
        assert!(ValueType::Entity.accepts(ValueType::Text));
        assert!(ValueType::Text.accepts(ValueType::Entity));
        assert!(!ValueType::Number.accepts(ValueType::Bool));
    }
}
