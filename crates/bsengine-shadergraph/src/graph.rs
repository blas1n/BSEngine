//! The shader graph data model.
//!
//! Deliberately free of ECS, GPU and UI types: the compiler is a pure
//! function from this model to a WGSL string, which is what lets it be
//! unit-tested without an adapter and reused from both the editor UI and
//! asset tooling.

use serde::{Deserialize, Serialize};

/// One node's operation and its inline parameters.
///
/// Each variant's doc names its input ports. Every node has exactly one
/// output port, named `"out"`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    /// The interpolated UV coordinate, `vec2<f32>`. No inputs.
    Uv,
    /// Seconds since start, from `camera.time`, `f32`. No inputs.
    Time,
    /// A literal `f32`. No inputs.
    Constant(f32),
    /// A literal `vec3<f32>`. No inputs.
    ConstantVec3([f32; 3]),
    /// Samples the entity's texture at a `vec2<f32>` uv, giving `vec3<f32>`.
    ///
    /// Input port: `"uv"` (`vec2<f32>`, required).
    TextureSample,
    /// Component-wise sum of two same-typed inputs.
    ///
    /// Input ports: `"a"` and `"b"`, both required. A `f32` on either side
    /// broadcasts against a vector on the other, as it does in WGSL.
    Add,
    /// Component-wise product of two same-typed inputs.
    ///
    /// Input ports: `"a"` and `"b"`, both required. A `f32` on either side
    /// broadcasts against a vector on the other, as it does in WGSL.
    Multiply,
    /// `sin` of an `f32`.
    ///
    /// Input port: `"x"` (`f32`, required).
    Sin,
    /// Linear blend of `a` and `b` by an `f32` `t`.
    ///
    /// Input ports: `"a"` and `"b"` (same type, required) and `"t"`
    /// (`f32`, required).
    Lerp,
    /// 0 below `edge`, 1 at or above it. Both `f32`.
    ///
    /// Input ports: `"edge"` and `"x"`, both `f32` and required.
    Step,
    /// Fractional part, keeping a scrolling UV inside `[0, 1)`.
    ///
    /// Input port: `"x"` (any type, required); the result has the same type.
    Fract,
    /// The shader's result. Exactly one graph node must be this.
    ///
    /// Input ports: `"color"` (`vec3<f32>`, required) and `"alpha"`
    /// (`f32`, optional -- defaults to fully opaque when unconnected).
    Output,
}

/// A node: a stable id plus what it does.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GraphNode {
    /// Unique within the graph; edges refer to nodes by this.
    pub id: u32,
    /// What this node computes.
    pub kind: NodeKind,
    /// Where the node sits on the editor canvas, in panel-local pixels.
    ///
    /// Part of the authored asset, not transient UI state: Unity and Unreal
    /// both store graph layout in the asset, and a side-car file would mean
    /// sharing a graph loses its layout, or that one logical asset is two
    /// files. The compiler ignores it entirely, so the crate stays a pure
    /// graph-to-WGSL function.
    ///
    /// `#[serde(default)]` so graphs authored before this field existed --
    /// `games/mini-arena/assets/shaders/scroll.shadergraph.ron` among them --
    /// keep parsing, landing at the origin instead of failing to load.
    #[serde(default)]
    pub position: [f32; 2],
}

/// A connection from one node's output port to another's input port.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Edge {
    /// Source `(node id, port name)`. Every node here has one output port
    /// named `"out"`.
    pub from: (u32, String),
    /// Destination `(node id, port name)` -- see `NodeKind`'s docs for each
    /// node's input port names.
    pub to: (u32, String),
}

/// A whole graph.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ShaderGraph {
    /// Every node, in no particular order.
    pub nodes: Vec<GraphNode>,
    /// Every connection.
    pub edges: Vec<Edge>,
}

