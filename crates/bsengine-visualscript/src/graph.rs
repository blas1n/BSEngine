//! The script graph data model.
//!
//! Deliberately free of ECS, runtime and UI types, like `bsengine-shadergraph`'s:
//! the compiler is a pure function from this model to JavaScript text, which is
//! what lets it be unit-tested without an isolate and reused from both the
//! editor UI and asset tooling.

use serde::{Deserialize, Serialize};

/// A literal value a node can hold or a variable can start as.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Value {
    /// A number; JavaScript has one numeric type and so does this.
    Number(f64),
    /// `true` or `false`.
    Bool(bool),
    /// A string -- also how an entity is named.
    Text(String),
}

/// How two numbers are compared by [`NodeKind::Compare`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CompareOp {
    /// `a < b`
    Less,
    /// `a <= b`
    LessOrEqual,
    /// `a > b`
    Greater,
    /// `a >= b`
    GreaterOrEqual,
    /// `a == b`
    Equal,
    /// `a != b`
    NotEqual,
}

/// One node's operation and its inline parameters.
///
/// Each variant's doc names its ports. Flow ports are spelled `"exec"` (the
/// one flow input a node that runs can have) and `"then"` (the flow output
/// most such nodes have); the variants with other flow outputs say so.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// Runs on the entity's first frame, before that frame's `OnUpdate`.
    ///
    /// Flow output: `"then"`.
    OnStart,
    /// Runs every frame.
    ///
    /// Flow output: `"then"`.
    OnUpdate,
    /// Runs on the frame the named key goes down -- `Bsengine.isKeyPressed`,
    /// once per press, not while held.
    ///
    /// Flow output: `"then"`.
    OnKeyPressed(String),
    /// Runs when this entity starts touching another.
    ///
    /// Flow output: `"then"`. Data output: `"other"` (the other entity's
    /// name), usable only downstream of this node's flow.
    OnCollision,
    /// Takes one of two flows depending on a condition.
    ///
    /// Flow input: `"exec"`. Data input: `"condition"` (`Bool`). Flow
    /// outputs: `"true"` and `"false"`.
    Branch,
    /// Runs up to three flows one after another.
    ///
    /// Flow input: `"exec"`. Flow outputs: `"then0"`, `"then1"`, `"then2"`,
    /// in that order; any may be left unconnected.
    Sequence,
    /// A literal. No inputs; data output `"out"` of the value's type.
    Literal(Value),
    /// The name of the entity this script runs on, `self`.
    ///
    /// Data output: `"out"` (`Entity`).
    SelfEntity,
    /// Reads a graph variable (see [`ScriptGraph::variables`]).
    ///
    /// Data output: `"out"` of the variable's type.
    GetVar(String),
    /// Writes a graph variable.
    ///
    /// Flow input: `"exec"`. Data input: `"value"` of the variable's type.
    /// Flow output: `"then"`.
    SetVar(String),
    /// Calls an engine function by its `Bsengine.<name>` -- one of
    /// [`crate::ops::OPS`], which fixes its parameters and result.
    ///
    /// A pure call (a getter) has only data ports: one input per parameter
    /// and `"out"` for the result. An impure call (anything that changes the
    /// world) also has flow input `"exec"` and flow output `"then"`, and its
    /// result, if any, is on `"out"` for nodes downstream of that flow.
    Call(String),
    /// `a + b` on two `Number`s. Data inputs `"a"`, `"b"`; output `"out"`.
    Add,
    /// `a - b`. Ports as [`NodeKind::Add`].
    Subtract,
    /// `a * b`. Ports as [`NodeKind::Add`].
    Multiply,
    /// `a / b`. Ports as [`NodeKind::Add`].
    Divide,
    /// Compares two `Number`s. Data inputs `"a"`, `"b"`; output `"out"`
    /// (`Bool`).
    Compare(CompareOp),
    /// `!x`. Data input `"x"` (`Bool`); output `"out"` (`Bool`).
    Not,
    /// `a && b`. Data inputs `"a"`, `"b"` (`Bool`); output `"out"` (`Bool`).
    And,
    /// `a || b`. Ports as [`NodeKind::And`].
    Or,
    /// Builds a `Vec3` from three `Number`s: inputs `"x"`, `"y"`, `"z"`;
    /// output `"out"`.
    Vec3Make,
    /// Takes a `Vec3` apart: input `"v"`; outputs `"x"`, `"y"`, `"z"`
    /// (`Number`).
    Vec3Split,
    /// Runs `body` once per index from `first` to `last` inclusive, then
    /// `completed` -- Blueprint's ForLoop.
    ///
    /// Flow input: `"exec"`. Data inputs: `"first"`, `"last"` (`Number`).
    /// Flow outputs: `"body"`, `"completed"`. Data output: `"index"`
    /// (`Number`), readable only downstream of `body`.
    ForLoop,
    /// Runs `body` while `condition` holds, then `completed`. Stops after
    /// [`crate::compile::LOOP_LIMIT`] iterations with a logged warning, as
    /// Blueprint's runaway-loop guard does, so a condition that never turns
    /// false cannot hang the frame.
    ///
    /// Flow input: `"exec"`. Data input: `"condition"` (`Bool`). Flow
    /// outputs: `"body"`, `"completed"`.
    WhileLoop,
    /// Continues at `completed` after a number of frames -- Blueprint's
    /// Delay, on `Bsengine.setTimeout`. The flow after it runs on a later
    /// frame, not this one.
    ///
    /// Flow input: `"exec"`. Data input: `"frames"` (`Number`). Flow output:
    /// `"completed"`.
    Delay,
    /// Runs every so many seconds of game time -- Blueprint's looping timer,
    /// as an accumulator on `getDeltaTime`.
    ///
    /// Flow output: `"then"`.
    OnInterval(f64),
    /// A number as text, for HUD labels. Data input `"x"` (`Number`);
    /// output `"out"` (`Text`).
    ToText,
    /// Two texts joined. Data inputs `"a"`, `"b"` (`Text`); output `"out"`
    /// (`Text`).
    Concat,
}

