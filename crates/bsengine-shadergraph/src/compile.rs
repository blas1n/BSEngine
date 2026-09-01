//! Graph to WGSL compilation.
//!
//! [`compile`] finds the single `Output` node, walks backwards from it to
//! collect the nodes that actually contribute, topologically sorts them, type
//! checks every connection, and emits one SSA `let n{id} = ...;` line per
//! node. Dead-node elimination is not a pass of its own: it falls out of the
//! backward walk, because a node no edge chain reaches from `Output` is never
//! collected in the first place.
//!
//! Nothing here panics on malformed input. Every failure is a [`GraphError`]
//! value, because the node editor compiles half-built graphs on every edit and
//! shows the result next to the offending node.

use std::collections::{HashMap, HashSet};

use crate::graph::{GraphError, GraphNode, NodeKind, ShaderGraph};

/// The WGSL type a value carries between nodes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueType {
    F32,
    Vec2,
    Vec3,
}

impl ValueType {
    /// How the type is spelled in WGSL, and in `GraphError::TypeMismatch`.
    fn wgsl(self) -> &'static str {
        match self {
            ValueType::F32 => "f32",
            ValueType::Vec2 => "vec2<f32>",
            ValueType::Vec3 => "vec3<f32>",
        }
    }
}

/// One input port of a node kind.
struct Port {
    /// The name an `Edge`'s `to.1` must carry to connect here.
    name: &'static str,
    /// The type this port accepts, or `None` when it accepts any of them and
    /// the node's result type follows from what it is given.
    expected: Option<ValueType>,
    /// Whether leaving it unconnected is a `MissingInput` error.
    required: bool,
}

const fn port(name: &'static str, expected: Option<ValueType>, required: bool) -> Port {
    Port {
        name,
        expected,
        required,
    }
}

/// The input ports of a node kind, in the order they are checked.
///
/// The order is what makes error reporting deterministic: `Output` with
/// nothing connected is blamed on `color`, not on `alpha`.
fn input_ports(kind: &NodeKind) -> &'static [Port] {
    const NONE: &[Port] = &[];
    const UV: &[Port] = &[port("uv", Some(ValueType::Vec2), true)];
    const A_B: &[Port] = &[port("a", None, true), port("b", None, true)];
    const X_F32: &[Port] = &[port("x", Some(ValueType::F32), true)];
    const LERP: &[Port] = &[
        port("a", None, true),
        port("b", None, true),
        port("t", Some(ValueType::F32), true),
    ];
    const STEP: &[Port] = &[
        port("edge", Some(ValueType::F32), true),
        port("x", Some(ValueType::F32), true),
    ];
    const X_ANY: &[Port] = &[port("x", None, true)];
    const OUTPUT: &[Port] = &[
        port("color", Some(ValueType::Vec3), true),
        port("alpha", Some(ValueType::F32), false),
    ];

    match kind {
        NodeKind::Uv | NodeKind::Time | NodeKind::Constant(_) | NodeKind::ConstantVec3(_) => NONE,
        NodeKind::TextureSample => UV,
        NodeKind::Add | NodeKind::Multiply => A_B,
        NodeKind::Sin => X_F32,
        NodeKind::Lerp => LERP,
        NodeKind::Step => STEP,
        NodeKind::Fract => X_ANY,
        NodeKind::Output => OUTPUT,
    }
}

/// The WGSL literal for an `f32` parameter.
///
/// `{:?}` is the shortest representation that round-trips, and for every
/// finite `f32` it carries a `.` or an `e`, so the result is always a float
/// literal rather than an integer one. A non-finite constant has no WGSL
/// spelling at all and is emitted as-is, so it fails loudly at shader-parse
/// time; substituting some other number would hide the authoring mistake.
fn wgsl_f32(v: f32) -> String {
    format!("{v:?}")
}

/// The connections into one node, as `port name -> source node id`.
type Inputs = HashMap<&'static str, u32>;

/// Which source node feeds `port`, if any.
fn source(inputs: &Inputs, port: &str) -> Option<u32> {
    inputs.get(port).copied()
}

/// The SSA binding name a node's result is bound to.
fn binding(id: u32) -> String {
    format!("n{id}")
}

