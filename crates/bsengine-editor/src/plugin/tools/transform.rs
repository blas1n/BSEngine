//! MCP tools: Position, rotation and scale of one entity.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        selection,
        ..
    } = cx;

    // set_transform
    let queue2 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_transform".to_string(),
        description: "Set the world position of an entity by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "id": { "type": "number", "description": "Entity ID" },
                "x": { "type": "number" },
                "y": { "type": "number" },
                "z": { "type": "number" }
            },
            "required": ["id", "x", "y", "z"]
        })),
        handler: Box::new(move |input| {
            let id = match input["id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'id' field"),
            };
            let x = input["x"].as_f64().unwrap_or(0.0) as f32;
            let y = input["y"].as_f64().unwrap_or(0.0) as f32;
            let z = input["z"].as_f64().unwrap_or(0.0) as f32;
            queue2.lock().unwrap().push(EditorCommand::SetPosition {
                entity_id: id,
                x,
                y,
                z,
            });
            McpToolOutput::success(json!({"status": "queued", "id": id, "x": x, "y": y, "z": z}))
        }),
    });

    // move_entity
    let queue20 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "move_entity".to_string(),
        description: "Move an entity by a delta offset [dx, dy, dz] relative to its current position (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "dx": { "type": "number", "default": 0 },
                "dy": { "type": "number", "default": 0 },
                "dz": { "type": "number", "default": 0 }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
            let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
            let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
            queue20
                .lock()
                .unwrap()
                .push(EditorCommand::MoveEntity { entity_id, dx, dy, dz });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // set_rotation
    let queue21 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_rotation".to_string(),
        description:
            "Set an entity's rotation from Euler angles in degrees XYZ (applied next frame)"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "rx": { "type": "number", "default": 0 },
                "ry": { "type": "number", "default": 0 },
                "rz": { "type": "number", "default": 0 }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let rx = input["rx"].as_f64().unwrap_or(0.0) as f32;
            let ry = input["ry"].as_f64().unwrap_or(0.0) as f32;
            let rz = input["rz"].as_f64().unwrap_or(0.0) as f32;
            queue21.lock().unwrap().push(EditorCommand::SetRotation {
                entity_id,
                rx,
                ry,
                rz,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // set_scale
    let queue22 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_scale".to_string(),
        description: "Set an entity's uniform or non-uniform scale (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "sx": { "type": "number", "default": 1 },
                "sy": { "type": "number", "default": 1 },
                "sz": { "type": "number", "default": 1 }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
            let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
            let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
            queue22.lock().unwrap().push(EditorCommand::SetScale {
                entity_id,
                sx,
                sy,
                sz,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // set_entity_transform
    let queue29 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_entity_transform".to_string(),
        description: "Set position, rotation (degrees), and/or scale for an entity in one call (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "position": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 3, "maxItems": 3
                },
                "rotation": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 3, "maxItems": 3
                },
                "scale": {
                    "type": "array",
                    "items": { "type": "number" },
                    "minItems": 3, "maxItems": 3
                }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let parse = |v: &serde_json::Value| -> Option<[f32; 3]> {
                let a = v.as_array()?;
                Some([
                    a.first()?.as_f64()? as f32,
                    a.get(1)?.as_f64()? as f32,
                    a.get(2)?.as_f64()? as f32,
                ])
            };
            let position = parse(&input["position"]);
            let rotation = parse(&input["rotation"]);
            let scale = parse(&input["scale"]);
            queue29.lock().unwrap().push(EditorCommand::SetEntityTransform {
                entity_id,
                position,
                rotation,
                scale,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // offset_entity_rotation
    let snap_oer = snapshot.clone();
    let queue82 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "offset_entity_rotation".to_string(),
        description:
            "Add rotation offsets (degrees) to an entity's current rotation; returns {status}"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "rx": { "type": "number" },
                "ry": { "type": "number" },
                "rz": { "type": "number" }
            },
            "required": ["entity_id", "rx", "ry", "rz"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let drx = match input["rx"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing rx"),
            };
            let dry = match input["ry"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing ry"),
            };
            let drz = match input["rz"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing rz"),
            };
            let s = snap_oer.lock().unwrap();
            let cur = s
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .and_then(|e| e.rotation)
                .unwrap_or([0.0, 0.0, 0.0]);
            drop(s);
            queue82.lock().unwrap().push(EditorCommand::SetRotation {
                entity_id,
                rx: cur[0] + drx,
                ry: cur[1] + dry,
                rz: cur[2] + drz,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // offset_entity_scale
    let snap_oes = snapshot.clone();
    let queue83 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "offset_entity_scale".to_string(),
        description: "Add scale offsets to an entity's current scale; returns {status}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "sx": { "type": "number" },
                "sy": { "type": "number" },
                "sz": { "type": "number" }
            },
            "required": ["entity_id", "sx", "sy", "sz"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let dsx = match input["sx"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing sx"),
            };
            let dsy = match input["sy"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing sy"),
            };
            let dsz = match input["sz"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing sz"),
            };
            let s = snap_oes.lock().unwrap();
            let cur = s
                .entities
                .iter()
                .find(|e| e.id == entity_id)
                .and_then(|e| e.scale)
                .unwrap_or([1.0, 1.0, 1.0]);
            drop(s);
            queue83.lock().unwrap().push(EditorCommand::SetScale {
                entity_id,
                sx: cur[0] + dsx,
                sy: cur[1] + dsy,
                sz: cur[2] + dsz,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // scale_entity_uniformly
    let queue81 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "scale_entity_uniformly".to_string(),
        description: "Apply the same scale value on all three axes of an entity; returns {status}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "scale": { "type": "number" }
            },
            "required": ["entity_id", "scale"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let s = match input["scale"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing scale"),
            };
            queue81.lock().unwrap().push(EditorCommand::SetScale {
                entity_id,
                sx: s,
                sy: s,
                sz: s,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // reset_entity_scale
    let queue80 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_entity_scale".to_string(),
        description: "Reset a single entity's scale to [1,1,1]; returns {status}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            queue80.lock().unwrap().push(EditorCommand::SetScale {
                entity_id,
                sx: 1.0,
                sy: 1.0,
                sz: 1.0,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // reset_entity_position
    let queue78 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_entity_position".to_string(),
        description: "Reset a single entity's position to [0,0,0]; returns {status}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            queue78.lock().unwrap().push(EditorCommand::SetPosition {
                entity_id,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // reset_entity_rotation
    let queue79 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_entity_rotation".to_string(),
        description: "Reset a single entity's rotation to [0,0,0] degrees; returns {status}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            queue79.lock().unwrap().push(EditorCommand::SetRotation {
                entity_id,
                rx: 0.0,
                ry: 0.0,
                rz: 0.0,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // distribute_entities_evenly
    let snap_dee = snapshot.clone();
    let sel_dee = selection.clone();
    let queue69 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "distribute_entities_evenly".to_string(),
        description: "Distribute selected entities at equal intervals along the given axis (min/max preserved); axis: 'x'|'y'|'z'; returns {affected_count}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "axis": { "type": "string", "enum": ["x", "y", "z"] } },
            "required": ["axis"]
        })),
        handler: Box::new(move |input| {
            let axis = input["axis"].as_str().unwrap_or("x");
            let axis_idx = match axis { "y" => 1, "z" => 2, _ => 0 };
            let selected: Vec<u64> = sel_dee.lock().unwrap().iter().cloned().collect();
            let s = snap_dee.lock().unwrap();
            let mut items: Vec<(u64, [f32; 3])> = selected.iter()
                .filter_map(|&id| s.entities.iter().find(|e| e.id == id).and_then(|e| e.position).map(|p| (id, p)))
                .collect();
            if items.len() < 2 {
                return McpToolOutput::success(json!({"affected_count": 0}));
            }
            items.sort_by(|a, b| a.1[axis_idx].partial_cmp(&b.1[axis_idx]).unwrap_or(std::cmp::Ordering::Equal));
            let min_v = items.first().unwrap().1[axis_idx];
            let max_v = items.last().unwrap().1[axis_idx];
            let n = (items.len() - 1) as f32;
            let mut q = queue69.lock().unwrap();
            for (i, (id, mut pos)) in items.into_iter().enumerate() {
                pos[axis_idx] = min_v + (max_v - min_v) * (i as f32) / n;
                q.push(crate::snapshot::EditorCommand::SetPosition {
                    entity_id: id,
                    x: pos[0],
                    y: pos[1],
                    z: pos[2],
                });
            }
            McpToolOutput::success(json!({"affected_count": selected.len() as u64}))
        }),
    });

    // copy_transform_from_entity / paste_transform_to_selection (shared clipboard)
    let transform_clipboard: std::sync::Arc<Mutex<Option<([f32; 3], [f32; 3], [f32; 3])>>> =
        std::sync::Arc::new(Mutex::new(None));

    let snap_ctfe = snapshot.clone();
    let clipboard_copy = transform_clipboard.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "copy_transform_from_entity".to_string(),
        description: "Copy position, rotation, and scale from the given entity into the transform clipboard; returns {position, rotation, scale}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("entity_id required"),
            };
            let s = snap_ctfe.lock().unwrap();
            let e = match s.entities.iter().find(|e| e.id == id) {
                Some(e) => e,
                None => return McpToolOutput::error("entity not found"),
            };
            let pos = e.position.unwrap_or([0.0, 0.0, 0.0]);
            let rot = e.rotation.unwrap_or([0.0, 0.0, 0.0]);
            let sc  = e.scale.unwrap_or([1.0, 1.0, 1.0]);
            *clipboard_copy.lock().unwrap() = Some((pos, rot, sc));
            McpToolOutput::success(json!({
                "position": pos,
                "rotation": rot,
                "scale": sc,
            }))
        }),
    });

    let clipboard_paste = transform_clipboard.clone();
    let sel_ptts = selection.clone();
    let queue67 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "paste_transform_to_selection".to_string(),
        description: "Apply the copied transform (position, rotation, scale) to all currently selected entities; returns {affected_count}".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let (pos, rot, sc) = match *clipboard_paste.lock().unwrap() {
                Some(t) => t,
                None => return McpToolOutput::error("no transform in clipboard"),
            };
            let selected: Vec<u64> = sel_ptts.lock().unwrap().iter().cloned().collect();
            let count = selected.len() as u64;
            let mut q = queue67.lock().unwrap();
            for id in selected {
                q.push(crate::snapshot::EditorCommand::SetEntityTransform {
                    entity_id: id,
                    position: Some(pos),
                    rotation: Some(rot),
                    scale: Some(sc),
                });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // move_entity_to_origin
    let queue33 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "move_entity_to_origin".to_string(),
        description: "Move an entity to position [0, 0, 0]".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            queue33.lock().unwrap().push(EditorCommand::SetPosition {
                entity_id,
                x: 0.0,
                y: 0.0,
                z: 0.0,
            });
            McpToolOutput::success(json!({"entity_id": entity_id}))
        }),
    });

    // set_entity_scale_uniform
    let queue31 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_entity_scale_uniform".to_string(),
        description: "Set uniform scale (same value for X, Y, Z) on an entity".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "scale": { "type": "number" }
            },
            "required": ["entity_id", "scale"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let s = match input["scale"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing scale"),
            };
            queue31.lock().unwrap().push(EditorCommand::SetScale {
                entity_id,
                sx: s,
                sy: s,
                sz: s,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id, "scale": s}))
        }),
    });

    // copy_entity_transform
    let snap_cet = snapshot.clone();
    let queue_cet = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "copy_entity_transform".to_string(),
        description:
            "Copy the position, rotation, and scale from source_entity_id to target_entity_id"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "source_entity_id": { "type": "integer" },
                "target_entity_id": { "type": "integer" }
            },
            "required": ["source_entity_id", "target_entity_id"]
        })),
        handler: Box::new(move |input| {
            let src_id = match input["source_entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing source_entity_id"),
            };
            let tgt_id = match input["target_entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing target_entity_id"),
            };
            let (pos, rot, scale) = {
                let s = snap_cet.lock().unwrap();
                match s.entities.iter().find(|e| e.id == src_id) {
                    Some(e) => (e.position, e.rotation, e.scale),
                    None => return McpToolOutput::error("source entity not found"),
                }
            };
            let mut q = queue_cet.lock().unwrap();
            q.push(crate::snapshot::EditorCommand::SetEntityTransform {
                entity_id: tgt_id,
                position: pos,
                rotation: rot,
                scale,
            });
            McpToolOutput::success(json!({"source_entity_id": src_id, "target_entity_id": tgt_id}))
        }),
    });

    // snap_entity_to_grid
    let snap_setg = snapshot.clone();
    let queue_setg = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "snap_entity_to_grid".to_string(),
        description: "Round each axis of the entity's position to the nearest grid_size multiple"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "grid_size": { "type": "number" }
            },
            "required": ["entity_id", "grid_size"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let grid = match input["grid_size"].as_f64() {
                Some(g) if g > 0.0 => g as f32,
                _ => return McpToolOutput::error("grid_size must be a positive number"),
            };
            let s = snap_setg.lock().unwrap();
            match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => {
                    let [x, y, z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                    let snap = |v: f32| (v / grid).round() * grid;
                    let (sx, sy, sz) = (snap(x), snap(y), snap(z));
                    queue_setg
                        .lock()
                        .unwrap()
                        .push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id,
                            x: sx,
                            y: sy,
                            z: sz,
                        });
                    McpToolOutput::success(
                        json!({"entity_id": entity_id, "position": [sx, sy, sz]}),
                    )
                }
                None => McpToolOutput::error("entity not found"),
            }
        }),
    });

    // reset_entity_transform
    let queue_ret = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_entity_transform".to_string(),
        description: "Reset position to (0,0,0), rotation to (0,0,0), and scale to (1,1,1) (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let mut q = queue_ret.lock().unwrap();
            q.push(EditorCommand::SetPosition { entity_id, x: 0.0, y: 0.0, z: 0.0 });
            q.push(EditorCommand::SetRotation { entity_id, rx: 0.0, ry: 0.0, rz: 0.0 });
            q.push(EditorCommand::SetScale { entity_id, sx: 1.0, sy: 1.0, sz: 1.0 });
            McpToolOutput::success(json!({"entity_id": entity_id}))
        }),
    });

    // scale_entity_uniform
    let queue_seu = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "scale_entity_uniform".to_string(),
        description: "Set sx=sy=sz=factor for the entity (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "factor": { "type": "number" }
            },
            "required": ["entity_id", "factor"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let factor = input["factor"].as_f64().unwrap_or(1.0) as f32;
            queue_seu.lock().unwrap().push(EditorCommand::SetScale {
                entity_id,
                sx: factor,
                sy: factor,
                sz: factor,
            });
            McpToolOutput::success(json!({"entity_id": entity_id, "factor": factor}))
        }),
    });

    // reset_transform
    let queue_rt = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_transform".to_string(),
        description: "Reset position to [0,0,0], rotation to [0,0,0], and scale to [1,1,1] (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            queue_rt.lock().unwrap().push(EditorCommand::SetEntityTransform {
                entity_id,
                position: Some([0.0, 0.0, 0.0]),
                rotation: Some([0.0, 0.0, 0.0]),
                scale: Some([1.0, 1.0, 1.0]),
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // mirror_entity
    let snap_mir = snapshot.clone();
    let queue_mir = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "mirror_entity".to_string(),
        description:
            "Negate the entity's position along the given axis (x, y, or z), applied next frame"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "axis": { "type": "string", "enum": ["x", "y", "z"] }
            },
            "required": ["entity_id", "axis"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let axis = match input["axis"].as_str() {
                Some(a) => a.to_string(),
                None => return McpToolOutput::error("missing axis"),
            };
            let s = snap_mir.lock().unwrap();
            let pos = match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => match e.position {
                    Some(p) => p,
                    None => return McpToolOutput::error("entity has no position"),
                },
                None => return McpToolOutput::error("entity not found"),
            };
            drop(s);
            let mirrored = match axis.as_str() {
                "x" => [-pos[0], pos[1], pos[2]],
                "y" => [pos[0], -pos[1], pos[2]],
                "z" => [pos[0], pos[1], -pos[2]],
                _ => return McpToolOutput::error("axis must be x, y, or z"),
            };
            queue_mir
                .lock()
                .unwrap()
                .push(EditorCommand::SetEntityTransform {
                    entity_id,
                    position: Some(mirrored),
                    rotation: None,
                    scale: None,
                });
            McpToolOutput::success(json!({"status": "queued", "mirrored_position": mirrored}))
        }),
    });

    // snap_to_grid
    let snap_sg = snapshot.clone();
    let queue_sg = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "snap_to_grid".to_string(),
        description: "Round entity position to the nearest grid cell (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "grid_size": { "type": "number" }
            },
            "required": ["entity_id", "grid_size"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let grid = input["grid_size"].as_f64().unwrap_or(1.0) as f32;
            if grid <= 0.0 {
                return McpToolOutput::error("grid_size must be positive");
            }
            let s = snap_sg.lock().unwrap();
            let pos = match s.entities.iter().find(|e| e.id == entity_id) {
                Some(e) => match e.position {
                    Some(p) => p,
                    None => return McpToolOutput::error("entity has no position"),
                },
                None => return McpToolOutput::error("entity not found"),
            };
            drop(s);
            let snapped = [
                (pos[0] / grid).round() * grid,
                (pos[1] / grid).round() * grid,
                (pos[2] / grid).round() * grid,
            ];
            queue_sg
                .lock()
                .unwrap()
                .push(EditorCommand::SetEntityTransform {
                    entity_id,
                    position: Some(snapped),
                    rotation: None,
                    scale: None,
                });
            McpToolOutput::success(json!({"status": "queued", "snapped_position": snapped}))
        }),
    });

    // copy_transform
    let snap_ct = snapshot.clone();
    let queue_ct = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "copy_transform".to_string(),
        description:
            "Copy position, rotation, and scale from one entity to another (applied next frame)"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "from_entity_id": { "type": "integer" },
                "to_entity_id": { "type": "integer" }
            },
            "required": ["from_entity_id", "to_entity_id"]
        })),
        handler: Box::new(move |input| {
            let from_id = match input["from_entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing from_entity_id"),
            };
            let to_id = match input["to_entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing to_entity_id"),
            };
            let s = snap_ct.lock().unwrap();
            let src = match s.entities.iter().find(|e| e.id == from_id) {
                Some(e) => e.clone(),
                None => return McpToolOutput::error("source entity not found"),
            };
            if s.entities.iter().find(|e| e.id == to_id).is_none() {
                return McpToolOutput::error("target entity not found");
            }
            drop(s);
            queue_ct
                .lock()
                .unwrap()
                .push(EditorCommand::SetEntityTransform {
                    entity_id: to_id,
                    position: src.position,
                    rotation: src.rotation,
                    scale: src.scale,
                });
            McpToolOutput::success(json!({"status": "queued", "from": from_id, "to": to_id}))
        }),
    });
}