/// A node: a stable id plus what it does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique within the graph; edges refer to nodes by this.
    pub id: u32,
    /// What this node does.
    pub kind: NodeKind,
    /// Where the node sits on the editor canvas, in panel-local pixels. Part
    /// of the authored asset for the reason the shader graph's is: Unity and
    /// Unreal store layout in the asset, and a graph without its layout is a
    /// different file to everyone who opens it. The compiler ignores it.
    #[serde(default)]
    pub position: [f32; 2],
}

/// A connection from one node's output port to another's input port -- a
/// flow edge or a data edge, told apart by the ports it joins.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source `(node id, port name)`.
    pub from: (u32, String),
    /// Destination `(node id, port name)`.
    pub to: (u32, String),
}

/// A graph variable: state that persists across frames for one entity's
/// script, as Blueprint and Unity graph variables do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Variable {
    /// How `GetVar` and `SetVar` nodes name it. Must be a JavaScript
    /// identifier fragment: letters, digits and underscores.
    pub name: String,
    /// Its value on the entity's first frame, and its type thereafter.
    pub initial: Value,
}

/// A whole graph.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ScriptGraph {
    /// Every node, in no particular order.
    pub nodes: Vec<GraphNode>,
    /// Every connection.
    pub edges: Vec<Edge>,
    /// Every variable. `#[serde(default)]` so a graph with none need not
    /// spell an empty list.
    #[serde(default)]
    pub variables: Vec<Variable>,
}