/// Why a graph could not be compiled.
///
/// Returned rather than panicked deliberately: sub-step 2/2's node editor
/// will show these next to the offending node, so they must be values.
#[derive(Debug, Clone, PartialEq)]
pub enum GraphError {
    /// The graph has no `Output` node.
    NoOutput,
    /// More than one `Output` node, so the result is ambiguous.
    MultipleOutputs,
    /// A cycle exists; the node ids are those still unresolved.
    Cycle(Vec<u32>),
    /// A required input port has no incoming edge.
    MissingInput {
        /// The node whose input is unconnected.
        node: u32,
        /// The port that needs a connection.
        port: String,
    },
    /// An edge connects incompatible types.
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
    /// An edge refers to a node id that does not exist.
    UnknownNode(u32),
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::NoOutput => {
                write!(f, "the graph has no Output node, so there is nothing to compile")
            }
            GraphError::MultipleOutputs => write!(
                f,
                "the graph has more than one Output node, so which one is the shader's result is ambiguous"
            ),
            GraphError::Cycle(ids) => {
                write!(f, "the graph has a cycle; these nodes never become resolvable: ")?;
                for (i, id) in ids.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{id}")?;
                }
                Ok(())
            }
            GraphError::MissingInput { node, port } => write!(
                f,
                "node {node}'s required input port \"{port}\" has no incoming edge"
            ),
            GraphError::TypeMismatch {
                node,
                port,
                expected,
                found,
            } => write!(
                f,
                "node {node}'s input port \"{port}\" needs {expected} but is given {found}"
            ),
            GraphError::UnknownNode(id) => {
                write!(f, "an edge refers to node {id}, which does not exist in the graph")
            }
        }
    }
}

impl std::error::Error for GraphError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_graph_round_trips_through_ron() {
        // Graphs are authored as files, so a serde mismatch is a silent
        // authoring failure rather than a compile error. Cover every shape
        // the format has: a unit variant, a newtype variant carrying a
        // float, a newtype variant carrying an array, and the `(u32,
        // String)` port tuples. Each node also carries a distinct position,
        // so a serialiser that emitted them in the wrong order or dropped
        // one would fail here rather than only in the dedicated position
        // test.
        let graph = ShaderGraph {
            nodes: vec![
                GraphNode {
                    id: 0,
                    kind: NodeKind::Uv,
                    position: [0.0, 0.0],
                },
                GraphNode {
                    id: 1,
                    kind: NodeKind::Time,
                    position: [10.0, 20.0],
                },
                GraphNode {
                    id: 2,
                    kind: NodeKind::Constant(0.25),
                    position: [10.0, 80.0],
                },
                GraphNode {
                    id: 3,
                    kind: NodeKind::ConstantVec3([1.0, 0.5, 0.0]),
                    position: [10.0, 140.0],
                },
                GraphNode {
                    id: 4,
                    kind: NodeKind::Multiply,
                    position: [180.0, 50.0],
                },
                GraphNode {
                    id: 5,
                    kind: NodeKind::Add,
                    position: [340.0, 25.0],
                },
                GraphNode {
                    id: 6,
                    kind: NodeKind::Fract,
                    position: [500.0, 25.0],
                },
                GraphNode {
                    id: 7,
                    kind: NodeKind::TextureSample,
                    position: [660.0, 25.0],
                },
                GraphNode {
                    id: 8,
                    kind: NodeKind::Output,
                    position: [820.0, 25.0],
                },
            ],
            edges: vec![
                Edge {
                    from: (1, "out".to_string()),
                    to: (4, "a".to_string()),
                },
                Edge {
                    from: (2, "out".to_string()),
                    to: (4, "b".to_string()),
                },
                Edge {
                    from: (0, "out".to_string()),
                    to: (5, "a".to_string()),
                },
                Edge {
                    from: (4, "out".to_string()),
                    to: (5, "b".to_string()),
                },
                Edge {
                    from: (5, "out".to_string()),
                    to: (6, "x".to_string()),
                },
                Edge {
                    from: (6, "out".to_string()),
                    to: (7, "uv".to_string()),
                },
                Edge {
                    from: (7, "out".to_string()),
                    to: (8, "color".to_string()),
                },
            ],
        };

