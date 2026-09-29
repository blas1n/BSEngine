//! MCP tools: Scene and prefab files: save, load, clear, write, apply.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        prefab_apply_cmd_queue,
        project_dir,
        ..
    } = cx;

    // save_scene
    let snap3 = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "save_scene".to_string(),
        description: "Serialize current named entities to a RON scene file".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Destination file path (.ron)" } },
            "required": ["path"]
        })),
        handler: Box::new(move |input| {
            let path = match input["path"].as_str() {
                Some(p) => p.to_string(),
                None => return McpToolOutput::error("missing 'path' field"),
            };
            let s = snap3.lock().unwrap();
            let entities = build_entity_descriptors(&s.entities);
            let count = entities.len();
            let scene = SceneDescriptor { entities, skybox: None };
            match ron::to_string(&scene) {
                Ok(ron_str) => match std::fs::write(&path, &ron_str) {
                    Ok(()) => McpToolOutput::success(json!({
                        "status": "saved",
                        "path": path,
                        "entity_count": count,
                    })),
                    Err(e) => McpToolOutput::error(&format!("write failed: {e}")),
                },
                Err(e) => McpToolOutput::error(&format!("serialize failed: {e}")),
            }
        }),
    });

    // prefab_write
    let snap_pw = snapshot.clone();
    let project_dir_pw = project_dir.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "prefab_write".to_string(),
        description: "Extract an entity and its descendants from the live scene into a \
            new assets/prefabs/<name>.ron file. Auto-suffixes the filename (#2, #3, ...) \
            if it already exists."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer", "description": "Root entity id; itself and all descendants are saved" },
                "name": { "type": "string", "description": "Prefab file name, no extension or directory (written to assets/prefabs/<name>.ron)" },
            },
            "required": ["entity_id", "name"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing 'entity_id' field"),
            };
            let name = match input["name"].as_str() {
                Some(n) => n.to_string(),
                None => return McpToolOutput::error("missing 'name' field"),
            };
            let s = snap_pw.lock().unwrap();
            match save_entities_as_prefab(&s.entities, entity_id, &name, project_dir_pw.as_ref()) {
                Ok(path) => McpToolOutput::success(json!({ "status": "saved", "path": path })),
                Err(e) => McpToolOutput::error(&e),
            }
        }),
    });

    // apply_to_prefab
    let queue_atp = prefab_apply_cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "apply_to_prefab".to_string(),
        description: "Push this prefab instance's field-level overrides back into its source .ron \
            file. Structural changes (added/removed entities) are not promoted. Queued for \
            processing; check the instance's state on a subsequent call to confirm the result."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer", "description": "The prefab instance's root entity id" },
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing 'entity_id' field"),
            };
            queue_atp
                .lock()
                .unwrap()
                .push(crate::snapshot::PrefabApplyCommand { entity_id });
            McpToolOutput::success(json!({ "status": "queued" }))
        }),
    });

    // terrain_write
    let queue_terrain = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "terrain_write".to_string(),
        description: "Spawn a new terrain entity from a heightmap (applied next frame). \
            The existing terrain system loads the heightmap and spawns chunk children \
            (render mesh + heightfield collider) automatically."
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "heightmap_path": { "type": "string", "description": "Path to the heightmap asset (16-bit grayscale PNG)" },
                "chunk_count": { "type": "array", "items": {"type": "integer"}, "minItems": 2, "maxItems": 2, "description": "[chunks_x, chunks_z]" },
                "chunk_size": { "type": "number", "description": "World-space size of one chunk along each axis" },
                "height_scale": { "type": "number", "description": "Multiplier applied to the normalized heightmap sample" },
                "layer0_texture_path": { "type": "string", "description": "Diffuse texture for the low/flat splat layer (e.g. grass)" },
                "layer1_texture_path": { "type": "string", "description": "Diffuse texture for the steep-slope splat layer (e.g. rock)" },
                "layer2_texture_path": { "type": "string", "description": "Diffuse texture for the paint-only splat layer (e.g. dirt)" },
                "layer3_texture_path": { "type": "string", "description": "Diffuse texture for the high-altitude splat layer (e.g. snow)" },
                "splatmap_path": { "type": "string", "description": "Optional path to a whole-terrain RGBA8 splatmap image; omit to keep procedural splat generation" }
            },
            "required": ["heightmap_path", "chunk_count", "chunk_size", "height_scale", "layer0_texture_path", "layer1_texture_path", "layer2_texture_path", "layer3_texture_path"]
        })),
        handler: Box::new(move |input| {
            let heightmap_path = match input["heightmap_path"].as_str() {
                Some(p) => p.to_string(),
                None => return McpToolOutput::error("missing 'heightmap_path' field"),
            };
            let chunk_count = match input["chunk_count"].as_array() {
                Some(arr) if arr.len() == 2 => {
                    match (arr[0].as_u64(), arr[1].as_u64()) {
                        (Some(x), Some(z)) => (x as u32, z as u32),
                        _ => return McpToolOutput::error(
                            "'chunk_count' must be an array of 2 integers",
                        ),
                    }
                }
                _ => return McpToolOutput::error(
                    "'chunk_count' must be an array of 2 integers",
                ),
            };
            let chunk_size = match input["chunk_size"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing numeric 'chunk_size' field"),
            };
            let height_scale = match input["height_scale"].as_f64() {
                Some(v) => v as f32,
                None => {
                    return McpToolOutput::error("missing numeric 'height_scale' field")
                }
            };
            let layer0_texture_path = match input["layer0_texture_path"].as_str() {
                Some(p) => p.to_string(),
                None => {
                    return McpToolOutput::error("missing 'layer0_texture_path' field")
                }
            };
            let layer1_texture_path = match input["layer1_texture_path"].as_str() {
                Some(p) => p.to_string(),
                None => {
                    return McpToolOutput::error("missing 'layer1_texture_path' field")
                }
            };
            let layer2_texture_path = match input["layer2_texture_path"].as_str() {
                Some(p) => p.to_string(),
                None => {
                    return McpToolOutput::error("missing 'layer2_texture_path' field")
                }
            };
            let layer3_texture_path = match input["layer3_texture_path"].as_str() {
                Some(p) => p.to_string(),
                None => {
                    return McpToolOutput::error("missing 'layer3_texture_path' field")
                }
            };
            let splatmap_path = input["splatmap_path"].as_str().map(|s| s.to_string());
            queue_terrain.lock().unwrap().push(EditorCommand::SpawnTerrain {
                heightmap_path,
                chunk_count,
                chunk_size,
                height_scale,
                layer0_texture_path,
                layer1_texture_path,
                layer2_texture_path,
                layer3_texture_path,
                splatmap_path,
            });
            McpToolOutput::success(json!({"status": "queued"}))
        }),
    });

    // load_scene
    let queue4 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "load_scene".to_string(),
        description: "Load and spawn entities from a RON scene file (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Source file path (.ron)" } },
            "required": ["path"]
        })),
        handler: Box::new(move |input| {
            let path = match input["path"].as_str() {
                Some(p) => p.to_string(),
                None => return McpToolOutput::error("missing 'path' field"),
            };
            queue4
                .lock()
                .unwrap()
                .push(EditorCommand::LoadScene(path.clone()));
            McpToolOutput::success(json!({"status": "queued", "path": path}))
        }),
    });

    // clear_scene
    let queue15 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "clear_scene".to_string(),
        description: "Despawn all entities in the scene (applied next frame)".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            queue15.lock().unwrap().push(EditorCommand::ClearScene);
            McpToolOutput::success(json!({"status": "queued"}))
        }),
    });
}
