//! MCP tools: Reading the scene: list, get, query, components, hierarchy, stats.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        reflect_cmd_queue,
        selection,
        type_registry,
        ..
    } = cx;

    // list_entities
    let snap = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "list_entities".to_string(),
        description: "List all entities with their IDs, names, and positions".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let s = snap.lock().unwrap();
            McpToolOutput::success(json!({
                "entities": s.entities.iter().map(|e| json!({
                    "id": e.id,
                    "name": e.name,
                    "position": e.position,
                    "mesh_id": e.mesh_id,
                    "rotation": e.rotation,
                    "scale": e.scale,
                    "parent_id": e.parent_id,
                    "tags": e.tags,
                    "visible": e.visible,
                    "selected": e.selected,
                    "light_type": e.light_type,
                    "light_color": e.light_color,
                    "light_intensity": e.light_intensity,
                    "light_range": e.light_range,
                    "camera_fov": e.camera_fov,
                })).collect::<Vec<_>>()
            }))
        }),
    });

    // get_entity
    let snap2 = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "get_entity".to_string(),
        description: "Get detailed info for a specific entity by ID".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let s = snap2.lock().unwrap();
            match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => McpToolOutput::success(json!({ "entity": {
                    "id": e.id,
                    "name": e.name,
                    "position": e.position,
                    "rotation": e.rotation,
                    "scale": e.scale,
                    "mesh_id": e.mesh_id,
                    "light_type": e.light_type,
                    "light_color": e.light_color,
                    "light_intensity": e.light_intensity,
                    "light_range": e.light_range,
                    "camera_fov": e.camera_fov,
                    "parent_id": e.parent_id,
                    "tags": e.tags,
                    "visible": e.visible,
                    "selected": e.selected,
                }})),
                None => McpToolOutput::error("entity not found"),
            }
        }),
    });

    // get_scene_stats
    let snap_stats = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "get_scene_stats".to_string(),
        description: "Return aggregate counts for the current scene".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let s = snap_stats.lock().unwrap();
            let total = s.entities.len();
            let light_count = s.entities.iter().filter(|e| e.light_type.is_some()).count();
            let mesh_count = s.entities.iter().filter(|e| e.mesh_id.is_some()).count();
            let named_count = s.entities.iter().filter(|e| e.name.is_some()).count();
            McpToolOutput::success(json!({
                "total_entities": total,
                "light_count": light_count,
                "mesh_count": mesh_count,
                "named_count": named_count,
            }))
        }),
    });

    // query_entities -- see `entity_query` for why this is one tool.
    let snap_query = snapshot.clone();
    let sel_query = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "query_entities".to_string(),
        description: "Find, count, select or summarise entities by any combination of \
            conditions: `where` (all must hold) and `any` (one must hold) take \
            {field, op, value}; `sort` {by, desc}, `limit`, and `from`/`from_entity` \
            for the `distance` field. `action`: get (default), count, select, \
            deselect, select_only, tags (histogram), bounds (min/max/center). \
            Example: lights above y=3 named *Lamp*: {\"where\": [{\"field\": \
            \"light.type\", \"op\": \"exists\"}, {\"field\": \"position.y\", \"op\": \
            \"gt\", \"value\": 3}, {\"field\": \"name\", \"op\": \"contains\", \
            \"value\": \"Lamp\"}]}. An unknown field, op or key is an error."
            .to_string(),
        input_schema: Some(crate::entity_query::input_schema()),
        handler: Box::new(move |input| {
            // Snapshot first, selection second -- the order every
            // other handler that holds both takes them in.
            let s = snap_query.lock().unwrap();
            let mut sel = sel_query.lock().unwrap();
            match crate::entity_query::execute(&input, &s.entities, &mut sel) {
                Ok(v) => McpToolOutput::success(v),
                Err(e) => McpToolOutput::error(&format!("query_entities: {e}")),
            }
        }),
    });

    // get_components
    let snap_comp = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "get_components".to_string(),
        description: "List which components are attached to a specific entity".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let snapshot = snap_comp.lock().unwrap();
            let info = snapshot.entities.iter().find(|e| e.id == entity_id);
            match info {
                None => McpToolOutput::error("entity not found"),
                Some(e) => {
                    let mut components: Vec<&str> = Vec::new();
                    if e.name.is_some() {
                        components.push("Name");
                    }
                    if e.position.is_some() {
                        components.push("Transform");
                    }
                    if e.mesh_id.is_some() {
                        components.push("MeshRenderer");
                    }
                    if e.light_type.as_deref() == Some("point") {
                        components.push("PointLight");
                    } else if e.light_type.as_deref() == Some("directional") {
                        components.push("DirectionalLight");
                    } else if e.light_type.as_deref() == Some("spot") {
                        components.push("SpotLight");
                    }
                    if e.camera_fov.is_some() {
                        components.push("Camera");
                    }
                    McpToolOutput::success(
                        json!({"entity_id": entity_id, "components": components}),
                    )
                }
            }
        }),
    });

    // search_entities
    let snap_se = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "search_entities".to_string(),
        description:
            "Search entities by name (substring) or tag (exact match); returns union of matches"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "query": { "type": "string" } },
            "required": ["query"]
        })),
        handler: Box::new(move |input| {
            let query = match input["query"].as_str() {
                Some(q) => q.to_string(),
                None => return McpToolOutput::error("missing query"),
            };
            let s = snap_se.lock().unwrap();
            let entities: Vec<serde_json::Value> = s
                .entities
                .iter()
                .filter(|e| {
                    let name_match = e
                        .name
                        .as_deref()
                        .map(|n| n.contains(&*query))
                        .unwrap_or(false);
                    let tag_match = e.tags.iter().any(|t| t == &query);
                    name_match || tag_match
                })
                .map(|e| json!({"id": e.id, "name": e.name}))
                .collect();
            McpToolOutput::success(json!({"entities": entities}))
        }),
    });

    // get_scene_hierarchy
    let snap_hier = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "get_scene_hierarchy".to_string(),
        description: "Return the full scene hierarchy as a nested tree of {id, name, children}"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_hier.lock().unwrap();
            use std::collections::HashMap;
            let mut children_map: HashMap<u64, Vec<u64>> = HashMap::new();
            let mut root_ids: Vec<u64> = Vec::new();
            for e in &s.entities {
                match e.parent_id {
                    Some(pid) => children_map.entry(pid).or_default().push(e.id),
                    None => root_ids.push(e.id),
                }
            }
            fn build_node(
                id: u64,
                entities: &[crate::snapshot::EntityInfo],
                children_map: &HashMap<u64, Vec<u64>>,
            ) -> serde_json::Value {
                let name = entities
                    .iter()
                    .find(|e| e.id == id)
                    .and_then(|e| e.name.clone());
                let children: Vec<serde_json::Value> = children_map
                    .get(&id)
                    .map(|ids| {
                        ids.iter()
                            .map(|&cid| build_node(cid, entities, children_map))
                            .collect()
                    })
                    .unwrap_or_default();
                json!({"id": id, "name": name, "children": children})
            }
            let roots: Vec<serde_json::Value> = root_ids
                .iter()
                .map(|&id| build_node(id, &s.entities, &children_map))
                .collect();
            McpToolOutput::success(json!({"roots": roots}))
        }),
    });

    // has_component
    let snap_hc = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "has_component".to_string(),
        description: "Check whether an entity has a given component type (mesh, camera, point_light, directional_light, spot_light, transform)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "component": { "type": "string" }
            },
            "required": ["entity_id", "component"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let component = match input["component"].as_str() {
                Some(c) => c.to_string(),
                None => return McpToolOutput::error("missing component"),
            };
            let s = snap_hc.lock().unwrap();
            let e = match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => e,
                None => return McpToolOutput::error("entity not found"),
            };
            let has = match component.as_str() {
                "mesh" => e.mesh_id.is_some(),
                "camera" => e.camera_fov.is_some(),
                "point_light" => e.light_type.as_deref() == Some("point"),
                "directional_light" => e.light_type.as_deref() == Some("directional"),
                "spot_light" => e.light_type.as_deref() == Some("spot"),
                "transform" => e.position.is_some(),
                _ => return McpToolOutput::error("unknown component type"),
            };
            McpToolOutput::success(json!({"has_component": has, "entity_id": entity_id, "component": component}))
        }),
    });

    // set_reflected_component — generic attach-or-update for any
    // #[derive(Reflect, Component)] type, from a JSON value matching its
    // field shape. Closes the gap where AnimationStateMachine/NavMeshAgent/
    // Shield/Bloom/ToneMap have no attach path anywhere in the ~700-tool
    // MCP surface (their scripting setters are no-ops on a missing
    // component; only the native GUI's "Add Component" picker could attach
    // them before this). Reuses the exact ApplyComponentValue path the
    // GUI's own reflected field editor already goes through.
    let reflect_queue_for_set = reflect_cmd_queue.clone();
    let type_registry_for_set = type_registry.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_reflected_component".to_string(),
        description: "Attach (if missing) or update a reflected component on an \
            entity by its fully-qualified type path, from a JSON value matching \
            the component's field shape. Applied next frame via the same path \
            the Inspector's generic reflected field editor uses. Works for any \
            type registered with app.register_type -- including \
            AnimationStateMachine, NavMeshAgent, Shield, Bloom, ToneMap, and any \
            other component with no dedicated attach tool. Example value_json for \
            NavMeshAgent: {\"destination\": null, \"speed\": 3.5, \"angular_speed\": \
            2.0, \"acceleration\": 8.0, \"stopping_distance\": 0.1, \"radius\": 0.3, \
            \"height\": 1.8, \"state\": \"Idle\", \"enabled\": true}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "type_path": {
                    "type": "string",
                    "description": "e.g. bsengine_core::nav_mesh_agent::NavMeshAgent"
                },
                "value_json": {
                    "type": "string",
                    "description": "JSON object matching the component's fields"
                },
            },
            "required": ["entity_id", "type_path", "value_json"],
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let type_path = match input["type_path"].as_str() {
                Some(v) => v.to_string(),
                None => return McpToolOutput::error("missing type_path"),
            };
            let value_json = match input["value_json"].as_str() {
                Some(v) => v.to_string(),
                None => return McpToolOutput::error("missing value_json"),
            };
            let registry = type_registry_for_set.read();
            let Some(registration) = registry.get_with_type_path(&type_path) else {
                return McpToolOutput::error(&format!("unknown type path '{type_path}'"));
            };
            let de = bevy_reflect::serde::TypedReflectDeserializer::new(registration, &registry);
            let mut deserializer = serde_json::Deserializer::from_str(&value_json);
            let value = match serde::de::DeserializeSeed::deserialize(de, &mut deserializer) {
                Ok(v) => v,
                Err(e) => {
                    return McpToolOutput::error(&format!(
                        "value_json doesn't match '{type_path}': {e}"
                    ))
                }
            };
            reflect_queue_for_set
                .lock()
                .unwrap()
                .push(ReflectCommand::ApplyComponentValue {
                    entity_id,
                    type_path,
                    value,
                });
            McpToolOutput::success(json!({ "queued": true }))
        }),
    });
}