/// The result type of a node, given the types of its connected inputs.
///
/// The polymorphic kinds resolve here rather than in the per-port check,
/// because their result depends on what they were given.
fn result_type(
    id: u32,
    kind: &NodeKind,
    types: &HashMap<&str, ValueType>,
) -> Result<ValueType, GraphError> {
    // Every required port is checked before this runs, so the polymorphic
    // ports below are known to be present.
    let get = |name: &str| types.get(name).copied().unwrap_or(ValueType::F32);

    let mismatch = |port: &str, expected: ValueType, found: ValueType| GraphError::TypeMismatch {
        node: id,
        port: port.to_string(),
        expected: expected.wgsl().to_string(),
        found: found.wgsl().to_string(),
    };

    Ok(match kind {
        NodeKind::Uv => ValueType::Vec2,
        NodeKind::Time | NodeKind::Constant(_) => ValueType::F32,
        NodeKind::ConstantVec3(_) | NodeKind::TextureSample => ValueType::Vec3,
        NodeKind::Sin | NodeKind::Step => ValueType::F32,
        NodeKind::Fract => get("x"),
        NodeKind::Add | NodeKind::Multiply => {
            // WGSL broadcasts a scalar against a vector, so mixing an f32
            // with a vector is well defined and widens to the vector; two
            // different vector widths are not.
            let (a, b) = (get("a"), get("b"));
            match (a, b) {
                _ if a == b => a,
                (ValueType::F32, other) | (other, ValueType::F32) => other,
                _ => return Err(mismatch("b", a, b)),
            }
        }
        NodeKind::Lerp => {
            let (a, b) = (get("a"), get("b"));
            if a != b {
                return Err(mismatch("b", a, b));
            }
            a
        }
        // `Output` is emitted specially and its result is never consumed.
        NodeKind::Output => ValueType::Vec3,
    })
}

/// The WGSL expression a node's SSA binding is assigned.
fn expression(kind: &NodeKind, inputs: &Inputs) -> String {
    let arg = |name: &str| {
        source(inputs, name)
            .map(binding)
            // Only reachable for an optional port, which every caller below
            // handles before asking.
            .unwrap_or_else(|| "0.0".to_string())
    };

    match kind {
        NodeKind::Uv => "in.uv".to_string(),
        NodeKind::Time => "camera.time".to_string(),
        NodeKind::Constant(v) => wgsl_f32(*v),
        NodeKind::ConstantVec3([x, y, z]) => format!(
            "vec3<f32>({}, {}, {})",
            wgsl_f32(*x),
            wgsl_f32(*y),
            wgsl_f32(*z)
        ),
        NodeKind::TextureSample => {
            format!("textureSample(t_diffuse, s_diffuse, {}).rgb", arg("uv"))
        }
        NodeKind::Add => format!("({} + {})", arg("a"), arg("b")),
        NodeKind::Multiply => format!("({} * {})", arg("a"), arg("b")),
        NodeKind::Sin => format!("sin({})", arg("x")),
        NodeKind::Lerp => format!("mix({}, {}, {})", arg("a"), arg("b"), arg("t")),
        NodeKind::Step => format!("step({}, {})", arg("edge"), arg("x")),
        NodeKind::Fract => format!("fract({})", arg("x")),
        NodeKind::Output => {
            let alpha = source(inputs, "alpha")
                .map(binding)
                .unwrap_or_else(|| "1.0".to_string());
            format!("vec4<f32>({}, {})", arg("color"), alpha)
        }
    }
}

