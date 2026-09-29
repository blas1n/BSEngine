//! Mesh, physics, light and camera tools.

use super::*;

#[test]
fn mcp_attach_mesh_adds_mesh_renderer() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((
            Name("Cube".to_string()),
            Transform::from_position(Vec3::ZERO),
        ))
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "attach_mesh",
                json!({"entity_id": eid.index() as u64, "mesh_id": 42u64}),
            )
            .expect("attach_mesh not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let mut q = app
        .world_mut()
        .query::<(&Name, &bsengine_render::MeshRenderer)>();
    let found = q
        .iter(app.world())
        .any(|(n, m)| n.0 == "Cube" && m.mesh_id == 42);
    assert!(found, "MeshRenderer not attached");
}

#[test]
fn mcp_attach_physics_body_adds_physics_body_desc() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((
            Name("Box".to_string()),
            Transform::from_position(Vec3::ZERO),
        ))
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "attach_physics_body",
                json!({
                    "entity_id": eid.index() as u64,
                    "rigidbody": "Dynamic",
                    "collider_shape": "Sphere",
                    "radius": 0.75,
                }),
            )
            .expect("attach_physics_body not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let desc = app
        .world()
        .get::<bsengine_scene::PhysicsBodyDesc>(eid)
        .expect("PhysicsBodyDesc should be attached");
    assert_eq!(desc.rigidbody, bsengine_scene::RigidBodyDesc::Dynamic);
    match &desc.collider.shape {
        bsengine_scene::ColliderShapeDesc::Sphere { radius } => {
            assert!((radius - 0.75).abs() < 1e-5);
        }
        other => panic!("expected Sphere shape, got {other:?}"),
    }
}

#[test]
fn mcp_spawn_point_light_creates_entity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "spawn_point_light",
                json!({"color":[1.0,0.5,0.0],"intensity":2.0,"range":8.0,"position":[0.0,3.0,0.0]}),
            )
            .expect("spawn_point_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let mut q = app.world_mut().query::<&bsengine_core::PointLight>();
    let lights: Vec<_> = q.iter(app.world()).collect();
    assert_eq!(lights.len(), 1);
    assert!((lights[0].intensity - 2.0).abs() < 1e-4);
    assert!((lights[0].range - 8.0).abs() < 1e-4);
}

#[test]
fn mcp_spawn_directional_light_creates_entity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "spawn_directional_light",
                json!({"direction":[0.0,-1.0,0.0],"color":[1.0,1.0,1.0],"ambient":[0.1,0.1,0.1]}),
            )
            .expect("spawn_directional_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let mut q = app
        .world_mut()
        .query::<(&bsengine_core::DirectionalLight, &Transform)>();
    let (_, transform) = q
        .iter(app.world())
        .next()
        .expect("no DirectionalLight spawned");
    // direction lives on Transform.rotation (rotation * -Z), same as SpotLight.
    let derived_dir = transform.rotation.0 * Vec3::NEG_Z;
    assert!(
        (derived_dir - Vec3::new(0.0, -1.0, 0.0)).length() < 1e-4,
        "expected direction (0,-1,0), derived {:?}",
        derived_dir
    );
}

#[test]
fn mcp_update_directional_light_direction_rotates_transform() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    let eid = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "spawn_directional_light",
                json!({"direction":[0.0,-1.0,0.0],"color":[1.0,1.0,1.0],"ambient":[0.1,0.1,0.1]}),
            )
            .expect("spawn_directional_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
        app.update();
        let mut q = app
            .world_mut()
            .query::<(bevy_ecs::entity::Entity, &bsengine_core::DirectionalLight)>();
        q.iter(app.world()).next().unwrap().0.index() as u64
    };

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "update_directional_light",
                json!({"entity_id": eid, "direction": [1.0, 0.0, 0.0]}),
            )
            .expect("update_directional_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();
    app.update();

    let mut q = app.world_mut().query::<&Transform>();
    let transform = q
        .iter(app.world())
        .next()
        .expect("entity should still have a Transform");
    let derived_dir = transform.rotation.0 * Vec3::NEG_Z;
    assert!(
        (derived_dir - Vec3::new(1.0, 0.0, 0.0)).length() < 1e-4,
        "expected direction (1,0,0) after update, derived {:?}",
        derived_dir
    );
}

#[test]
fn mcp_remove_light_removes_point_light() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn(bsengine_core::PointLight::default())
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("remove_light", json!({"entity_id": eid.index() as u64}))
            .expect("remove_light not found");
    }
    app.update();

    let mut q = app.world_mut().query::<&bsengine_core::PointLight>();
    assert!(
        q.iter(app.world()).next().is_none(),
        "PointLight still present"
    );
}