/// Why a graph could not be compiled.
///
/// Returned rather than panicked: the node editor compiles half-built graphs
/// on every edit and shows these beside the offending node, so they must be
/// values that name it.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    /// The graph has no event node, so nothing would ever run.
    NoEvent,
    /// An edge refers to a node id that does not exist.
    UnknownNode(u32),
    /// A `GetVar`/`SetVar` names a variable the graph does not declare.
    UnknownVariable {
        /// The node naming it.
        node: u32,
        /// The name.
        name: String,
    },
    /// A variable's name is not a JavaScript identifier fragment.
    BadVariableName(String),
    /// A `Call` names a function that is not in [`crate::ops::OPS`].
    UnknownOp {
        /// The node naming it.
        node: u32,
        /// The name.
        op: String,
    },
    /// A data input has no incoming edge.
    MissingInput {
        /// The node whose input is unconnected.
        node: u32,
        /// The port that needs a connection.
        port: String,
    },
    /// An edge connects incompatible types, or a flow port to a data port.
    TypeMismatch {
        /// The node whose input received the wrong type.
        node: u32,
        /// The port that received it.
        port: String,
        /// What that port needs.
        expected: String,
        /// What it was given.
        found: String,
    },
    /// A data dependency cycle; the node ids are those on it.
    Cycle(Vec<u32>),
    /// A flow that leads back into itself, which would run forever.
    FlowCycle(Vec<u32>),
    /// One flow output connected to more than one node, so which runs is
    /// undefined. Unreal and Unity allow exactly one connection per flow
    /// output for the same reason.
    AmbiguousFlow {
        /// The node with the doubly-connected flow output.
        node: u32,
        /// The port.
        port: String,
    },
    /// A value read outside the flow that produces it: a collision's
    /// `other` or a loop's `index` read from a flow the callback or loop
    /// body does not enclose. In the generated JavaScript that variable
    /// does not exist there, and the script would throw on the first frame
    /// that reached it.
    OutOfScope {
        /// The node reading it.
        node: u32,
        /// The port it reads it on.
        port: String,
        /// The node whose flow-scoped output it is.
        source: u32,
    },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::NoEvent => write!(
                f,
                "the graph has no event node (OnStart, OnUpdate, OnKeyPressed, OnCollision), so nothing would run"
            ),
            GraphError::UnknownNode(id) => {
                write!(f, "an edge refers to node {id}, which does not exist in the graph")
            }
            GraphError::UnknownVariable { node, name } => write!(
                f,
                "node {node} names variable \"{name}\", which the graph does not declare"
            ),
            GraphError::BadVariableName(name) => write!(
                f,
                "variable \"{name}\" is not a valid name: use letters, digits and underscores"
            ),
            GraphError::UnknownOp { node, op } => write!(
                f,
                "node {node} calls \"{op}\", which is not an engine function this compiler knows"
            ),
            GraphError::MissingInput { node, port } => {
                write!(f, "node {node}'s input port \"{port}\" has no incoming edge")
            }
            GraphError::TypeMismatch {
                node,
                port,
                expected,
                found,
            } => write!(
                f,
                "node {node}'s input port \"{port}\" needs {expected} but is given {found}"
            ),
            GraphError::Cycle(ids) => {
                write!(f, "the graph has a data cycle through nodes ")?;
                join_ids(f, ids)
            }
            GraphError::FlowCycle(ids) => {
                write!(f, "the flow loops back into itself through nodes ")?;
                join_ids(f, ids)
            }
            GraphError::AmbiguousFlow { node, port } => write!(
                f,
                "node {node}'s flow output \"{port}\" is connected to more than one node; a flow can only continue in one place"
            ),
            GraphError::OutOfScope { node, port, source } => write!(
                f,
                "node {node}'s input \"{port}\" reads a value of node {source} that only exists inside that node's own flow (its loop body or callback)"
            ),
        }
    }
}

fn join_ids(f: &mut std::fmt::Formatter<'_>, ids: &[u32]) -> std::fmt::Result {
    for (i, id) in ids.iter().enumerate() {
        if i > 0 {
            write!(f, ", ")?;
        }
        write!(f, "{id}")?;
    }
    Ok(())
}