/// Everything the shader body needs around it, up to and including the vertex
/// stage.
///
/// `CameraUniform` and `ModelUniform` are copied **verbatim** from `MESH_WGSL`
/// in `crates/bsengine-rhi-wgpu/src/surface.rs`, `_pad` fields and all. Custom
/// shaders are compiled against the same `pipeline_layout` as the standard mesh
/// pipeline, so a difference of one field name, one position, or one dropped
/// pad makes pipeline creation fail inside wgpu validation -- with a message
/// naming the pipeline, nowhere near this file. `bsengine-rhi-wgpu`'s
/// `the_shadergraph_preamble_matches_the_mesh_pipeline_uniforms` compares these
/// two declarations byte for byte against the real `MESH_WGSL` so the drift is
/// caught here instead.
///
/// Groups 2 (light) and 3 (texture) belong to that same layout but are only
/// declared when used: wgpu validates the bind groups a shader references, not
/// the ones the layout offers. Group 3 arrives with [`TEXTURE_BINDINGS`].
const PREAMBLE: &str = r#"// Generated by bsengine-shadergraph. Do not edit by hand.
//
// The uniform structs below are copied verbatim from MESH_WGSL in
// crates/bsengine-rhi-wgpu/src/surface.rs. Custom shaders are compiled against
// the standard mesh pipeline's layout, so field name, order and padding must
// match it exactly or pipeline creation fails wgpu validation.
struct CameraUniform {
    view_proj: mat4x4<f32>,
    light_view_proj: mat4x4<f32>,
    cam_pos: vec3<f32>,
    time: f32,
};
struct ModelUniform {
    model: mat4x4<f32>,
    metallic: f32,
    roughness: f32,
    _pad0: f32,
    _pad1: f32,
    emissive: vec3<f32>,
    _pad2: f32,
    base_color: vec3<f32>,
    opacity: f32,
};
@group(0) @binding(0) var<uniform> camera: CameraUniform;
@group(1) @binding(0) var<uniform> model_data: ModelUniform;

struct VertIn {
    @location(0) pos: vec3<f32>,
    @location(1) col: vec3<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) uv: vec2<f32>,
}
// Unlike games/mini-arena/assets/shaders/glow.wgsl's, this VertOut carries uv.
// A generated shader emits both stages, so it owns its interstage struct, and
// the `Uv` node reads `in.uv` in the fragment stage -- which is only there if
// the vertex stage passed it through. Copying glow.wgsl's normal/position-only
// VertOut would break every graph that samples a texture.
struct VertOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) world_normal: vec3<f32>,
    @location(1) world_pos: vec3<f32>,
    @location(2) uv: vec2<f32>,
}

@vertex
fn vs_main(in: VertIn) -> VertOut {
    var out: VertOut;
    let world_pos4 = model_data.model * vec4<f32>(in.pos, 1.0);
    out.clip_pos = camera.view_proj * world_pos4;
    out.world_pos = world_pos4.xyz;
    let normal_matrix = mat3x3<f32>(
        model_data.model[0].xyz,
        model_data.model[1].xyz,
        model_data.model[2].xyz,
    );
    out.world_normal = normalize(normal_matrix * in.normal);
    out.uv = in.uv;
    return out;
}
"#;

/// The group 3 bindings a `TextureSample` node's expression reads.
///
/// Emitted only when the graph actually contains one. Declaring them
/// unconditionally would be harmless for the layout but would make every
/// generated shader claim a texture it never reads.
const TEXTURE_BINDINGS: &str = r#"
@group(3) @binding(0) var t_diffuse: texture_2d<f32>;
@group(3) @binding(1) var s_diffuse: sampler;
"#;

