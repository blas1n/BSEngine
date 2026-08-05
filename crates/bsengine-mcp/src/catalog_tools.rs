use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use crate::tool::{McpTool, McpToolOutput};

/// What an agent reads when deciding whether to call this tool at all.
///
/// Written at length on purpose. The catalogue is only worth building if it
/// gets consulted *before* a component or op is written, and an agent that
/// reads a terse label will not think to ask. So this says when to call it,
/// what the two lists mean separately, and that an empty answer is a real
/// answer rather than a failure.
const CATALOG_DESCRIPTION: &str = "\
Ask what already owns a concept in this engine — call this BEFORE adding a new component or a new \
scripting op, to find out whether the concept exists already and who owns it.\n\n\
Pass `concept` as the single word you were about to name the new thing after: \"velocity\", \
\"health\", \"grounded\". You get back every component whose type name or field name uses that \
word, and every #[op2] scripting op whose name uses it, each with its crate, source location and \
rustdoc.\n\n\
Read BOTH lists, not just the first one that has hits. A concept can be owned by a component, by \
a family of ops, or by both at once in different crates. `velocity` is the case that motivated \
this tool: `bsengine_core::Velocity` owns it as a component, and separately a whole family of ops \
in `bsengine-scripting` owns it against the physics backend — an answer mentioning only one of \
those is how a duplicate gets written.\n\n\
Ops with no component mean the concept lives outside the component set entirely — in a backend, \
in a resource, or only in the scripting layer. Extend what is already there rather than adding a \
component that would shadow it.\n\n\
An empty result is a real and useful answer: nothing owns that word yet, so adding it is clear.\n\n\
Omit `concept` to list the whole catalogue instead — every component with its crate, fields, \
rustdoc, whether it is `pub`, and whether it is registered for reflection (an unregistered public \
component is invisible to scenes, the Inspector and MCP) — plus every scripting op.";

/// Builds the `component_catalog` tool, answering against a fresh scan of the
/// workspace at `root`.
///
/// The scan runs per call rather than once at startup: the tool is asked
/// during design, while the source it reports on is being edited, so a cached
/// answer would go stale exactly when it is being relied on.
pub fn catalog_tools(root: PathBuf) -> Vec<McpTool> {
    vec![McpTool {
        name: "component_catalog".to_string(),
        description: CATALOG_DESCRIPTION.to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "concept": {
                    "type": "string",
                    "description": "One word to look for, e.g. 'velocity'. Matched against \
                        component names, component field names and op names, word by word. \
                        Omit to list the entire catalogue.",
                },
            },
        })),
        handler: Box::new(move |args| component_catalog(&root, args)),
    }]
}

fn component_catalog(root: &Path, args: Value) -> McpToolOutput {
    let catalog = match bsengine_catalog::Catalog::scan(root) {
        Ok(c) => c,
        Err(e) => {
            return McpToolOutput::error(&format!(
                "failed to scan the workspace at {}: {e}",
                root.display()
            ))
        }
    };

    // A `concept` that matches nothing is answered with empty lists, not an
    // error: "nothing owns this yet" is the answer that unblocks adding it.
    let serialised = match args.get("concept").and_then(|v| v.as_str()) {
        Some(concept) => serde_json::to_value(catalog.concept(concept)),
        None => serde_json::to_value(&catalog),
    };

    match serialised {
        Ok(v) => McpToolOutput::success(v),
        Err(e) => McpToolOutput::error(&format!("failed to serialise the catalogue: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_root() -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root is two levels above crates/bsengine-mcp")
            .to_path_buf()
    }

    #[test]
    fn the_tool_is_exposed_with_a_schema() {
        let tools = catalog_tools(workspace_root());
        let t = tools
            .iter()
            .find(|t| t.name == "component_catalog")
            .expect("component_catalog is exposed");
        assert!(t.input_schema.is_some());
    }

    #[test]
    fn a_concept_query_reports_both_owners() {
        // The query that would have prevented this whole item: velocity is
        // owned by a component in one crate and by ops driving another.
        let tools = catalog_tools(workspace_root());
        let t = tools
            .iter()
            .find(|t| t.name == "component_catalog")
            .unwrap();
        let out = (t.handler)(serde_json::json!({ "concept": "velocity" }));
        assert!(out.error.is_none(), "query failed: {:?}", out.error);
        let text = serde_json::to_string(&out.content).expect("serialises");
        assert!(text.contains("\"Velocity\""), "names the component");
        assert!(text.contains("bsengine_get_velocity"), "names the ops");
    }

    #[test]
    fn listing_reports_every_component_with_its_crate() {
        let tools = catalog_tools(workspace_root());
        let t = tools
            .iter()
            .find(|t| t.name == "component_catalog")
            .unwrap();
        let out = (t.handler)(serde_json::json!({}));
        assert!(out.error.is_none(), "listing failed: {:?}", out.error);
        let text = serde_json::to_string(&out.content).expect("serialises");
        assert!(text.contains("RigidBody") && text.contains("bsengine-physics"));
    }

    #[test]
    fn an_unknown_concept_is_an_empty_answer_not_an_error() {
        // "nothing owns this yet" is the answer that unblocks adding it.
        let tools = catalog_tools(workspace_root());
        let t = tools
            .iter()
            .find(|t| t.name == "component_catalog")
            .unwrap();
        let out = (t.handler)(serde_json::json!({ "concept": "zzzznotathing" }));
        assert!(out.error.is_none(), "unknown concepts are not errors");
    }
}