#[test]
fn mcp_set_light_intensity_updates_point_light_intensity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "spawn_point_light",
                json!({
                    "color": [1.0, 1.0, 1.0],
                    "intensity": 100.0,
                    "range": 5.0,
                    "position": [0.0, 0.0, 0.0]
                }),
            )
            .unwrap();
    }
    app.update();
    app.update();

    let all = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone()
    };
    let light_id = all
        .iter()
        .find(|e| e["light_type"].as_str() == Some("point"))
        .unwrap()["id"]
        .as_u64()
        .unwrap();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "set_light_intensity",
                json!({"entity_id": light_id, "intensity": 500.0}),
            )
            .unwrap();
        assert!(out.is_ok());
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let updated = mcp
        .0
        .lock()
        .unwrap()
        .execute("list_entities", json!({}))
        .unwrap()
        .content["entities"]
        .as_array()
        .unwrap()
        .clone();
    let light = updated
        .iter()
        .find(|e| e["id"].as_u64().unwrap() == light_id)
        .unwrap();
    assert!(
        (light["light_intensity"].as_f64().unwrap() - 500.0).abs() < 0.1,
        "intensity should be 500"
    );
}

#[test]
fn mcp_set_all_lights_color_updates_all_light_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("spawn_point_light", json!({
            "color": [1.0, 1.0, 1.0], "intensity": 100.0, "range": 5.0, "position": [0.0, 0.0, 0.0]
        })).unwrap();
        m.execute("spawn_point_light", json!({
            "color": [0.5, 0.5, 0.5], "intensity": 200.0, "range": 8.0, "position": [1.0, 0.0, 0.0]
        })).unwrap();
        m.execute(
            "batch_spawn",
            json!({"entities": [{"name": "NoLight", "position": [2.0, 0.0, 0.0]}]}),
        )
        .unwrap();
    }
    app.update();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "set_all_lights_color",
                json!({"r": 1.0, "g": 0.0, "b": 0.0}),
            )
            .unwrap();
        assert!(out.is_ok());
        assert_eq!(out.content["affected_count"].as_u64().unwrap(), 2);
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let updated = mcp
        .0
        .lock()
        .unwrap()
        .execute("list_entities", json!({}))
        .unwrap()
        .content["entities"]
        .as_array()
        .unwrap()
        .clone();
    for e in &updated {
        if !e["light_type"].is_null() {
            let color = e["light_color"].as_array().unwrap();
            assert!(
                (color[0].as_f64().unwrap() - 1.0).abs() < 0.01,
                "r should be 1.0"
            );
            assert!((color[1].as_f64().unwrap()).abs() < 0.01, "g should be 0.0");
            assert!((color[2].as_f64().unwrap()).abs() < 0.01, "b should be 0.0");
        }
    }
}

#[test]
fn mcp_detach_all_meshes_removes_mesh_renderers() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "batch_spawn",
                json!({"entities": [
                    {"name": "DetachA"},
                    {"name": "DetachB"}
                ]}),
            )
            .unwrap();
    }
    app.update();
    app.update();

    let (id_a, id_b) = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let ents = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap()
            .content["entities"]
            .as_array()
            .unwrap()
            .clone();
        let a = ents
            .iter()
            .find(|e| e["name"].as_str() == Some("DetachA"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        let b = ents
            .iter()
            .find(|e| e["name"].as_str() == Some("DetachB"))
            .unwrap()["id"]
            .as_u64()
            .unwrap();
        (a, b)
    };
    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let m = mcp.0.lock().unwrap();
        m.execute("attach_mesh", json!({"entity_id": id_a, "mesh_id": 1}))
            .unwrap();
        m.execute("attach_mesh", json!({"entity_id": id_b, "mesh_id": 2}))
            .unwrap();
    }
    app.update();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("detach_all_meshes", json!({}))
            .unwrap();
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("query_entities", json!({"has_mesh": true}))
        .unwrap();
    let meshed_ids: Vec<u64> = out.content["entities"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["id"].as_u64().unwrap())
        .collect();
    assert!(
        !meshed_ids.contains(&id_a),
        "DetachA should have no mesh after detach_all"
    );
    assert!(
        !meshed_ids.contains(&id_b),
        "DetachB should have no mesh after detach_all"
    );
}

#[test]
fn mcp_spawn_camera_creates_camera_entity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "spawn_camera",
                json!({"fov_y_degrees": 75.0, "position": [0.0, 5.0, 10.0]}),
            )
            .expect("spawn_camera not found");
        assert!(result.is_ok());
    }
    app.update(); // process_editor_commands spawns Camera
    app.update(); // update_editor_snapshot captures it

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let list = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let entities = list.content["entities"].as_array().unwrap();
        let cam_entity = entities
            .iter()
            .find(|e| e["camera_fov"].is_number())
            .expect("no camera entity in snapshot");
        let fov = cam_entity["camera_fov"].as_f64().unwrap();
        assert!((fov - 75.0).abs() < 0.5, "expected 75 fov, got {fov}");
        let pos = cam_entity["position"].as_array().unwrap();
        assert!((pos[1].as_f64().unwrap() - 5.0).abs() < 1e-4);
    }
}