/// Compiles a graph to a WGSL shader.
///
/// Only the nodes reachable backwards from the single `Output` node are
/// emitted, each as one SSA `let` binding, in an order where every node's
/// inputs are already bound.
///
/// # Errors
///
/// Returns a [`GraphError`] rather than panicking for every malformed graph:
/// no `Output` or several of them, an edge naming a node that does not exist,
/// a cycle, a required input left unconnected, or a connection between
/// incompatible types. Half-built graphs are the normal case for the node
/// editor, so none of these is exceptional.
pub fn compile(graph: &ShaderGraph) -> Result<String, GraphError> {
    let mut nodes: HashMap<u32, &GraphNode> = HashMap::new();
    for node in &graph.nodes {
        // A duplicated id is authoring corruption with no error variant of
        // its own; the first definition wins so the result stays stable.
        nodes.entry(node.id).or_insert(node);
    }

    // 1. Exactly one Output.
    let mut outputs = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::Output)
        .map(|n| n.id);
    let output_id = outputs.next().ok_or(GraphError::NoOutput)?;
    if outputs.next().is_some() {
        return Err(GraphError::MultipleOutputs);
    }

    // The connections into each node, resolved once. An edge naming a port
    // the destination node does not have is ignored, as is a second edge into
    // an already-connected port -- neither has an error variant, and the
    // first connection winning keeps compilation deterministic.
    let mut incoming: HashMap<u32, Inputs> = HashMap::new();
    for edge in &graph.edges {
        let (to_id, to_port) = (&edge.to.0, edge.to.1.as_str());
        let Some(dest) = nodes.get(to_id) else {
            continue;
        };
        let Some(port) = input_ports(&dest.kind).iter().find(|p| p.name == to_port) else {
            continue;
        };
        incoming
            .entry(*to_id)
            .or_default()
            .entry(port.name)
            .or_insert(edge.from.0);
    }
    let no_inputs = Inputs::new();
    let inputs_of = |id: &u32| incoming.get(id).unwrap_or(&no_inputs);

    // 2. Walk backwards from Output. `seen` is what stops a cycle here from
    // looping forever -- the cycle itself is reported in step 3.
    let mut seen: HashSet<u32> = HashSet::from([output_id]);
    let mut stack = vec![output_id];
    while let Some(id) = stack.pop() {
        let kind = &nodes[&id].kind;
        let inputs = inputs_of(&id);
        for p in input_ports(kind) {
            let Some(src) = source(inputs, p.name) else {
                continue;
            };
            if !nodes.contains_key(&src) {
                return Err(GraphError::UnknownNode(src));
            }
            if seen.insert(src) {
                stack.push(src);
            }
        }
    }

    // 3. Topologically sort the reachable set. Starting from ascending ids
    // and preserving that order makes the emitted shader reproducible.
    let mut pending: Vec<u32> = seen.iter().copied().collect();
    pending.sort_unstable();

    let mut order: Vec<u32> = Vec::with_capacity(pending.len());
    let mut resolved: HashSet<u32> = HashSet::new();
    while !pending.is_empty() {
        let mut blocked = Vec::new();
        let mut progressed = false;
        for id in pending {
            let inputs = inputs_of(&id);
            let ready = input_ports(&nodes[&id].kind)
                .iter()
                .filter_map(|p| source(inputs, p.name))
                .all(|src| resolved.contains(&src));
            if ready {
                resolved.insert(id);
                order.push(id);
                progressed = true;
            } else {
                blocked.push(id);
            }
        }
        if !progressed {
            return Err(GraphError::Cycle(blocked));
        }
        pending = blocked;
    }

    // 4 and 5. Type check and emit, in dependency order so every input's
    // type is already known by the time it is read.
    let mut types: HashMap<u32, ValueType> = HashMap::new();
    let mut body = String::new();
    for id in &order {
        let node = nodes[id];
        let inputs = inputs_of(id);
        let mut input_types: HashMap<&str, ValueType> = HashMap::new();

        for p in input_ports(&node.kind) {
            let Some(src) = source(inputs, p.name) else {
                if p.required {
                    return Err(GraphError::MissingInput {
                        node: *id,
                        port: p.name.to_string(),
                    });
                }
                continue;
            };
            let found = types[&src];
            if let Some(expected) = p.expected {
                if expected != found {
                    return Err(GraphError::TypeMismatch {
                        node: *id,
                        port: p.name.to_string(),
                        expected: expected.wgsl().to_string(),
                        found: found.wgsl().to_string(),
                    });
                }
            }
            input_types.insert(p.name, found);
        }

        types.insert(*id, result_type(*id, &node.kind, &input_types)?);
        body.push_str(&format!(
            "    let {} = {};\n",
            binding(*id),
            expression(&node.kind, inputs)
        ));
    }

    let uses_texture = order
        .iter()
        .any(|id| nodes[id].kind == NodeKind::TextureSample);

    let mut wgsl = String::from(PREAMBLE);
    if uses_texture {
        wgsl.push_str(TEXTURE_BINDINGS);
    }
    wgsl.push_str("\n@fragment\nfn fs_main(in: VertOut) -> @location(0) vec4<f32> {\n");
    wgsl.push_str(&body);
    wgsl.push_str(&format!("    return {};\n}}\n", binding(output_id)));
    Ok(wgsl)
}

#[cfg(test)]
mod tests {
    use crate::compile::compile;
    use crate::graph::{Edge, GraphError, GraphNode, NodeKind, ShaderGraph};

    /// `(from_node, to_node, to_port)` -- every node's single output port is
    /// named `"out"`, so spelling it at each call site only adds noise.
    fn edge(from: u32, to: u32, port: &str) -> Edge {
        Edge {
            from: (from, "out".to_string()),
            to: (to, port.to_string()),
        }
    }

    fn node(id: u32, kind: NodeKind) -> GraphNode {
        GraphNode { id, kind }
    }

