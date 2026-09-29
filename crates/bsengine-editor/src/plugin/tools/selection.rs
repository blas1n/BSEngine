//! MCP tools: The selection, and operations applied to everything in it.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        selection,
        ..
    } = cx;

    // select_entity
    let sel1 = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "select_entity".to_string(),
        description: "Add an entity to the editor selection set (immediate)".to_string(),
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
            sel1.lock().unwrap().insert(entity_id);
            McpToolOutput::success(json!({"status": "selected", "entity_id": entity_id}))
        }),
    });

    // deselect_entity
    let sel2 = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "deselect_entity".to_string(),
        description: "Remove an entity from the editor selection set (immediate)".to_string(),
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
            sel2.lock().unwrap().remove(&entity_id);
            McpToolOutput::success(json!({"status": "deselected", "entity_id": entity_id}))
        }),
    });

    // get_selection
    let sel3 = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "get_selection".to_string(),
        description: "Return the list of currently selected entity IDs".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let ids: Vec<u64> = sel3.lock().unwrap().iter().copied().collect();
            McpToolOutput::success(json!({"selected_ids": ids}))
        }),
    });

    // clear_selection
    let sel4 = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "clear_selection".to_string(),
        description: "Clear all selected entities (immediate)".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            sel4.lock().unwrap().clear();
            McpToolOutput::success(json!({"status": "cleared"}))
        }),
    });

    // get_selected_entities
    let snap_gse = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "get_selected_entities".to_string(),
        description: "Return all currently selected entities; returns {entities}".to_string(),
        input_schema: Some(json!({"type": "object"})),
        handler: Box::new(move |_input| {
            let s = snap_gse.lock().unwrap();
            let entities: Vec<serde_json::Value> = s
                .entities
                .iter()
                .filter(|e| e.selected)
                .map(|e| json!({"id": e.id, "name": e.name, "selected": e.selected}))
                .collect();
            McpToolOutput::success(json!({"entities": entities}))
        }),
    });

    // select/deselect for no_name

    // toggle_entity_selection
    let sel_tes = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "toggle_entity_selection".to_string(),
        description: "Toggle the selection state of a single entity (select if unselected, deselect if selected); returns {selected}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "entity_id": { "type": "integer" } },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let target = match input["entity_id"].as_u64() {
                Some(id) => id, None => return McpToolOutput::error("missing entity_id"),
            };
            let mut sel = sel_tes.lock().unwrap();
            let selected = if sel.contains(&target) {
                sel.remove(&target); false
            } else {
                sel.insert(target); true
            };
            McpToolOutput::success(json!({"selected": selected}))
        }),
    });

    // shrink_selection_to_roots
    let snap_sstr = snapshot.clone();
    let sel_sstr = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "shrink_selection_to_roots".to_string(),
        description: "Keep only selected entities that have no selected ancestor (topmost selections); removes descendants from selection; returns {removed_count}".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_sstr.lock().unwrap();
            let sel_ids: Vec<u64> = sel_sstr.lock().unwrap().iter().copied().collect();
            let sel_set: std::collections::HashSet<u64> = sel_ids.iter().copied().collect();
            let mut to_remove: Vec<u64> = Vec::new();
            for &id in &sel_ids {
                // walk ancestors; if any ancestor is also selected, remove this entity
                let mut cur = id;
                let mut has_selected_ancestor = false;
                loop {
                    let p = s.entities.iter().find(|e| e.id == cur).and_then(|e| e.parent_id);
                    match p {
                        Some(pid) => {
                            if sel_set.contains(&pid) { has_selected_ancestor = true; break; }
                            cur = pid;
                        }
                        None => break,
                    }
                }
                if has_selected_ancestor { to_remove.push(id); }
            }
            let count = to_remove.len() as u64;
            let mut sel = sel_sstr.lock().unwrap();
            for id in to_remove { sel.remove(&id); }
            McpToolOutput::success(json!({"removed_count": count}))
        }),
    });

    // expand_selection_to_subtrees
    let snap_ests = snapshot.clone();
    let sel_ests = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "expand_selection_to_subtrees".to_string(),
        description: "For each selected entity, add its entire subtree (all descendants) to the selection; returns {added_count}".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_ests.lock().unwrap();
            let roots: Vec<u64> = sel_ests.lock().unwrap().iter().copied().collect();
            let mut to_add: Vec<u64> = Vec::new();
            for root in &roots {
                let mut queue = vec![*root];
                while let Some(cur) = queue.pop() {
                    let children: Vec<u64> = s.entities.iter()
                        .filter(|e| e.parent_id == Some(cur))
                        .map(|e| e.id).collect();
                    for child in children {
                        to_add.push(child);
                        queue.push(child);
                    }
                }
            }
            let count = to_add.len() as u64;
            let mut sel = sel_ests.lock().unwrap();
            for id in to_add { sel.insert(id); }
            McpToolOutput::success(json!({"added_count": count}))
        }),
    });

    // select/deselect/count for positive_x, negative_x, positive_y, negative_y, positive_z, negative_z

    // select/deselect/count for same_parent

    // untag_all_selected
    let sel_uas = selection.clone();
    let queue89 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "untag_all_selected".to_string(),
        description: "Remove a tag from every currently selected entity; returns {untagged_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "tag": { "type": "string" } },
            "required": ["tag"]
        })),
        handler: Box::new(move |input| {
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            let selected: Vec<u64> = sel_uas.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue89.lock().unwrap();
            for entity_id in selected {
                q.push(EditorCommand::UntagEntity {
                    entity_id,
                    tag: tag.clone(),
                });
            }
            McpToolOutput::success(json!({"untagged_count": count}))
        }),
    });

    // tag_all_selected
    let sel_tas = selection.clone();
    let queue88 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "tag_all_selected".to_string(),
        description: "Apply a tag to every currently selected entity; returns {tagged_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "tag": { "type": "string" } },
            "required": ["tag"]
        })),
        handler: Box::new(move |input| {
            let tag = match input["tag"].as_str() {
                Some(t) => t.to_string(),
                None => return McpToolOutput::error("missing tag"),
            };
            let selected: Vec<u64> = sel_tas.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue88.lock().unwrap();
            for entity_id in selected {
                q.push(EditorCommand::TagEntity {
                    entity_id,
                    tag: tag.clone(),
                });
            }
            McpToolOutput::success(json!({"tagged_count": count}))
        }),
    });

    // offset_selection_rotation
    let snap_osr = snapshot.clone();
    let sel_osr = selection.clone();
    let queue86 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "offset_selection_rotation".to_string(),
        description:
            "Add rotation offsets (degrees) to all selected entities; returns {rotated_count}"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "rx": { "type": "number" },
                "ry": { "type": "number" },
                "rz": { "type": "number" }
            },
            "required": ["rx", "ry", "rz"]
        })),
        handler: Box::new(move |input| {
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
            let selected = sel_osr.lock().unwrap().clone();
            let s = snap_osr.lock().unwrap();
            let updates: Vec<(u64, [f32; 3])> = s
                .entities
                .iter()
                .filter(|e| selected.contains(&e.id))
                .map(|e| {
                    let cur = e.rotation.unwrap_or([0.0, 0.0, 0.0]);
                    (e.id, [cur[0] + drx, cur[1] + dry, cur[2] + drz])
                })
                .collect();
            drop(s);
            let count = updates.len() as u64;
            let mut q = queue86.lock().unwrap();
            for (entity_id, [rx, ry, rz]) in updates {
                q.push(EditorCommand::SetRotation {
                    entity_id,
                    rx,
                    ry,
                    rz,
                });
            }
            McpToolOutput::success(json!({"rotated_count": count}))
        }),
    });

    // offset_selection_scale
    let snap_oss = snapshot.clone();
    let sel_oss = selection.clone();
    let queue87 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "offset_selection_scale".to_string(),
        description: "Add scale offsets to all selected entities; returns {scaled_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "sx": { "type": "number" },
                "sy": { "type": "number" },
                "sz": { "type": "number" }
            },
            "required": ["sx", "sy", "sz"]
        })),
        handler: Box::new(move |input| {
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
            let selected = sel_oss.lock().unwrap().clone();
            let s = snap_oss.lock().unwrap();
            let updates: Vec<(u64, [f32; 3])> = s
                .entities
                .iter()
                .filter(|e| selected.contains(&e.id))
                .map(|e| {
                    let cur = e.scale.unwrap_or([1.0, 1.0, 1.0]);
                    (e.id, [cur[0] + dsx, cur[1] + dsy, cur[2] + dsz])
                })
                .collect();
            drop(s);
            let count = updates.len() as u64;
            let mut q = queue87.lock().unwrap();
            for (entity_id, [sx, sy, sz]) in updates {
                q.push(EditorCommand::SetScale {
                    entity_id,
                    sx,
                    sy,
                    sz,
                });
            }
            McpToolOutput::success(json!({"scaled_count": count}))
        }),
    });

    // offset_selection_position
    let snap_osp = snapshot.clone();
    let sel_osp = selection.clone();
    let queue84 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "offset_selection_position".to_string(),
        description: "Add position offsets to all selected entities; returns {moved_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "dx": { "type": "number" },
                "dy": { "type": "number" },
                "dz": { "type": "number" }
            },
            "required": ["dx", "dy", "dz"]
        })),
        handler: Box::new(move |input| {
            let dx = match input["dx"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing dx"),
            };
            let dy = match input["dy"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing dy"),
            };
            let dz = match input["dz"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing dz"),
            };
            let selected = sel_osp.lock().unwrap().clone();
            let s = snap_osp.lock().unwrap();
            let moves: Vec<(u64, [f32; 3])> = s
                .entities
                .iter()
                .filter(|e| selected.contains(&e.id))
                .map(|e| {
                    let cur = e.position.unwrap_or([0.0, 0.0, 0.0]);
                    (e.id, [cur[0] + dx, cur[1] + dy, cur[2] + dz])
                })
                .collect();
            drop(s);
            let count = moves.len() as u64;
            let mut q = queue84.lock().unwrap();
            for (entity_id, [x, y, z]) in moves {
                q.push(EditorCommand::SetPosition { entity_id, x, y, z });
            }
            McpToolOutput::success(json!({"moved_count": count}))
        }),
    });

    // scale_selection_uniformly
    let snap_ssu = snapshot.clone();
    let sel_ssu = selection.clone();
    let queue85 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "scale_selection_uniformly".to_string(),
        description: "Apply a uniform scale to all selected entities; returns {scaled_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "scale": { "type": "number" } },
            "required": ["scale"]
        })),
        handler: Box::new(move |input| {
            let s_val = match input["scale"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing scale"),
            };
            let selected = sel_ssu.lock().unwrap().clone();
            let s = snap_ssu.lock().unwrap();
            let ids: Vec<u64> = s
                .entities
                .iter()
                .filter(|e| selected.contains(&e.id))
                .map(|e| e.id)
                .collect();
            drop(s);
            let count = ids.len() as u64;
            let mut q = queue85.lock().unwrap();
            for entity_id in ids {
                q.push(EditorCommand::SetScale {
                    entity_id,
                    sx: s_val,
                    sy: s_val,
                    sz: s_val,
                });
            }
            McpToolOutput::success(json!({"scaled_count": count}))
        }),
    });

    // detach_selection_from_parent
    let sel_dsfp = selection.clone();
    let queue76 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "detach_selection_from_parent".to_string(),
        description:
            "Remove the parent from all selected entities (move to root); returns {affected_count}"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let selected: Vec<u64> = sel_dsfp.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue76.lock().unwrap();
            for id in selected {
                q.push(EditorCommand::RemoveParent { entity_id: id });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // reparent_selection
    let sel_rs = selection.clone();
    let queue77 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reparent_selection".to_string(),
        description: "Set a new parent for all selected entities; returns {affected_count}"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "parent_id": { "type": "integer" } },
            "required": ["parent_id"]
        })),
        handler: Box::new(move |input| {
            let parent_id = match input["parent_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing parent_id"),
            };
            let selected: Vec<u64> = sel_rs.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue77.lock().unwrap();
            for id in selected {
                q.push(EditorCommand::SetParent {
                    entity_id: id,
                    parent_id,
                });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // hide_all_selected
    let sel_has = selection.clone();
    let queue72 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "hide_all_selected".to_string(),
        description:
            "Hide all currently selected entities (set visible=false); returns {affected_count}"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let selected: Vec<u64> = sel_has.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue72.lock().unwrap();
            for id in selected {
                q.push(EditorCommand::SetVisible {
                    entity_id: id,
                    visible: false,
                });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // show_all_selected
    let sel_sas = selection.clone();
    let queue73 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "show_all_selected".to_string(),
        description:
            "Show all currently selected entities (set visible=true); returns {affected_count}"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let selected: Vec<u64> = sel_sas.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue73.lock().unwrap();
            for id in selected {
                q.push(EditorCommand::SetVisible {
                    entity_id: id,
                    visible: true,
                });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // move_selection_to_origin
    let sel_msto = selection.clone();
    let queue70 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "move_selection_to_origin".to_string(),
        description: "Move all selected entities to position [0,0,0]; returns {affected_count}"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let selected: Vec<u64> = sel_msto.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue70.lock().unwrap();
            for id in selected {
                q.push(EditorCommand::SetPosition {
                    entity_id: id,
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // set_selection_uniform_scale
    let sel_ssus = selection.clone();
    let queue71 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_selection_uniform_scale".to_string(),
        description: "Set all selected entities to a uniform scale (sx=sy=sz=scale); returns {affected_count}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "scale": { "type": "number" } },
            "required": ["scale"]
        })),
        handler: Box::new(move |input| {
            let scale = match input["scale"].as_f64() {
                Some(s) => s as f32,
                None => return McpToolOutput::error("missing scale"),
            };
            let selected: Vec<u64> = sel_ssus.lock().unwrap().iter().copied().collect();
            let count = selected.len() as u64;
            let mut q = queue71.lock().unwrap();
            for id in selected {
                q.push(EditorCommand::SetScale { entity_id: id, sx: scale, sy: scale, sz: scale });
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // are_all_selected_visible
    let snap_aasv = snapshot.clone();
    let sel_aasv = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "are_all_selected_visible".to_string(),
        description:
            "Return true if all currently selected entities are visible; returns {all_visible}"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let selected = sel_aasv.lock().unwrap().clone();
            if selected.is_empty() {
                return McpToolOutput::success(json!({"all_visible": true}));
            }
            let s = snap_aasv.lock().unwrap();
            let all_visible = selected.iter().all(|&id| {
                s.entities
                    .iter()
                    .find(|e| e.id == id)
                    .map(|e| e.visible)
                    .unwrap_or(false)
            });
            McpToolOutput::success(json!({"all_visible": all_visible}))
        }),
    });

    // select/deselect/count for directional_light, spot_light, point_light, light_type

    // align_entities_to_selection_pivot
    let snap_aetsp = snapshot.clone();
    let sel_aetsp = selection.clone();
    let queue68 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "align_entities_to_selection_pivot".to_string(),
        description: "Align all selected entities' position along the given axis to match the pivot entity; pivot_entity_id: entity whose axis value is the reference; axis: 'x'|'y'|'z'; returns {affected_count}".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "pivot_entity_id": { "type": "integer" },
                "axis": { "type": "string", "enum": ["x", "y", "z"] }
            },
            "required": ["pivot_entity_id", "axis"]
        })),
        handler: Box::new(move |input| {
            let axis = input["axis"].as_str().unwrap_or("x");
            let pivot_id = match input["pivot_entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("pivot_entity_id required"),
            };
            let selected: Vec<u64> = sel_aetsp.lock().unwrap().iter().cloned().collect();
            if selected.is_empty() {
                return McpToolOutput::success(json!({"affected_count": 0}));
            }
            let s = snap_aetsp.lock().unwrap();
            let pivot_pos = match s.entities.iter().find(|e| e.id == pivot_id).and_then(|e| e.position) {
                Some(p) => p,
                None => return McpToolOutput::error("pivot entity has no position"),
            };
            let axis_idx = match axis { "y" => 1, "z" => 2, _ => 0 };
            let mut affected = 0u64;
            let mut q = queue68.lock().unwrap();
            for &id in selected.iter().filter(|&&id| id != pivot_id) {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(mut pos) = e.position {
                        pos[axis_idx] = pivot_pos[axis_idx];
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id,
                            x: pos[0],
                            y: pos[1],
                            z: pos[2],
                        });
                        affected += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"affected_count": affected}))
        }),
    });

    // rename_selection_replace
    let snap_rsr2 = snapshot.clone();
    let sel_rsr2 = selection.clone();
    let queue66 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "rename_selection_replace".to_string(),
        description: "For each selected entity, replace all occurrences of 'from' with 'to' in its name; returns renamed_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "from": { "type": "string" },
                "to":   { "type": "string" }
            },
            "required": ["from", "to"]
        })),
        handler: Box::new(move |input| {
            let from = input["from"].as_str().unwrap_or("").to_string();
            let to   = input["to"].as_str().unwrap_or("").to_string();
            let s = snap_rsr2.lock().unwrap();
            let sel = sel_rsr2.lock().unwrap();
            let mut q = queue66.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(name) = &e.name {
                        let new_name = name.replace(&from[..], &to[..]);
                        if new_name != *name {
                            q.push(crate::snapshot::EditorCommand::RenameEntity { entity_id: id, name: new_name });
                            count += 1;
                        }
                    }
                }
            }
            McpToolOutput::success(json!({"renamed_count": count}))
        }),
    });

    // select/deselect/count for duplicate_names, empty_name

    // distribute_selection_along_y
    let snap_dsay = snapshot.clone();
    let sel_dsay = selection.clone();
    let queue65 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "distribute_selection_along_y".to_string(),
        description: "Distribute selected entities evenly along Y axis with given spacing, sorted by current Y; returns distributed_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "spacing": { "type": "number" } },
            "required": ["spacing"]
        })),
        handler: Box::new(move |input| {
            let spacing = input["spacing"].as_f64().unwrap_or(1.0) as f32;
            let s = snap_dsay.lock().unwrap();
            let sel = sel_dsay.lock().unwrap();
            let mut q = queue65.lock().unwrap();
            let mut entries: Vec<(u64, [f32; 3])> = sel.iter()
                .filter_map(|&id| s.entities.iter().find(|e| e.id == id))
                .filter_map(|e| e.position.map(|p| (e.id, p)))
                .collect();
            entries.sort_by(|a, b| a.1[1].partial_cmp(&b.1[1]).unwrap_or(std::cmp::Ordering::Equal));
            let count = entries.len();
            for (i, (id, pos)) in entries.into_iter().enumerate() {
                let new_y = i as f32 * spacing;
                q.push(crate::snapshot::EditorCommand::SetPosition { entity_id: id, x: pos[0], y: new_y, z: pos[2] });
            }
            McpToolOutput::success(json!({"distributed_count": count}))
        }),
    });

    // distribute_selection_along_x
    let snap_dsax = snapshot.clone();
    let sel_dsax = selection.clone();
    let queue63 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "distribute_selection_along_x".to_string(),
        description: "Distribute selected entities evenly along X axis with given spacing, sorted by current X; returns distributed_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "spacing": { "type": "number" } },
            "required": ["spacing"]
        })),
        handler: Box::new(move |input| {
            let spacing = input["spacing"].as_f64().unwrap_or(1.0) as f32;
            let s = snap_dsax.lock().unwrap();
            let sel = sel_dsax.lock().unwrap();
            let mut q = queue63.lock().unwrap();
            let mut entries: Vec<(u64, [f32; 3])> = sel.iter()
                .filter_map(|&id| s.entities.iter().find(|e| e.id == id))
                .filter_map(|e| e.position.map(|p| (e.id, p)))
                .collect();
            entries.sort_by(|a, b| a.1[0].partial_cmp(&b.1[0]).unwrap_or(std::cmp::Ordering::Equal));
            let count = entries.len();
            for (i, (id, pos)) in entries.into_iter().enumerate() {
                let new_x = i as f32 * spacing;
                q.push(crate::snapshot::EditorCommand::SetPosition { entity_id: id, x: new_x, y: pos[1], z: pos[2] });
            }
            McpToolOutput::success(json!({"distributed_count": count}))
        }),
    });

    // distribute_selection_along_z
    let snap_dsaz = snapshot.clone();
    let sel_dsaz = selection.clone();
    let queue64 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "distribute_selection_along_z".to_string(),
        description: "Distribute selected entities evenly along Z axis with given spacing, sorted by current Z; returns distributed_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "spacing": { "type": "number" } },
            "required": ["spacing"]
        })),
        handler: Box::new(move |input| {
            let spacing = input["spacing"].as_f64().unwrap_or(1.0) as f32;
            let s = snap_dsaz.lock().unwrap();
            let sel = sel_dsaz.lock().unwrap();
            let mut q = queue64.lock().unwrap();
            let mut entries: Vec<(u64, [f32; 3])> = sel.iter()
                .filter_map(|&id| s.entities.iter().find(|e| e.id == id))
                .filter_map(|e| e.position.map(|p| (e.id, p)))
                .collect();
            entries.sort_by(|a, b| a.1[2].partial_cmp(&b.1[2]).unwrap_or(std::cmp::Ordering::Equal));
            let count = entries.len();
            for (i, (id, pos)) in entries.into_iter().enumerate() {
                let new_z = i as f32 * spacing;
                q.push(crate::snapshot::EditorCommand::SetPosition { entity_id: id, x: pos[0], y: pos[1], z: new_z });
            }
            McpToolOutput::success(json!({"distributed_count": count}))
        }),
    });

    // align_selection_to_y
    let snap_asty = snapshot.clone();
    let sel_asty = selection.clone();
    let queue61 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "align_selection_to_y".to_string(),
        description: "Set the Y position of all selected entities to y while preserving X and Z; returns aligned_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "y": { "type": "number" } },
            "required": ["y"]
        })),
        handler: Box::new(move |input| {
            let target_y = input["y"].as_f64().unwrap_or(0.0) as f32;
            let s = snap_asty.lock().unwrap();
            let sel = sel_asty.lock().unwrap();
            let mut q = queue61.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(pos) = e.position {
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id, x: pos[0], y: target_y, z: pos[2],
                        });
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"aligned_count": count}))
        }),
    });

    // align_selection_to_z
    let snap_astz = snapshot.clone();
    let sel_astz = selection.clone();
    let queue62 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "align_selection_to_z".to_string(),
        description: "Set the Z position of all selected entities to z while preserving X and Y; returns aligned_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "z": { "type": "number" } },
            "required": ["z"]
        })),
        handler: Box::new(move |input| {
            let target_z = input["z"].as_f64().unwrap_or(0.0) as f32;
            let s = snap_astz.lock().unwrap();
            let sel = sel_astz.lock().unwrap();
            let mut q = queue62.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(pos) = e.position {
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id, x: pos[0], y: pos[1], z: target_z,
                        });
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"aligned_count": count}))
        }),
    });

    // align_selection_to_x
    let snap_astx = snapshot.clone();
    let sel_astx = selection.clone();
    let queue60 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "align_selection_to_x".to_string(),
        description: "Set the X position of all selected entities to x while preserving Y and Z; returns aligned_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "x": { "type": "number" } },
            "required": ["x"]
        })),
        handler: Box::new(move |input| {
            let target_x = input["x"].as_f64().unwrap_or(0.0) as f32;
            let s = snap_astx.lock().unwrap();
            let sel = sel_astx.lock().unwrap();
            let mut q = queue60.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(pos) = e.position {
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id,
                            x: target_x,
                            y: pos[1],
                            z: pos[2],
                        });
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"aligned_count": count}))
        }),
    });

    // number_selection
    let snap_ns = snapshot.clone();
    let sel_ns = selection.clone();
    let queue59 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "number_selection".to_string(),
        description:
            "Rename selected entities to prefix_1, prefix_2, … in id order; returns renamed_count"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "prefix": { "type": "string" } },
            "required": ["prefix"]
        })),
        handler: Box::new(move |input| {
            let prefix = input["prefix"].as_str().unwrap_or("Entity").to_string();
            let s = snap_ns.lock().unwrap();
            let sel = sel_ns.lock().unwrap();
            let mut ids: Vec<u64> = sel
                .iter()
                .copied()
                .filter(|&id| s.entities.iter().any(|e| e.id == id))
                .collect();
            ids.sort();
            let mut q = queue59.lock().unwrap();
            let count = ids.len() as u64;
            for (i, entity_id) in ids.into_iter().enumerate() {
                q.push(crate::snapshot::EditorCommand::RenameEntity {
                    entity_id,
                    name: format!("{}_{}", prefix, i + 1),
                });
            }
            McpToolOutput::success(json!({"renamed_count": count}))
        }),
    });

    // reset_selection_scale
    let snap_rss = snapshot.clone();
    let sel_rss = selection.clone();
    let queue58 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_selection_scale".to_string(),
        description: "Set scale to (1,1,1) for all selected entities; returns reset_count"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_rss.lock().unwrap();
            let sel = sel_rss.lock().unwrap();
            let mut q = queue58.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if s.entities.iter().any(|e| e.id == id && e.scale.is_some()) {
                    q.push(crate::snapshot::EditorCommand::SetScale {
                        entity_id: id,
                        sx: 1.0,
                        sy: 1.0,
                        sz: 1.0,
                    });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"reset_count": count}))
        }),
    });

    // reset_selection_position
    let snap_rsp = snapshot.clone();
    let sel_rsp = selection.clone();
    let queue56 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_selection_position".to_string(),
        description: "Set position to (0,0,0) for all selected entities; returns reset_count"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_rsp.lock().unwrap();
            let sel = sel_rsp.lock().unwrap();
            let mut q = queue56.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if s.entities
                    .iter()
                    .any(|e| e.id == id && e.position.is_some())
                {
                    q.push(crate::snapshot::EditorCommand::SetPosition {
                        entity_id: id,
                        x: 0.0,
                        y: 0.0,
                        z: 0.0,
                    });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"reset_count": count}))
        }),
    });

    // reset_selection_rotation
    let snap_rsr = snapshot.clone();
    let sel_rsr = selection.clone();
    let queue57 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "reset_selection_rotation".to_string(),
        description: "Set rotation to (0,0,0) for all selected entities; returns reset_count"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_rsr.lock().unwrap();
            let sel = sel_rsr.lock().unwrap();
            let mut q = queue57.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if s.entities
                    .iter()
                    .any(|e| e.id == id && e.rotation.is_some())
                {
                    q.push(crate::snapshot::EditorCommand::SetRotation {
                        entity_id: id,
                        rx: 0.0,
                        ry: 0.0,
                        rz: 0.0,
                    });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"reset_count": count}))
        }),
    });

    // toggle_visibility_on_selection
    let snap_tvos = snapshot.clone();
    let sel_tvos = selection.clone();
    let queue55 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "toggle_visibility_on_selection".to_string(),
        description:
            "Toggle visibility (visible↔hidden) for all selected entities; returns toggled_count"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_tvos.lock().unwrap();
            let sel = sel_tvos.lock().unwrap();
            let mut q = queue55.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    q.push(crate::snapshot::EditorCommand::SetVisible {
                        entity_id: id,
                        visible: !e.visible,
                    });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"toggled_count": count}))
        }),
    });

    // select/deselect/count for child_count

    // expand_selection_to_children
    let snap_estc = snapshot.clone();
    let sel_estc = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "expand_selection_to_children".to_string(),
        description: "Add all direct children of currently selected entities to the selection; returns added_count".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_estc.lock().unwrap();
            let mut sel = sel_estc.lock().unwrap();
            let current: Vec<u64> = sel.iter().copied().collect();
            let current_set: std::collections::HashSet<u64> = current.iter().copied().collect();
            let children: Vec<u64> = s.entities.iter()
                .filter(|e| e.parent_id.is_some_and(|p| current_set.contains(&p)) && !current_set.contains(&e.id))
                .map(|e| e.id)
                .collect();
            let count = children.len() as u64;
            for id in children {
                sel.insert(id);
            }
            McpToolOutput::success(json!({"added_count": count}))
        }),
    });

    // replace_tag_on_selection
    let snap_rtos = snapshot.clone();
    let sel_rtos = selection.clone();
    let queue53 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "replace_tag_on_selection".to_string(),
        description: "On selected entities that have old_tag, remove it and add new_tag; returns replaced_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "old_tag": { "type": "string" },
                "new_tag": { "type": "string" }
            },
            "required": ["old_tag", "new_tag"]
        })),
        handler: Box::new(move |input| {
            let old_tag = input["old_tag"].as_str().unwrap_or("").to_string();
            let new_tag = input["new_tag"].as_str().unwrap_or("").to_string();
            let s = snap_rtos.lock().unwrap();
            let sel = sel_rtos.lock().unwrap();
            let mut q = queue53.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if e.tags.contains(&old_tag) {
                        q.push(crate::snapshot::EditorCommand::UntagEntity { entity_id: id, tag: old_tag.clone() });
                        q.push(crate::snapshot::EditorCommand::TagEntity { entity_id: id, tag: new_tag.clone() });
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"replaced_count": count}))
        }),
    });

    // clear_tags_on_selection
    let snap_ctos = snapshot.clone();
    let sel_ctos = selection.clone();
    let queue54 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "clear_tags_on_selection".to_string(),
        description: "Remove all tags from all selected entities; returns cleared_count (number of entities whose tags were cleared)".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_ctos.lock().unwrap();
            let sel = sel_ctos.lock().unwrap();
            let mut q = queue54.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if !e.tags.is_empty() {
                        for tag in &e.tags {
                            q.push(crate::snapshot::EditorCommand::UntagEntity { entity_id: id, tag: tag.clone() });
                        }
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"cleared_count": count}))
        }),
    });

    // add_prefix_to_selection
    let snap_apts = snapshot.clone();
    let sel_apts = selection.clone();
    let queue51 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "add_prefix_to_selection".to_string(),
        description: "Prepend prefix to the name of all selected entities; returns renamed_count"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "prefix": { "type": "string" } },
            "required": ["prefix"]
        })),
        handler: Box::new(move |input| {
            let prefix = input["prefix"].as_str().unwrap_or("").to_string();
            let s = snap_apts.lock().unwrap();
            let sel = sel_apts.lock().unwrap();
            let mut q = queue51.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    let new_name = format!("{}{}", prefix, e.name.as_deref().unwrap_or(""));
                    q.push(crate::snapshot::EditorCommand::RenameEntity {
                        entity_id: id,
                        name: new_name,
                    });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"renamed_count": count}))
        }),
    });

    // add_suffix_to_selection
    let snap_asts = snapshot.clone();
    let sel_asts = selection.clone();
    let queue52 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "add_suffix_to_selection".to_string(),
        description: "Append suffix to the name of all selected entities; returns renamed_count"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "suffix": { "type": "string" } },
            "required": ["suffix"]
        })),
        handler: Box::new(move |input| {
            let suffix = input["suffix"].as_str().unwrap_or("").to_string();
            let s = snap_asts.lock().unwrap();
            let sel = sel_asts.lock().unwrap();
            let mut q = queue52.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    let new_name = format!("{}{}", e.name.as_deref().unwrap_or(""), suffix);
                    q.push(crate::snapshot::EditorCommand::RenameEntity {
                        entity_id: id,
                        name: new_name,
                    });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"renamed_count": count}))
        }),
    });

    // clone_selection
    let sel_csel = selection.clone();
    let queue50 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "clone_selection".to_string(),
        description: "Duplicate all selected entities; returns duplicated_count".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let sel = sel_csel.lock().unwrap();
            let mut q = queue50.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::DuplicateEntity { entity_id });
            }
            McpToolOutput::success(json!({"duplicated_count": count}))
        }),
    });

    // center_selection
    let snap_cs = snapshot.clone();
    let sel_cs = selection.clone();
    let queue49 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "center_selection".to_string(),
        description:
            "Move all selected entities so their collective centroid is at the world origin (0,0,0)"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_cs.lock().unwrap();
            let sel = sel_cs.lock().unwrap();
            let selected_entities: Vec<&crate::snapshot::EntityInfo> = s
                .entities
                .iter()
                .filter(|e| sel.contains(&e.id) && e.position.is_some())
                .collect();
            if selected_entities.is_empty() {
                return McpToolOutput::success(json!({"centered_count": 0}));
            }
            let count = selected_entities.len() as f32;
            let cx = selected_entities
                .iter()
                .map(|e| e.position.unwrap()[0])
                .sum::<f32>()
                / count;
            let cy = selected_entities
                .iter()
                .map(|e| e.position.unwrap()[1])
                .sum::<f32>()
                / count;
            let cz = selected_entities
                .iter()
                .map(|e| e.position.unwrap()[2])
                .sum::<f32>()
                / count;
            let mut q = queue49.lock().unwrap();
            for e in &selected_entities {
                let pos = e.position.unwrap();
                q.push(crate::snapshot::EditorCommand::SetPosition {
                    entity_id: e.id,
                    x: pos[0] - cx,
                    y: pos[1] - cy,
                    z: pos[2] - cz,
                });
            }
            McpToolOutput::success(json!({"centered_count": selected_entities.len()}))
        }),
    });

    // select/deselect/count for exactly_one_child

    // align_selection_to_ground
    let snap_astg = snapshot.clone();
    let sel_astg = selection.clone();
    let queue48 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "align_selection_to_ground".to_string(),
        description: "Set Y position to 0 for all selected entities; returns aligned_count"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_astg.lock().unwrap();
            let sel = sel_astg.lock().unwrap();
            let mut q = queue48.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(pos) = e.position {
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id,
                            x: pos[0],
                            y: 0.0,
                            z: pos[2],
                        });
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"aligned_count": count}))
        }),
    });

    // set_selection_rotation
    let sel_ssr = selection.clone();
    let queue46 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_selection_rotation".to_string(),
        description: "Set rotation (degrees) for all selected entities; returns rotated_count"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "rx": { "type": "number" },
                "ry": { "type": "number" },
                "rz": { "type": "number" }
            },
            "required": ["rx", "ry", "rz"]
        })),
        handler: Box::new(move |input| {
            let rx = input["rx"].as_f64().unwrap_or(0.0) as f32;
            let ry = input["ry"].as_f64().unwrap_or(0.0) as f32;
            let rz = input["rz"].as_f64().unwrap_or(0.0) as f32;
            let sel = sel_ssr.lock().unwrap();
            let mut q = queue46.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::SetRotation {
                    entity_id,
                    rx,
                    ry,
                    rz,
                });
            }
            McpToolOutput::success(json!({"rotated_count": count}))
        }),
    });

    // set_selection_scale
    let sel_sss = selection.clone();
    let queue47 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_selection_scale".to_string(),
        description: "Set scale for all selected entities; returns scaled_count".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "sx": { "type": "number" },
                "sy": { "type": "number" },
                "sz": { "type": "number" }
            },
            "required": ["sx", "sy", "sz"]
        })),
        handler: Box::new(move |input| {
            let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
            let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
            let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
            let sel = sel_sss.lock().unwrap();
            let mut q = queue47.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::SetScale {
                    entity_id,
                    sx,
                    sy,
                    sz,
                });
            }
            McpToolOutput::success(json!({"scaled_count": count}))
        }),
    });

    // select/deselect/count for non_default_scale

    // snap_selection_to_grid
    let snap_sstg = snapshot.clone();
    let sel_sstg = selection.clone();
    let queue45 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "snap_selection_to_grid".to_string(),
        description: "Snap the position of all selected entities to the nearest grid cell"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "grid_size": { "type": "number" } },
            "required": ["grid_size"]
        })),
        handler: Box::new(move |input| {
            let grid = input["grid_size"].as_f64().unwrap_or(1.0) as f32;
            if grid <= 0.0 {
                return McpToolOutput::error("grid_size must be positive");
            }
            let s = snap_sstg.lock().unwrap();
            let sel = sel_sstg.lock().unwrap();
            let mut q = queue45.lock().unwrap();
            let mut count = 0u64;
            for &id in sel.iter() {
                if let Some(e) = s.entities.iter().find(|e| e.id == id) {
                    if let Some(pos) = e.position {
                        let snap = |v: f32| (v / grid).round() * grid;
                        q.push(crate::snapshot::EditorCommand::SetPosition {
                            entity_id: id,
                            x: snap(pos[0]),
                            y: snap(pos[1]),
                            z: snap(pos[2]),
                        });
                        count += 1;
                    }
                }
            }
            McpToolOutput::success(json!({"snapped_count": count}))
        }),
    });

    // expand_selection_to_siblings
    let snap_ests = snapshot.clone();
    let sel_ests = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "expand_selection_to_siblings".to_string(),
        description:
            "Add all entities sharing the same parent as any selected entity to the selection"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_ests.lock().unwrap();
            let mut sel = sel_ests.lock().unwrap();
            let selected_ids: Vec<u64> = sel.iter().cloned().collect();
            let selected_parents: std::collections::HashSet<Option<u64>> = selected_ids
                .iter()
                .filter_map(|id| s.entities.iter().find(|e| e.id == *id))
                .map(|e| e.parent_id)
                .collect();
            let mut added = 0u64;
            for e in &s.entities {
                if selected_parents.contains(&e.parent_id) && !sel.contains(&e.id) {
                    sel.insert(e.id);
                    added += 1;
                }
            }
            McpToolOutput::success(json!({"added_count": added}))
        }),
    });

    // select/deselect/count for non_default_rotation

    // set_selection_parent
    let sel_ssp = selection.clone();
    let queue43 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_selection_parent".to_string(),
        description: "Set the parent of all selected entities to the given parent entity"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "parent_id": { "type": "integer" } },
            "required": ["parent_id"]
        })),
        handler: Box::new(move |input| {
            let parent_id = input["parent_id"].as_u64().unwrap_or(u64::MAX);
            let sel = sel_ssp.lock().unwrap();
            let mut q = queue43.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::SetParent {
                    entity_id,
                    parent_id,
                });
            }
            McpToolOutput::success(json!({"parented_count": count}))
        }),
    });

    // unparent_selection
    let sel_ups = selection.clone();
    let queue44 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "unparent_selection".to_string(),
        description: "Remove the parent from all selected entities (make them root-level)"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let sel = sel_ups.lock().unwrap();
            let mut q = queue44.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::RemoveParent { entity_id });
            }
            McpToolOutput::success(json!({"unparented_count": count}))
        }),
    });

    // select/deselect/count for scale_above, scale_below

    // rotate_selection_by
    let snap_rsb = snapshot.clone();
    let sel_rsb = selection.clone();
    let queue42 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "rotate_selection_by".to_string(),
        description: "Add Euler-angle delta (degrees) to the rotation of all selected entities"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "drx": { "type": "number" },
                "dry": { "type": "number" },
                "drz": { "type": "number" }
            },
            "required": ["drx", "dry", "drz"]
        })),
        handler: Box::new(move |input| {
            let drx = input["drx"].as_f64().unwrap_or(0.0) as f32;
            let dry = input["dry"].as_f64().unwrap_or(0.0) as f32;
            let drz = input["drz"].as_f64().unwrap_or(0.0) as f32;
            let sel = sel_rsb.lock().unwrap();
            let s = snap_rsb.lock().unwrap();
            let mut q = queue42.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                let current = s
                    .entities
                    .iter()
                    .find(|e| e.id == entity_id)
                    .and_then(|e| e.rotation)
                    .unwrap_or([0.0, 0.0, 0.0]);
                q.push(crate::snapshot::EditorCommand::SetRotation {
                    entity_id,
                    rx: current[0] + drx,
                    ry: current[1] + dry,
                    rz: current[2] + drz,
                });
            }
            McpToolOutput::success(json!({"rotated_count": count}))
        }),
    });

    // move_selection_by
    let sel_msb = selection.clone();
    let queue40 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "move_selection_by".to_string(),
        description: "Move all selected entities by (dx, dy, dz)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "dx": { "type": "number" },
                "dy": { "type": "number" },
                "dz": { "type": "number" }
            },
            "required": ["dx", "dy", "dz"]
        })),
        handler: Box::new(move |input| {
            let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
            let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
            let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
            let sel = sel_msb.lock().unwrap();
            let mut q = queue40.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::MoveEntity {
                    entity_id,
                    dx,
                    dy,
                    dz,
                });
            }
            McpToolOutput::success(json!({"moved_count": count}))
        }),
    });

    // scale_selection_by
    let snap_ssb = snapshot.clone();
    let sel_ssb = selection.clone();
    let queue41 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "scale_selection_by".to_string(),
        description: "Multiply the scale of all selected entities by (sx, sy, sz)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "sx": { "type": "number" },
                "sy": { "type": "number" },
                "sz": { "type": "number" }
            },
            "required": ["sx", "sy", "sz"]
        })),
        handler: Box::new(move |input| {
            let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
            let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
            let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
            let sel = sel_ssb.lock().unwrap();
            let s = snap_ssb.lock().unwrap();
            let mut q = queue41.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                let current_scale = s
                    .entities
                    .iter()
                    .find(|e| e.id == entity_id)
                    .and_then(|e| e.scale)
                    .unwrap_or([1.0, 1.0, 1.0]);
                q.push(crate::snapshot::EditorCommand::SetScale {
                    entity_id,
                    sx: current_scale[0] * sx,
                    sy: current_scale[1] * sy,
                    sz: current_scale[2] * sz,
                });
            }
            McpToolOutput::success(json!({"scaled_count": count}))
        }),
    });

    // add_tag_to_selection
    let sel_atts = selection.clone();
    let queue38 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "add_tag_to_selection".to_string(),
        description: "Add a tag to all currently selected entities".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "tag": { "type": "string" } },
            "required": ["tag"]
        })),
        handler: Box::new(move |input| {
            let tag = input["tag"].as_str().unwrap_or("").to_string();
            let sel = sel_atts.lock().unwrap();
            let mut q = queue38.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::TagEntity {
                    entity_id,
                    tag: tag.clone(),
                });
            }
            McpToolOutput::success(json!({"tagged_count": count}))
        }),
    });

    // remove_tag_from_selection
    let sel_rtfs = selection.clone();
    let queue39 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "remove_tag_from_selection".to_string(),
        description: "Remove a tag from all currently selected entities".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "tag": { "type": "string" } },
            "required": ["tag"]
        })),
        handler: Box::new(move |input| {
            let tag = input["tag"].as_str().unwrap_or("").to_string();
            let sel = sel_rtfs.lock().unwrap();
            let mut q = queue39.lock().unwrap();
            let count = sel.len() as u64;
            for &entity_id in sel.iter() {
                q.push(crate::snapshot::EditorCommand::UntagEntity {
                    entity_id,
                    tag: tag.clone(),
                });
            }
            McpToolOutput::success(json!({"untagged_count": count}))
        }),
    });

    // offset_selected_positions
    let snap_osp = snapshot.clone();
    let sel_osp = selection.clone();
    let queue_osp = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "offset_selected_positions".to_string(),
        description: "Move all currently selected entities by (dx, dy, dz)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "dx": {"type": "number"},
                "dy": {"type": "number"},
                "dz": {"type": "number"}
            },
            "required": ["dx", "dy", "dz"]
        })),
        handler: Box::new(move |input| {
            let get = |k: &str| input[k].as_f64().map(|v| v as f32);
            let (dx, dy, dz) = match (get("dx"), get("dy"), get("dz")) {
                (Some(a), Some(b), Some(c)) => (a, b, c),
                _ => return McpToolOutput::error("missing dx/dy/dz"),
            };
            let ids: Vec<u64> = {
                let sel = sel_osp.lock().unwrap();
                sel.iter().copied().collect()
            };
            let existing: std::collections::HashSet<u64> = {
                let s = snap_osp.lock().unwrap();
                s.entities.iter().map(|e| e.id).collect()
            };
            let count = ids.len() as u64;
            let mut q = queue_osp.lock().unwrap();
            for id in ids {
                if existing.contains(&id) {
                    q.push(crate::snapshot::EditorCommand::MoveEntity {
                        entity_id: id,
                        dx,
                        dy,
                        dz,
                    });
                }
            }
            McpToolOutput::success(json!({"moved_count": count}))
        }),
    });

    // delete_selected_entities
    let sel_dse = selection.clone();
    let queue_dse = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "delete_selected_entities".to_string(),
        description: "Despawn all currently selected entities and clear the selection".to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let ids: Vec<u64> = {
                let s = sel_dse.lock().unwrap();
                s.iter().copied().collect()
            };
            let count = ids.len() as u64;
            {
                let mut q = queue_dse.lock().unwrap();
                for id in &ids {
                    q.push(crate::snapshot::EditorCommand::Despawn { entity_id: *id });
                }
            }
            sel_dse.lock().unwrap().clear();
            McpToolOutput::success(json!({"deleted_count": count}))
        }),
    });

    // mirror_selected_on_axis
    let snap_msoa = snapshot.clone();
    let sel_msoa = selection.clone();
    let queue_msoa = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "mirror_selected_on_axis".to_string(),
        description:
            "Negate the specified axis (x/y/z) of all selected entities (applied next frame)"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": { "axis": { "type": "string", "enum": ["x", "y", "z"] } },
            "required": ["axis"]
        })),
        handler: Box::new(move |input| {
            let axis = match input["axis"].as_str() {
                Some(a) => a.to_string(),
                None => return McpToolOutput::error("missing axis"),
            };
            let selected: Vec<u64> = sel_msoa.lock().unwrap().iter().cloned().collect();
            let s = snap_msoa.lock().unwrap();
            let mut q = queue_msoa.lock().unwrap();
            let mut count = 0u64;
            for &entity_id in &selected {
                if let Some(e) = s.entities.iter().find(|e| e.id == entity_id) {
                    let [mut x, mut y, mut z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                    match axis.as_str() {
                        "x" => x = -x,
                        "y" => y = -y,
                        "z" => z = -z,
                        _ => {}
                    }
                    q.push(EditorCommand::SetPosition { entity_id, x, y, z });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"mirrored_count": count, "axis": axis}))
        }),
    });

    // align_selected_on_axis
    let snap_asa = snapshot.clone();
    let sel_asa = selection.clone();
    let queue_asa = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "align_selected_on_axis".to_string(),
        description: "Set the specified axis (x/y/z) of all selected entities to the given value (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "axis":  { "type": "string", "enum": ["x", "y", "z"] },
                "value": { "type": "number" }
            },
            "required": ["axis", "value"]
        })),
        handler: Box::new(move |input| {
            let axis = match input["axis"].as_str() {
                Some(a) => a.to_string(),
                None => return McpToolOutput::error("missing axis"),
            };
            let value = input["value"].as_f64().unwrap_or(0.0) as f32;
            let selected: Vec<u64> = sel_asa.lock().unwrap().iter().cloned().collect();
            let s = snap_asa.lock().unwrap();
            let mut count = 0u64;
            let mut q = queue_asa.lock().unwrap();
            for &entity_id in &selected {
                if let Some(e) = s.entities.iter().find(|e| e.id == entity_id) {
                    let [mut x, mut y, mut z] = e.position.unwrap_or([0.0, 0.0, 0.0]);
                    match axis.as_str() {
                        "x" => x = value,
                        "y" => y = value,
                        "z" => z = value,
                        _ => {}
                    }
                    q.push(EditorCommand::SetPosition { entity_id, x, y, z });
                    count += 1;
                }
            }
            McpToolOutput::success(json!({"aligned_count": count, "axis": axis, "value": value}))
        }),
    });

    // translate_selected_entities
    let sel_tse = selection.clone();
    let queue_tse = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "translate_selected_entities".to_string(),
        description: "Move all selected entities by a delta offset (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "dx": { "type": "number" },
                "dy": { "type": "number" },
                "dz": { "type": "number" }
            },
            "required": ["dx", "dy", "dz"]
        })),
        handler: Box::new(move |input| {
            let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
            let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
            let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
            let ids: Vec<u64> = sel_tse.lock().unwrap().iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_tse.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::MoveEntity {
                    entity_id,
                    dx,
                    dy,
                    dz,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // invert_selection
    let snap_inv = snapshot.clone();
    let sel_inv = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "invert_selection".to_string(),
        description:
            "Invert the selection: deselect selected entities, select unselected ones (immediate)"
                .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_inv.lock().unwrap();
            let all_ids: Vec<u64> = s.entities.iter().map(|e| e.id).collect();
            drop(s);
            let mut sel = sel_inv.lock().unwrap();
            let mut new_sel = std::collections::HashSet::new();
            for id in all_ids {
                if !sel.contains(&id) {
                    new_sel.insert(id);
                }
            }
            *sel = new_sel;
            McpToolOutput::success(json!({"status": "ok", "count": sel.len()}))
        }),
    });

    // rotate_selected_entities
    let sel_rotate = selection.clone();
    let queue_rotate = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "rotate_selected_entities".to_string(),
        description: "Set rotation (Euler degrees) for all selected entities (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "rx": { "type": "number" },
                "ry": { "type": "number" },
                "rz": { "type": "number" }
            },
            "required": ["rx", "ry", "rz"]
        })),
        handler: Box::new(move |input| {
            let rx = input["rx"].as_f64().unwrap_or(0.0) as f32;
            let ry = input["ry"].as_f64().unwrap_or(0.0) as f32;
            let rz = input["rz"].as_f64().unwrap_or(0.0) as f32;
            let ids: Vec<u64> = sel_rotate.lock().unwrap().iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_rotate.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::SetRotation {
                    entity_id,
                    rx,
                    ry,
                    rz,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // scale_selected_entities
    let sel_scale = selection.clone();
    let queue_scale = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "scale_selected_entities".to_string(),
        description: "Set scale for all selected entities (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "sx": { "type": "number" },
                "sy": { "type": "number" },
                "sz": { "type": "number" }
            },
            "required": ["sx", "sy", "sz"]
        })),
        handler: Box::new(move |input| {
            let sx = input["sx"].as_f64().unwrap_or(1.0) as f32;
            let sy = input["sy"].as_f64().unwrap_or(1.0) as f32;
            let sz = input["sz"].as_f64().unwrap_or(1.0) as f32;
            let ids: Vec<u64> = sel_scale.lock().unwrap().iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_scale.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::SetScale {
                    entity_id,
                    sx,
                    sy,
                    sz,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // despawn_selected
    let sel_despawn = selection.clone();
    let queue_despawn = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "despawn_selected".to_string(),
        description: "Despawn all selected entities and clear selection (applied next frame)"
            .to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let mut sel = sel_despawn.lock().unwrap();
            let ids: Vec<u64> = sel.iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_despawn.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::Despawn { entity_id });
            }
            sel.clear();
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // hide_selected
    let sel_hide = selection.clone();
    let queue_hide = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "hide_selected".to_string(),
        description: "Hide all selected entities (visible=false, applied next frame)".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let ids: Vec<u64> = sel_hide.lock().unwrap().iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_hide.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::SetVisible {
                    entity_id,
                    visible: false,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // show_selected
    let sel_show = selection.clone();
    let queue_show = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "show_selected".to_string(),
        description: "Show all selected entities (visible=true, applied next frame)".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let ids: Vec<u64> = sel_show.lock().unwrap().iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_show.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::SetVisible {
                    entity_id,
                    visible: true,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // move_selected_entities
    let sel_move = selection.clone();
    let queue_move = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "move_selected_entities".to_string(),
        description: "Move all selected entities by (dx, dy, dz) (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "dx": { "type": "number" },
                "dy": { "type": "number" },
                "dz": { "type": "number" }
            },
            "required": ["dx", "dy", "dz"]
        })),
        handler: Box::new(move |input| {
            let dx = input["dx"].as_f64().unwrap_or(0.0) as f32;
            let dy = input["dy"].as_f64().unwrap_or(0.0) as f32;
            let dz = input["dz"].as_f64().unwrap_or(0.0) as f32;
            let ids: Vec<u64> = sel_move.lock().unwrap().iter().copied().collect();
            let count = ids.len();
            let mut queue = queue_move.lock().unwrap();
            for entity_id in ids {
                queue.push(EditorCommand::MoveEntity {
                    entity_id,
                    dx,
                    dy,
                    dz,
                });
            }
            McpToolOutput::success(json!({"status": "queued", "count": count}))
        }),
    });

    // select_all
    let sel5 = selection.clone();
    let snap_sel = snapshot.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "select_all".to_string(),
        description: "Select all entities in the scene (immediate)".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            let ids: Vec<u64> = snap_sel
                .lock()
                .unwrap()
                .entities
                .iter()
                .map(|e| e.id)
                .collect();
            let count = ids.len();
            let mut sel = sel5.lock().unwrap();
            for id in ids {
                sel.insert(id);
            }
            McpToolOutput::success(json!({"status": "selected", "count": count}))
        }),
    });

    // deselect_all
    let sel6 = selection.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "deselect_all".to_string(),
        description: "Deselect all entities (immediate)".to_string(),
        input_schema: Some(json!({ "type": "object" })),
        handler: Box::new(move |_input| {
            sel6.lock().unwrap().clear();
            McpToolOutput::success(json!({"status": "cleared"}))
        }),
    });
}