#[test]
fn mcp_update_camera_changes_fov() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("spawn_camera", json!({"fov_y_degrees": 60.0}))
            .unwrap();
    }
    app.update();
    app.update();

    let entity_id = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let list = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        list.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["camera_fov"].is_number())
            .expect("no camera")["id"]
            .as_u64()
            .unwrap()
    };

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "update_camera",
                json!({"entity_id": entity_id, "fov_y_degrees": 90.0}),
            )
            .unwrap();
        assert!(result.is_ok());
    }
    app.update();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let list = mcp
            .0
            .lock()
            .unwrap()
            .execute("list_entities", json!({}))
            .unwrap();
        let fov = list.content["entities"]
            .as_array()
            .unwrap()
            .iter()
            .find(|e| e["camera_fov"].is_number())
            .unwrap()["camera_fov"]
            .as_f64()
            .unwrap();
        assert!((fov - 90.0).abs() < 0.5, "expected 90 fov, got {fov}");
    }
}

#[test]
fn mcp_spawn_spot_light_creates_entity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "spawn_spot_light",
                json!({
                    "color": [1.0, 1.0, 1.0],
                    "intensity": 3.0,
                    "range": 15.0,
                    "inner_angle": 0.3,
                    "outer_angle": 0.6,
                    "position": [0.0, 5.0, 0.0]
                }),
            )
            .expect("spawn_spot_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let mut q = app.world_mut().query::<&bsengine_core::SpotLight>();
    let lights: Vec<_> = q.iter(app.world()).collect();
    assert_eq!(lights.len(), 1);
    assert!((lights[0].intensity - 3.0).abs() < 1e-4);
    assert!((lights[0].inner_angle_degrees.0 - 0.3_f32.to_degrees()).abs() < 1e-3);
}

#[test]
fn mcp_update_point_light_changes_intensity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn(bsengine_core::PointLight::default())
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "update_point_light",
                json!({"entity_id": eid.index() as u64, "intensity": 5.0, "range": 20.0}),
            )
            .expect("update_point_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let light = app
        .world_mut()
        .query::<&bsengine_core::PointLight>()
        .iter(app.world())
        .next()
        .unwrap();
    assert!(
        (light.intensity - 5.0).abs() < 1e-4,
        "intensity not updated"
    );
    assert!((light.range - 20.0).abs() < 1e-4, "range not updated");
}

#[test]
fn mcp_update_directional_light_changes_color() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn(bsengine_core::DirectionalLight::default())
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "update_directional_light",
                json!({"entity_id": eid.index() as u64, "color": [0.5, 0.5, 0.5]}),
            )
            .expect("update_directional_light not found");
        assert!(result.is_ok(), "{:?}", result.error);
    }
    app.update();

    let light = app
        .world_mut()
        .query::<&bsengine_core::DirectionalLight>()
        .iter(app.world())
        .next()
        .unwrap();
    assert!((light.color.x - 0.5).abs() < 1e-4, "color.r not updated");
}

#[test]
fn mcp_detach_mesh_removes_mesh_renderer() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((
            Name("Sphere".to_string()),
            Transform::from_position(Vec3::ZERO),
            bsengine_render::MeshRenderer { mesh_id: 7 },
            bsengine_core::GlobalTransform::default(),
        ))
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("detach_mesh", json!({"entity_id": eid.index() as u64}))
            .expect("detach_mesh not found");
    }
    app.update();

    let mut q = app.world_mut().query::<&bsengine_render::MeshRenderer>();
    assert!(
        q.iter(app.world()).next().is_none(),
        "MeshRenderer still present after detach"
    );
}

#[test]
fn mcp_detach_physics_body_removes_physics_body_desc() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((
            Name("Box".to_string()),
            Transform::from_position(Vec3::ZERO),
            bsengine_scene::PhysicsBodyDesc {
                rigidbody: bsengine_scene::RigidBodyDesc::Static,
                collider: bsengine_scene::ColliderDesc {
                    shape: bsengine_scene::ColliderShapeDesc::Box {
                        hx: 1.0,
                        hy: 1.0,
                        hz: 1.0,
                    },
                    restitution: 0.0,
                    friction: 0.5,
                    sensor: false,
                },
                linear_damping: None,
                angular_damping: None,
            },
        ))
        .id();
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "detach_physics_body",
                json!({"entity_id": eid.index() as u64}),
            )
            .expect("detach_physics_body not found");
    }
    app.update();

    assert!(app
        .world()
        .get::<bsengine_scene::PhysicsBodyDesc>(eid)
        .is_none());
}