    #[test]
    fn a_constant_into_output_compiles() {
        let graph = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([0.25, 0.5, 0.75])),
                node(1, NodeKind::Output),
            ],
            edges: vec![edge(0, 1, "color")],
        };

        let wgsl = compile(&graph).expect("the simplest possible graph must compile");

        assert!(
            wgsl.contains("let n0 = vec3<f32>(0.25, 0.5, 0.75);"),
            "the constant must be emitted as its own SSA binding:\n{wgsl}"
        );
        assert!(
            wgsl.contains("fn fs_main"),
            "the result must be a whole shader, not a bare expression:\n{wgsl}"
        );
    }

    #[test]
    fn only_nodes_reachable_from_output_are_emitted() {
        // Node 2 is wired to nothing. Dead-node elimination falls out of
        // walking backwards from Output; asserting it here is what keeps
        // that property from silently regressing into "emit everything".
        let graph = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([0.25, 0.5, 0.75])),
                node(1, NodeKind::Output),
                node(2, NodeKind::ConstantVec3([9.5, 9.5, 9.5])),
            ],
            edges: vec![edge(0, 1, "color")],
        };

        let wgsl = compile(&graph).expect("a dead node must not stop a valid graph compiling");

        assert!(
            wgsl.contains("let n0 ="),
            "the reachable node must still be emitted:\n{wgsl}"
        );
        assert!(
            !wgsl.contains("let n2 ="),
            "the unreachable node must not be emitted:\n{wgsl}"
        );
        assert!(
            !wgsl.contains("9.5"),
            "the unreachable node's expression must not appear anywhere:\n{wgsl}"
        );
    }

    #[test]
    fn a_cycle_is_reported_not_panicked() {
        // 2 and 3 feed each other, and 2 feeds the Output so the cycle is
        // reachable. Must return Err, never panic and never loop forever --
        // the editor calls this on half-built graphs constantly.
        let graph = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([1.0, 1.0, 1.0])),
                node(1, NodeKind::Output),
                node(2, NodeKind::Add),
                node(3, NodeKind::Add),
            ],
            edges: vec![
                edge(0, 2, "a"),
                edge(3, 2, "b"),
                edge(0, 3, "a"),
                edge(2, 3, "b"),
                edge(2, 1, "color"),
            ],
        };

        match compile(&graph) {
            Err(GraphError::Cycle(mut ids)) => {
                ids.sort_unstable();
                // `Cycle` carries every node left unresolvable, which is the
                // two cycle members plus the Output that waits on them --
                // exactly the set the editor should mark as stuck. Node 0
                // resolves fine and must not be blamed.
                assert_eq!(ids, vec![1, 2, 3], "the stuck nodes must be named");
            }
            other => panic!("expected Err(Cycle), got {other:?}"),
        }
    }

    #[test]
    fn the_scrolling_texture_shape_emits_ssa_in_dependency_order() {
        // `Uv + (Time * 0.1)` -> `Fract` -> `TextureSample` -> `Output`, the
        // shape the demo graph is authored in. It pins two things a later
        // task depends on: the port names, and that adding an f32 to a
        // vec2 broadcasts rather than being a type error, so the UV offset
        // does not need a ConstantVec3 to spell.
        let graph = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::Uv),
                node(1, NodeKind::Time),
                node(2, NodeKind::Constant(0.1)),
                node(3, NodeKind::Multiply),
                node(4, NodeKind::Add),
                node(5, NodeKind::Fract),
                node(6, NodeKind::TextureSample),
                node(7, NodeKind::Output),
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
        };

        let wgsl = compile(&graph).expect("the demo graph shape must compile");

        for line in [
            "let n0 = in.uv;",
            "let n1 = camera.time;",
            "let n2 = 0.1;",
            "let n3 = (n1 * n2);",
            "let n4 = (n0 + n3);",
            "let n5 = fract(n4);",
            "let n6 = textureSample(t_diffuse, s_diffuse, n5).rgb;",
            "let n7 = vec4<f32>(n6, 1.0);",
            "return n7;",
        ] {
            assert!(wgsl.contains(line), "missing `{line}` in:\n{wgsl}");
        }

        // Every binding must be defined before it is read.
        let at = |needle: &str| wgsl.find(needle).expect(needle);
        assert!(at("let n3 =") < at("let n4 ="));
        assert!(at("let n4 =") < at("let n5 ="));
        assert!(at("let n6 =") < at("let n7 ="));
    }

    #[test]
    fn the_generated_vert_out_carries_uv_and_the_vertex_stage_fills_it() {
        // `glow.wgsl`'s VertOut has no uv, so copying it would leave `in.uv`
        // undefined in the fragment stage and break every texturing graph.
        // The generated shader owns both stages, so it must declare its own.
        let graph = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([1.0, 1.0, 1.0])),
                node(1, NodeKind::Output),
            ],
            edges: vec![edge(0, 1, "color")],
        };

        let wgsl = compile(&graph).expect("the simplest graph must compile");

        let vert_out = wgsl
            .split_once("struct VertOut {")
            .and_then(|(_, rest)| rest.split_once('}'))
            .map(|(body, _)| body.to_string())
            .expect("the preamble must declare a VertOut");
        assert!(
            vert_out.contains("uv: vec2<f32>"),
            "VertOut must carry uv, unlike glow.wgsl's:\n{vert_out}"
        );
        assert!(
            wgsl.contains("out.uv = in.uv;"),
            "vs_main must pass uv through, or `in.uv` is never written:\n{wgsl}"
        );
    }

    #[test]
    fn the_texture_bindings_appear_only_when_a_graph_samples_a_texture() {
        // wgpu validates the bind groups a shader references, not the ones its
        // layout offers, so group 3 is declared on demand. Asserting both
        // directions is what stops "declare it always" from creeping back in.
        let untextured = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([1.0, 1.0, 1.0])),
                node(1, NodeKind::Output),
            ],
            edges: vec![edge(0, 1, "color")],
        };
        let textured = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::Uv),
                node(1, NodeKind::TextureSample),
                node(2, NodeKind::Output),
            ],
            edges: vec![edge(0, 1, "uv"), edge(1, 2, "color")],
        };

        let without = compile(&untextured).expect("an untextured graph must compile");
        let with = compile(&textured).expect("a texturing graph must compile");

        assert!(
            !without.contains("t_diffuse"),
            "a graph that samples nothing must not declare group 3:\n{without}"
        );
        assert!(
            with.contains("@group(3) @binding(0) var t_diffuse: texture_2d<f32>;")
                && with.contains("@group(3) @binding(1) var s_diffuse: sampler;"),
            "a texturing graph must declare the bindings its body reads:\n{with}"
        );
        // Declaration before use: WGSL is order-independent for module-scope
        // items, but the emitted file should still read top to bottom.
        let at = |haystack: &str, needle: &str| haystack.find(needle).map(|i| i as isize);
        assert!(
            at(&with, "var t_diffuse") < at(&with, "fn fs_main"),
            "the bindings must precede the fragment stage that reads them"
        );
    }

    #[test]
    fn a_graph_without_an_output_is_rejected() {
        let graph = ShaderGraph {
            nodes: vec![node(0, NodeKind::ConstantVec3([1.0, 0.0, 0.0]))],
            edges: vec![],
        };

        assert_eq!(compile(&graph), Err(GraphError::NoOutput));
    }

    #[test]
    fn an_unconnected_required_input_is_rejected() {
        // `Output.color` is required; `Output.alpha` is not, so leaving both
        // unconnected must still be blamed on `color`.
        let graph = ShaderGraph {
            nodes: vec![node(7, NodeKind::Output)],
            edges: vec![],
        };

        assert_eq!(
            compile(&graph),
            Err(GraphError::MissingInput {
                node: 7,
                port: "color".to_string(),
            })
        );
    }

    #[test]
    fn connecting_a_vec3_into_a_float_port_is_rejected() {
        // `Output.alpha` is an f32 port; node 2 hands it a vec3.
        let graph = ShaderGraph {
            nodes: vec![
                node(0, NodeKind::ConstantVec3([1.0, 1.0, 1.0])),
                node(1, NodeKind::Output),
                node(2, NodeKind::ConstantVec3([0.5, 0.5, 0.5])),
            ],
            edges: vec![edge(0, 1, "color"), edge(2, 1, "alpha")],
        };

        assert_eq!(
            compile(&graph),
            Err(GraphError::TypeMismatch {
                node: 1,
                port: "alpha".to_string(),
                expected: "f32".to_string(),
                found: "vec3<f32>".to_string(),
            })
        );
    }
}