impl std::error::Error for GraphError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn edge(from: (u32, &str), to: (u32, &str)) -> Edge {
        Edge {
            from: (from.0, from.1.to_string()),
            to: (to.0, to.1.to_string()),
        }
    }

    /// Graphs are authored as files, so a serde mismatch is a silent
    /// authoring failure. Every shape the format has is here: unit variants,
    /// newtype variants carrying a string, a `Value` and a `CompareOp`, the
    /// port tuples, positions and a variable.
    #[test]
    fn a_graph_round_trips_through_ron() {
        let graph = ScriptGraph {
            nodes: vec![
                GraphNode {
                    id: 0,
                    kind: NodeKind::OnUpdate,
                    position: [0.0, 0.0],
                },
                GraphNode {
                    id: 1,
                    kind: NodeKind::OnKeyPressed("Space".to_string()),
                    position: [10.0, -20.5],
                },
                GraphNode {
                    id: 2,
                    kind: NodeKind::Literal(Value::Number(2.5)),
                    position: [0.0, 40.0],
                },
                GraphNode {
                    id: 3,
                    kind: NodeKind::Literal(Value::Text("Enemy".to_string())),
                    position: [0.0, 80.0],
                },
                GraphNode {
                    id: 4,
                    kind: NodeKind::Compare(CompareOp::GreaterOrEqual),
                    position: [120.0, 40.0],
                },
                GraphNode {
                    id: 5,
                    kind: NodeKind::Call("addPosition".to_string()),
                    position: [240.0, 0.0],
                },
                GraphNode {
                    id: 6,
                    kind: NodeKind::SetVar("speed".to_string()),
                    position: [240.0, 80.0],
                },
                GraphNode {
                    id: 7,
                    kind: NodeKind::OnInterval(0.5),
                    position: [0.0, 200.0],
                },
                GraphNode {
                    id: 8,
                    kind: NodeKind::ForLoop,
                    position: [120.0, 200.0],
                },
                GraphNode {
                    id: 9,
                    kind: NodeKind::Delay,
                    position: [240.0, 200.0],
                },
            ],
            edges: vec![
                edge((0, "then"), (5, "exec")),
                edge((2, "out"), (4, "a")),
                edge((5, "then"), (6, "exec")),
                edge((7, "then"), (8, "exec")),
                edge((8, "completed"), (9, "exec")),
            ],
            variables: vec![Variable {
                name: "speed".to_string(),
                initial: Value::Number(1.0),
            }],
        };

        let text = ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::default())
            .expect("a script graph must serialise to RON");
        let parsed: ScriptGraph =
            ron::from_str(&text).unwrap_or_else(|e| panic!("RON round-trip failed: {e}\n{text}"));
        assert_eq!(parsed, graph);
    }

    /// A graph written without positions or variables still parses -- the
    /// two fields an author is most likely to omit by hand.
    #[test]
    fn positions_and_variables_are_optional_in_the_file() {
        let text = r#"(nodes: [(id: 1, kind: OnUpdate)], edges: [])"#;
        let g: ScriptGraph = ron::from_str(text).expect("must parse without the optional fields");
        assert_eq!(g.nodes[0].position, [0.0, 0.0]);
        assert!(g.variables.is_empty());
    }

    /// The node editor shows these beside the offending node, so the ones
    /// that carry an id, a port or a name must actually spell them.
    #[test]
    fn every_graph_error_names_what_it_blames() {
        assert!(GraphError::NoEvent.to_string().contains("event"));
        assert!(GraphError::UnknownNode(99).to_string().contains("99"));
        let v = GraphError::UnknownVariable {
            node: 3,
            name: "speed".to_string(),
        }
        .to_string();
        assert!(v.contains('3') && v.contains("speed"), "{v}");
        assert!(GraphError::BadVariableName("a-b".to_string())
            .to_string()
            .contains("a-b"));
        let o = GraphError::UnknownOp {
            node: 4,
            op: "fly".to_string(),
        }
        .to_string();
        assert!(o.contains('4') && o.contains("fly"), "{o}");
        let m = GraphError::MissingInput {
            node: 12,
            port: "condition".to_string(),
        }
        .to_string();
        assert!(m.contains("12") && m.contains("condition"), "{m}");
        let t = GraphError::TypeMismatch {
            node: 5,
            port: "a".to_string(),
            expected: "Number".to_string(),
            found: "Bool".to_string(),
        }
        .to_string();
        assert!(
            t.contains('5') && t.contains("Number") && t.contains("Bool"),
            "{t}"
        );
        let c = GraphError::Cycle(vec![3, 7]).to_string();
        assert!(c.contains('3') && c.contains('7'), "{c}");
        let fc = GraphError::FlowCycle(vec![8, 9]).to_string();
        assert!(fc.contains('8') && fc.contains('9'), "{fc}");
        let a = GraphError::AmbiguousFlow {
            node: 6,
            port: "then".to_string(),
        }
        .to_string();
        assert!(a.contains('6') && a.contains("then"), "{a}");
        let s = GraphError::OutOfScope {
            node: 14,
            port: "text".to_string(),
            source: 9,
        }
        .to_string();
        assert!(
            s.contains("14") && s.contains("text") && s.contains('9'),
            "{s}"
        );
    }

    /// The demo graph committed with this crate is the artifact a user
    /// authors, and a graph that only exists as a literal in a test proves
    /// nothing about the one on disk.
    #[test]
    fn the_shipped_demo_graph_still_parses() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../games/mini-arena/assets/scripts/bob.scriptgraph.ron"
        );
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("the demo graph must still be readable at {path}: {e}"));
        let g: ScriptGraph = ron::from_str(&text)
            .unwrap_or_else(|e| panic!("the shipped demo graph must still parse: {e}"));
        assert!(
            g.nodes.iter().any(|n| n.kind == NodeKind::OnUpdate),
            "the demo must have an OnUpdate to be a script at all"
        );
        assert!(!g.variables.is_empty(), "and a variable, to show one");
    }
}