        let text = ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::default())
            .expect("a shader graph must serialise to RON");
        let parsed: ShaderGraph =
            ron::from_str(&text).unwrap_or_else(|e| panic!("RON round-trip failed: {e}\n{text}"));

        assert_eq!(parsed, graph);
    }

    #[test]
    fn every_graph_error_displays_its_node_and_port() {
        // The node editor shows these strings next to the offending node, so
        // the ones that carry an id or a port must actually name them.
        assert!(GraphError::NoOutput.to_string().contains("Output"));
        assert!(GraphError::MultipleOutputs.to_string().contains("Output"));

        let cycle = GraphError::Cycle(vec![3, 7]).to_string();
        assert!(cycle.contains('3') && cycle.contains('7'), "{cycle}");

        let missing = GraphError::MissingInput {
            node: 12,
            port: "color".to_string(),
        }
        .to_string();
        assert!(
            missing.contains("12") && missing.contains("color"),
            "{missing}"
        );

        let mismatch = GraphError::TypeMismatch {
            node: 4,
            port: "alpha".to_string(),
            expected: "f32".to_string(),
            found: "vec3<f32>".to_string(),
        }
        .to_string();
        assert!(
            mismatch.contains('4')
                && mismatch.contains("alpha")
                && mismatch.contains("f32")
                && mismatch.contains("vec3<f32>"),
            "{mismatch}"
        );

        assert!(GraphError::UnknownNode(99).to_string().contains("99"));
    }

    #[test]
    fn a_graph_without_positions_still_parses() {
        // The demo graph committed in sub-step 1/2
        // (`games/mini-arena/assets/shaders/scroll.shadergraph.ron`) has no
        // `position` field. `#[serde(default)]` is what keeps it loading --
        // assert it rather than assuming it, because the failure mode is a
        // parse error on an already-shipped asset.
        let ron = r#"(nodes: [(id: 1, kind: Time)], edges: [])"#;
        let g: ShaderGraph = ron::from_str(ron).expect("must parse without positions");
        assert_eq!(g.nodes[0].position, [0.0, 0.0]);
    }

    #[test]
    fn the_shipped_demo_graph_still_parses() {
        // The test above pins the mechanism on a synthetic string; this one
        // pins it on the real file, which is what actually breaks. Reading
        // the asset itself is the only way to catch someone adding a second
        // non-defaulted field later.
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../games/mini-arena/assets/shaders/scroll.shadergraph.ron"
        );
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|e| panic!("the demo graph must still be readable at {path}: {e}"));
        let g: ShaderGraph = ron::from_str(&text)
            .unwrap_or_else(|e| panic!("the shipped demo graph must still parse: {e}"));

        assert_eq!(
            g.nodes.len(),
            8,
            "the demo graph's eight nodes must survive"
        );
        assert!(
            g.nodes.iter().all(|n| n.position == [0.0, 0.0]),
            "a position-less node must default to the origin, not fail to parse"
        );
    }

    #[test]
    fn positions_round_trip_through_ron() {
        // Layout is part of the authored asset, so it must survive a
        // save/load cycle -- otherwise every reopen scrambles the graph.
        // Negative and fractional values are included because a serialiser
        // that dropped the sign or truncated would still pass on `[10, 20]`.
        let graph = ShaderGraph {
            nodes: vec![
                GraphNode {
                    id: 0,
                    kind: NodeKind::Uv,
                    position: [-120.5, 37.25],
                },
                GraphNode {
                    id: 1,
                    kind: NodeKind::Output,
                    position: [640.0, -0.75],
                },
            ],
            edges: vec![Edge {
                from: (0, "out".to_string()),
                to: (1, "color".to_string()),
            }],
        };

        let text = ron::ser::to_string_pretty(&graph, ron::ser::PrettyConfig::default())
            .expect("a positioned graph must serialise to RON");
        let parsed: ShaderGraph =
            ron::from_str(&text).unwrap_or_else(|e| panic!("RON round-trip failed: {e}\n{text}"));

        assert_eq!(parsed, graph);
        assert_eq!(parsed.nodes[0].position, [-120.5, 37.25]);
        assert_eq!(parsed.nodes[1].position, [640.0, -0.75]);
    }
}
