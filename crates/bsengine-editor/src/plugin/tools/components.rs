//! MCP tools: Meshes, physics bodies, lights and cameras on an entity.

use super::*;

/// Registers this group's tools; see [`super::register_all`].
pub(super) fn register(mcp: &McpRegistryResource, cx: &ToolContext) {
    let ToolContext {
        snapshot,
        cmd_queue,
        ..
    } = cx;

    // attach_mesh
    let queue5 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "attach_mesh".to_string(),
        description: "Attach a MeshRenderer to an entity by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" },
                "mesh_id":   { "type": "number", "description": "Registered mesh ID" }
            },
            "required": ["entity_id", "mesh_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            let mesh_id = match input["mesh_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'mesh_id' field"),
            };
            queue5
                .lock()
                .unwrap()
                .push(EditorCommand::AttachMeshRenderer { entity_id, mesh_id });
            McpToolOutput::success(
                json!({"status": "queued", "entity_id": entity_id, "mesh_id": mesh_id}),
            )
        }),
    });

    // detach_mesh
    let queue6 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "detach_mesh".to_string(),
        description: "Remove MeshRenderer from an entity by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            queue6
                .lock()
                .unwrap()
                .push(EditorCommand::DetachMeshRenderer { entity_id });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // attach_physics_body
    let queue_phys_attach = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "attach_physics_body".to_string(),
        description: "Attach a physics body (rigidbody + collider) to an entity by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id":     { "type": "number", "description": "Entity ID" },
                "rigidbody":     { "type": "string", "enum": ["Dynamic", "Static", "Kinematic"] },
                "collider_shape":{ "type": "string", "enum": ["Box", "Sphere", "Capsule"] },
                "hx":            { "type": "number", "description": "Box half-extent X" },
                "hy":            { "type": "number", "description": "Box half-extent Y" },
                "hz":            { "type": "number", "description": "Box half-extent Z" },
                "radius":        { "type": "number", "description": "Sphere/Capsule radius" },
                "half_height":   { "type": "number", "description": "Capsule half-height" },
                "restitution":   { "type": "number", "description": "Bounciness, 0-1 (default 0.0)" },
                "friction":      { "type": "number", "description": "Surface friction (default 0.5)" },
                "sensor":        { "type": "boolean", "description": "Overlap-only, no physical collision (default false)" }
            },
            "required": ["entity_id", "rigidbody", "collider_shape"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            let rigidbody = match input["rigidbody"].as_str() {
                Some("Dynamic") => bsengine_scene::RigidBodyDesc::Dynamic,
                Some("Static") => bsengine_scene::RigidBodyDesc::Static,
                Some("Kinematic") => bsengine_scene::RigidBodyDesc::Kinematic,
                _ => return McpToolOutput::error(
                    "'rigidbody' must be one of \"Dynamic\", \"Static\", \"Kinematic\"",
                ),
            };
            let shape = match input["collider_shape"].as_str() {
                Some("Box") => {
                    let hx = input["hx"].as_f64().unwrap_or(0.5) as f32;
                    let hy = input["hy"].as_f64().unwrap_or(0.5) as f32;
                    let hz = input["hz"].as_f64().unwrap_or(0.5) as f32;
                    bsengine_scene::ColliderShapeDesc::Box { hx, hy, hz }
                }
                Some("Sphere") => {
                    let radius = input["radius"].as_f64().unwrap_or(0.5) as f32;
                    bsengine_scene::ColliderShapeDesc::Sphere { radius }
                }
                Some("Capsule") => {
                    let half_height = input["half_height"].as_f64().unwrap_or(0.5) as f32;
                    let radius = input["radius"].as_f64().unwrap_or(0.3) as f32;
                    bsengine_scene::ColliderShapeDesc::Capsule { half_height, radius }
                }
                _ => return McpToolOutput::error(
                    "'collider_shape' must be one of \"Box\", \"Sphere\", \"Capsule\"",
                ),
            };
            let collider = bsengine_scene::ColliderDesc {
                shape,
                restitution: input["restitution"].as_f64().unwrap_or(0.0) as f32,
                friction: input["friction"].as_f64().unwrap_or(0.5) as f32,
                sensor: input["sensor"].as_bool().unwrap_or(false),
            };
            queue_phys_attach.lock().unwrap().push(EditorCommand::AttachPhysicsBody {
                entity_id,
                rigidbody,
                collider,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // detach_physics_body
    let queue_phys_detach = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "detach_physics_body".to_string(),
        description: "Remove an entity's physics body by ID (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            queue_phys_detach
                .lock()
                .unwrap()
                .push(EditorCommand::DetachPhysicsBody { entity_id });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // spawn_point_light
    let queue7 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "spawn_point_light".to_string(),
        description: "Spawn a point light entity at a position (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                "intensity": { "type": "number" },
                "range":     { "type": "number" },
                "position":  { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
            },
            "required": ["color", "intensity", "range", "position"]
        })),
        handler: Box::new(move |input| {
            let color = parse_vec3_input(&input["color"]).unwrap_or([1.0, 1.0, 1.0]);
            let intensity = input["intensity"].as_f64().unwrap_or(1.0) as f32;
            let range = input["range"].as_f64().unwrap_or(10.0) as f32;
            let position = parse_vec3_input(&input["position"]).unwrap_or([0.0, 0.0, 0.0]);
            queue7.lock().unwrap().push(EditorCommand::SpawnPointLight {
                color,
                intensity,
                range,
                position,
            });
            McpToolOutput::success(json!({"status": "queued"}))
        }),
    });

    // spawn_directional_light
    let queue8 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "spawn_directional_light".to_string(),
        description: "Spawn a directional light (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "direction": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" },
                "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                "ambient":   { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" }
            }
        })),
        handler: Box::new(move |input| {
            let direction = parse_vec3_input(&input["direction"]).unwrap_or([-0.4, -0.8, -0.4]);
            let color = parse_vec3_input(&input["color"]).unwrap_or([1.0, 1.0, 1.0]);
            let ambient = parse_vec3_input(&input["ambient"]).unwrap_or([0.15, 0.15, 0.15]);
            queue8.lock().unwrap().push(EditorCommand::SpawnDirectionalLight {
                direction,
                color,
                ambient,
            });
            McpToolOutput::success(json!({"status": "queued"}))
        }),
    });

    // remove_light
    let queue9 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "remove_light".to_string(),
        description: "Remove all light components from an entity by ID (applied next frame)"
            .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            queue9
                .lock()
                .unwrap()
                .push(EditorCommand::RemoveLight { entity_id });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // update_point_light
    let queue10 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "update_point_light".to_string(),
        description: "Update PointLight properties on an entity (all fields optional, applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" },
                "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                "intensity": { "type": "number" },
                "range":     { "type": "number" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            let color = parse_vec3_input(&input["color"]);
            let intensity = input["intensity"].as_f64().map(|v| v as f32);
            let range = input["range"].as_f64().map(|v| v as f32);
            queue10.lock().unwrap().push(EditorCommand::UpdatePointLight {
                entity_id,
                color,
                intensity,
                range,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // update_directional_light
    let queue11 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "update_directional_light".to_string(),
        description: "Update DirectionalLight properties on an entity (all fields optional, applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "number", "description": "Entity ID" },
                "direction": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" },
                "color":     { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                "ambient":   { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            let direction = parse_vec3_input(&input["direction"]);
            let color = parse_vec3_input(&input["color"]);
            let ambient = parse_vec3_input(&input["ambient"]);
            queue11.lock().unwrap().push(EditorCommand::UpdateDirectionalLight {
                entity_id,
                direction,
                color,
                ambient,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // spawn_spot_light
    let queue13 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "spawn_spot_light".to_string(),
        description: "Spawn a spot light entity at a position (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "color":       { "type": "array", "items": {"type":"number"}, "description": "[r,g,b]" },
                "intensity":   { "type": "number" },
                "range":       { "type": "number" },
                "inner_angle": { "type": "number", "description": "Inner cone half-angle in radians" },
                "outer_angle": { "type": "number", "description": "Outer cone half-angle in radians" },
                "position":    { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
            },
            "required": ["color", "intensity", "range", "inner_angle", "outer_angle", "position"]
        })),
        handler: Box::new(move |input| {
            let color = parse_vec3_input(&input["color"]).unwrap_or([1.0, 1.0, 1.0]);
            let intensity = input["intensity"].as_f64().unwrap_or(1.0) as f32;
            let range = input["range"].as_f64().unwrap_or(10.0) as f32;
            let inner_angle = input["inner_angle"].as_f64().unwrap_or(std::f64::consts::PI / 8.0) as f32;
            let outer_angle = input["outer_angle"].as_f64().unwrap_or(std::f64::consts::PI / 6.0) as f32;
            let position = parse_vec3_input(&input["position"]).unwrap_or([0.0, 0.0, 0.0]);
            queue13.lock().unwrap().push(EditorCommand::SpawnSpotLight {
                color,
                intensity,
                range,
                inner_angle,
                outer_angle,
                position,
            });
            McpToolOutput::success(json!({"status": "queued"}))
        }),
    });

    // update_spot_light
    let queue14 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "update_spot_light".to_string(),
        description:
            "Update SpotLight properties on an entity (all fields optional, applied next frame)"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id":   { "type": "number" },
                "color":       { "type": "array", "items": {"type":"number"} },
                "intensity":   { "type": "number" },
                "range":       { "type": "number" },
                "inner_angle": { "type": "number" },
                "outer_angle": { "type": "number" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(v) => v,
                None => return McpToolOutput::error("missing numeric 'entity_id' field"),
            };
            let color = parse_vec3_input(&input["color"]);
            let intensity = input["intensity"].as_f64().map(|v| v as f32);
            let range = input["range"].as_f64().map(|v| v as f32);
            let inner_angle = input["inner_angle"].as_f64().map(|v| v as f32);
            let outer_angle = input["outer_angle"].as_f64().map(|v| v as f32);
            queue14
                .lock()
                .unwrap()
                .push(EditorCommand::UpdateSpotLight {
                    entity_id,
                    color,
                    intensity,
                    range,
                    inner_angle,
                    outer_angle,
                });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // spawn_camera
    let queue17 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "spawn_camera".to_string(),
        description: "Spawn a perspective camera entity (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "fov_y_degrees": { "type": "number", "description": "Vertical field of view in degrees (default 60)" },
                "position": { "type": "array", "items": {"type":"number"}, "description": "[x,y,z]" }
            }
        })),
        handler: Box::new(move |input| {
            let fov_y_degrees = input["fov_y_degrees"].as_f64().unwrap_or(60.0) as f32;
            let position = parse_vec3_input(&input["position"]).unwrap_or([0.0, 0.0, 0.0]);
            queue17
                .lock()
                .unwrap()
                .push(EditorCommand::SpawnCamera { fov_y_degrees, position });
            McpToolOutput::success(json!({"status": "queued"}))
        }),
    });

    // update_camera
    let queue18 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "update_camera".to_string(),
        description: "Update a camera entity's field of view (applied next frame)".to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "fov_y_degrees": { "type": "number" }
            },
            "required": ["entity_id"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let fov_y_degrees = input["fov_y_degrees"].as_f64().map(|f| f as f32);
            queue18.lock().unwrap().push(EditorCommand::UpdateCamera {
                entity_id,
                fov_y_degrees,
            });
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // set_light_intensity
    let snap_sli = snapshot.clone();
    let queue74 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_light_intensity".to_string(),
        description:
            "Set the light intensity of a single light entity (point or spot); returns {status}"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "entity_id": { "type": "integer" },
                "intensity": { "type": "number" }
            },
            "required": ["entity_id", "intensity"]
        })),
        handler: Box::new(move |input| {
            let entity_id = match input["entity_id"].as_u64() {
                Some(id) => id,
                None => return McpToolOutput::error("missing entity_id"),
            };
            let intensity = match input["intensity"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing intensity"),
            };
            let light_type = {
                let s = snap_sli.lock().unwrap();
                s.entities
                    .iter()
                    .find(|e| e.id == entity_id)
                    .and_then(|e| e.light_type.clone())
            };
            let cmd = match light_type.as_deref() {
                Some("point") => EditorCommand::UpdatePointLight {
                    entity_id,
                    color: None,
                    intensity: Some(intensity),
                    range: None,
                },
                Some("spot") => EditorCommand::UpdateSpotLight {
                    entity_id,
                    color: None,
                    intensity: Some(intensity),
                    range: None,
                    inner_angle: None,
                    outer_angle: None,
                },
                _ => return McpToolOutput::error("entity has no point or spot light"),
            };
            queue74.lock().unwrap().push(cmd);
            McpToolOutput::success(json!({"status": "queued", "entity_id": entity_id}))
        }),
    });

    // set_all_lights_color
    let snap_salc = snapshot.clone();
    let queue75 = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "set_all_lights_color".to_string(),
        description:
            "Set the color [r,g,b] on every light entity in the scene; returns {affected_count}"
                .to_string(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {
                "r": { "type": "number" },
                "g": { "type": "number" },
                "b": { "type": "number" }
            },
            "required": ["r", "g", "b"]
        })),
        handler: Box::new(move |input| {
            let r = match input["r"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing r"),
            };
            let g = match input["g"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing g"),
            };
            let b = match input["b"].as_f64() {
                Some(v) => v as f32,
                None => return McpToolOutput::error("missing b"),
            };
            let color = [r, g, b];
            let lights: Vec<(u64, String)> = {
                let s = snap_salc.lock().unwrap();
                s.entities
                    .iter()
                    .filter_map(|e| e.light_type.as_ref().map(|t| (e.id, t.clone())))
                    .collect()
            };
            let count = lights.len() as u64;
            let mut q = queue75.lock().unwrap();
            for (entity_id, light_type) in lights {
                let cmd = match light_type.as_str() {
                    "point" => EditorCommand::UpdatePointLight {
                        entity_id,
                        color: Some(color),
                        intensity: None,
                        range: None,
                    },
                    "spot" => EditorCommand::UpdateSpotLight {
                        entity_id,
                        color: Some(color),
                        intensity: None,
                        range: None,
                        inner_angle: None,
                        outer_angle: None,
                    },
                    "directional" => EditorCommand::UpdateDirectionalLight {
                        entity_id,
                        direction: None,
                        color: Some(color),
                        ambient: None,
                    },
                    _ => continue,
                };
                q.push(cmd);
            }
            McpToolOutput::success(json!({"affected_count": count}))
        }),
    });

    // detach_all_meshes
    let snap_dam = snapshot.clone();
    let queue_dam = cmd_queue.clone();
    mcp.0.lock().unwrap().register(McpTool {
        name: "detach_all_meshes".to_string(),
        description: "Remove mesh renderers from all entities that have one (applied next frame)"
            .to_string(),
        input_schema: Some(json!({"type": "object", "properties": {}})),
        handler: Box::new(move |_input| {
            let s = snap_dam.lock().unwrap();
            let ids: Vec<u64> = s
                .entities
                .iter()
                .filter(|e| e.mesh_id.is_some())
                .map(|e| e.id)
                .collect();
            let count = ids.len() as u64;
            let mut q = queue_dam.lock().unwrap();
            for entity_id in ids {
                q.push(EditorCommand::DetachMeshRenderer { entity_id });
            }
            McpToolOutput::success(json!({"detached_count": count}))
        }),
    });
}
