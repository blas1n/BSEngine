//! Scene and prefab file tools.

use super::*;

#[test]
fn mcp_terrain_write_spawns_a_terrain_entity_with_the_given_params() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "terrain_write",
                json!({
                    "heightmap_path": "assets/terrain/test_heightmap.png",
                    "chunk_count": [2, 2],
                    "chunk_size": 32.0,
                    "height_scale": 20.0,
                    "layer0_texture_path": "assets/terrain/grass.png",
                    "layer1_texture_path": "assets/terrain/rock.png",
                    "layer2_texture_path": "assets/terrain/dirt.png",
                    "layer3_texture_path": "assets/terrain/snow.png",
                }),
            )
            .expect("terrain_write not registered");
        assert!(out.is_ok(), "terrain_write failed: {:?}", out.error);
    }
    app.update();

    // terrain_write spawns a brand-new entity (unlike attach_physics_body,
    // which attaches to a caller-supplied id), and the command-queue
    // pattern every other MCP spawn tool uses (spawn_point_light,
    // attach_physics_body, ...) is fire-and-forget with no synchronous id
    // returned in the response -- so the spawned entity is found by
    // querying for its Terrain component, mirroring
    // `spawn_mesh_asset_command_spawns_entity_with_name_and_gltf_asset`.
    let mut query = app.world_mut().query::<&bsengine_app::terrain::Terrain>();
    let terrain = query
        .iter(app.world())
        .next()
        .expect("expected one entity with a Terrain component");
    assert_eq!(terrain.heightmap_path, "assets/terrain/test_heightmap.png");
    assert_eq!(terrain.chunk_count, (2, 2));
    assert!((terrain.chunk_size - 32.0).abs() < 1e-5);
    assert!((terrain.height_scale - 20.0).abs() < 1e-5);
    assert_eq!(terrain.layer0_texture_path, "assets/terrain/grass.png");
    assert_eq!(terrain.layer1_texture_path, "assets/terrain/rock.png");
    assert_eq!(terrain.layer2_texture_path, "assets/terrain/dirt.png");
    assert_eq!(terrain.layer3_texture_path, "assets/terrain/snow.png");
}

/// `splatmap_path` is the one optional `terrain_write` arg (the other 6
/// are `required`); the test above proves omitting it still works, and
/// this proves the field round-trips onto the spawned `Terrain` when the
/// caller does provide it.
#[test]
fn mcp_terrain_write_threads_an_optional_splatmap_path_through_when_given() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let out = mcp
            .0
            .lock()
            .unwrap()
            .execute(
                "terrain_write",
                json!({
                    "heightmap_path": "assets/terrain/test_heightmap.png",
                    "chunk_count": [2, 2],
                    "chunk_size": 32.0,
                    "height_scale": 20.0,
                    "layer0_texture_path": "assets/terrain/grass.png",
                    "layer1_texture_path": "assets/terrain/rock.png",
                    "layer2_texture_path": "assets/terrain/dirt.png",
                    "layer3_texture_path": "assets/terrain/snow.png",
                    "splatmap_path": "assets/terrain/splatmap.png",
                }),
            )
            .expect("terrain_write not registered");
        assert!(out.is_ok(), "terrain_write failed: {:?}", out.error);
    }
    app.update();

    let mut query = app.world_mut().query::<&bsengine_app::terrain::Terrain>();
    let terrain = query
        .iter(app.world())
        .next()
        .expect("expected one entity with a Terrain component");
    assert_eq!(
        terrain.splatmap_path.as_deref(),
        Some("assets/terrain/splatmap.png")
    );
}

#[test]
fn mcp_clear_scene_removes_all_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.world_mut().spawn(bsengine_scene::Name("A".to_string()));
    app.world_mut().spawn(bsengine_scene::Name("B".to_string()));
    app.world_mut().spawn(bsengine_core::PointLight::default());
    app.update(); // snapshot: 3 entities

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("clear_scene", json!({}))
            .expect("clear_scene not found");
        assert!(result.is_ok());
    }
    app.update(); // process: all despawned
    app.update(); // snapshot: empty

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let stats = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_scene_stats", json!({}))
        .unwrap();
    assert_eq!(stats.content["total_entities"], 0);
}

#[test]
fn mcp_save_scene_writes_ron_file() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.world_mut().spawn((
        Name("Castle".to_string()),
        Transform::from_position(Vec3::new(5.0, 0.0, 0.0)),
    ));
    app.update();

    let path = std::env::temp_dir()
        .join("bsengine_test_save.ron")
        .to_string_lossy()
        .to_string();
    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let result = mcp
        .0
        .lock()
        .unwrap()
        .execute("save_scene", json!({"path": path}))
        .expect("save_scene not found");
    assert!(result.is_ok(), "save error: {:?}", result.error);
    assert_eq!(result.content["status"], "saved");
    assert_eq!(result.content["entity_count"], 1);
    assert!(std::path::Path::new(&path).exists());
}

#[test]
fn mcp_save_load_scene_round_trip() {
    let path = std::env::temp_dir()
        .join("bsengine_test_roundtrip.ron")
        .to_string_lossy()
        .to_string();

    // Save
    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn((
            Name("Tower".to_string()),
            Transform::from_position(Vec3::new(3.0, 1.0, 0.0)),
        ));
        app.update();
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let r = mcp
            .0
            .lock()
            .unwrap()
            .execute("save_scene", json!({"path": path}))
            .unwrap();
        assert!(r.is_ok(), "{:?}", r.error);
    }

    // Load in new app
    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("load_scene", json!({"path": path}))
                .expect("load_scene not found");
        }
        app.update();

        let mut q = app.world_mut().query::<(&Name, &Transform)>();
        let results: Vec<_> = q
            .iter(app.world())
            .map(|(n, t)| (n.0.as_str(), t.position))
            .collect();
        let found = results
            .iter()
            .find(|(name, _)| *name == "Tower")
            .expect("Tower not found after load");
        assert!((found.1.x - 3.0).abs() < 1e-4, "wrong x: {}", found.1.x);
    }
}

#[test]
fn mcp_save_load_scene_round_trip_preserves_physics_body_attached_via_mcp() {
    let path = std::env::temp_dir()
        .join("bsengine_test_roundtrip_physics_body.ron")
        .to_string_lossy()
        .to_string();

    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app
            .world_mut()
            .spawn((
                Name("Crate".to_string()),
                Transform::from_position(Vec3::new(2.0, 0.0, 0.0)),
            ))
            .id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let out = mcp
                .0
                .lock()
                .unwrap()
                .execute(
                    "attach_physics_body",
                    json!({
                        "entity_id": eid.index() as u64,
                        "rigidbody": "Static",
                        "collider_shape": "Box",
                        "hx": 1.0, "hy": 1.0, "hz": 1.0,
                    }),
                )
                .expect("attach_physics_body not found");
            assert!(out.is_ok(), "{:?}", out.error);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let r = mcp
            .0
            .lock()
            .unwrap()
            .execute("save_scene", json!({"path": path}))
            .unwrap();
        assert!(r.is_ok(), "{:?}", r.error);
    }

    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("load_scene", json!({"path": path}))
                .expect("load_scene not found");
        }
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_scene::PhysicsBodyDesc)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1, "PhysicsBodyDesc missing after load");
        assert_eq!(results[0].0 .0, "Crate");
        assert_eq!(
            results[0].1.rigidbody,
            bsengine_scene::RigidBodyDesc::Static
        );
    }
}

#[test]
fn mcp_save_load_scene_round_trip_preserves_animation_state_machine() {
    // AnimationStateMachine::triggers is a HashSet<String>; the generic
    // save-side serializer (TypedReflectSerializer) needs ReflectSerialize
    // registered for HashSet<String> or it fails ("did not register
    // ReflectSerialize") and silently drops the whole component from the
    // saved scene. Regression test for that specific failure mode.
    let path = std::env::temp_dir()
        .join("bsengine_test_roundtrip_animation_state_machine.ron")
        .to_string_lossy()
        .to_string();

    let value_json = r#"{
        "states": {
            "idle": {"clip": "idle_clip", "blend": null, "looping": true, "speed": 1.0, "duration": 1.0},
            "walk": {"clip": "walk_clip", "blend": null, "looping": true, "speed": 1.0, "duration": 1.0}
        },
        "transitions": [
            {"from": "idle", "to": "walk", "condition": {"FloatGreater": {"param": "speed", "threshold": 0.1}}, "blend_duration": 0.2}
        ],
        "current_state": "idle",
        "params_float": {"speed": 0.0},
        "params_bool": {},
        "triggers": [],
        "blend_from": null,
        "blend_weight": 1.0,
        "blend_duration": 0.0,
        "blend_elapsed": 0.0
    }"#;

    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        let eid = app.world_mut().spawn(Name("Player".to_string())).id();
        app.update();

        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            let registry = mcp.0.lock().unwrap();
            let out = registry
                .execute(
                    "set_reflected_component",
                    json!({
                        "entity_id": eid.index() as u64,
                        "type_path": "bsengine_core::animation_state_machine::AnimationStateMachine",
                        "value_json": value_json,
                    }),
                )
                .expect("tool should be registered");
            assert!(out.is_ok(), "{:?}", out.error);
        }
        app.update();
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let r = mcp
            .0
            .lock()
            .unwrap()
            .execute("save_scene", json!({"path": path}))
            .unwrap();
        assert!(r.is_ok(), "{:?}", r.error);
    }

    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("load_scene", json!({"path": path}))
                .expect("load_scene not found");
        }
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::AnimationStateMachine)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1, "AnimationStateMachine missing after load");
        assert_eq!(results[0].0 .0, "Player");
        assert_eq!(results[0].1.current_state, "idle");
        assert!(results[0].1.states.contains_key("walk"));
        assert_eq!(results[0].1.transitions.len(), 1);
    }
}

#[test]
fn mcp_save_load_scene_round_trip_preserves_camera_primitive_script_and_lights() {
    let path = std::env::temp_dir()
        .join("bsengine_test_roundtrip_full.ron")
        .to_string_lossy()
        .to_string();

    // Save: a camera, a primitive+scripted cube, a point light, and a spot light.
    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        app.world_mut().spawn((
            Name("MainCam".to_string()),
            bsengine_core::Camera::perspective(75.0, 16.0 / 9.0),
            Transform::from_position(Vec3::new(0.0, 2.0, 5.0)),
            bsengine_core::GlobalTransform::default(),
        ));
        app.world_mut().spawn((
            Name("Crate".to_string()),
            bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Cube),
            bsengine_scene::ScriptPath("assets/scripts/crate.js".to_string()),
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
            bsengine_core::GlobalTransform::default(),
        ));
        app.world_mut().spawn((
            Name("Lamp".to_string()),
            bsengine_core::PointLight {
                color: Vec3::new(1.0, 0.5, 0.2).into(),
                intensity: 2.5,
                range: 15.0,
            },
            Transform::from_position(Vec3::new(0.0, 3.0, 0.0)),
            bsengine_core::GlobalTransform::default(),
        ));
        app.world_mut().spawn((
            Name("Spot".to_string()),
            bsengine_core::SpotLight {
                color: Vec3::new(0.2, 0.8, 1.0).into(),
                intensity: 3.0,
                range: 20.0,
                inner_angle_degrees: 15.0.into(),
                outer_angle_degrees: 25.0.into(),
            },
            Transform::from_position(Vec3::new(2.0, 4.0, 0.0)),
            bsengine_core::GlobalTransform::default(),
        ));
        app.update();

        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let r = mcp
            .0
            .lock()
            .unwrap()
            .execute("save_scene", json!({"path": path}))
            .unwrap();
        assert!(r.is_ok(), "{:?}", r.error);
        assert_eq!(r.content["entity_count"], 4);
    }

    // Load in a fresh app and verify every component round-tripped.
    {
        let mut app = new_app();
        app.add_plugins(McpPlugin);
        app.add_plugins(EditorPlugin);
        {
            let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
            mcp.0
                .lock()
                .unwrap()
                .execute("load_scene", json!({"path": path}))
                .expect("load_scene not found");
        }
        app.update();

        let mut cam_q = app.world_mut().query::<(&Name, &bsengine_core::Camera)>();
        let (_, cam) = cam_q
            .iter(app.world())
            .find(|(n, _)| n.0 == "MainCam")
            .expect("MainCam not found after load");
        assert!(
            (cam.fov_y_degrees.0 - 75.0).abs() < 1e-3,
            "wrong fov: {}",
            cam.fov_y_degrees.0
        );

        let mut prim_q = app.world_mut().query::<(
            &Name,
            &bsengine_scene::PrimitiveMesh,
            &bsengine_scene::ScriptPath,
        )>();
        let (_, prim, script) = prim_q
            .iter(app.world())
            .find(|(n, _, _)| n.0 == "Crate")
            .expect("Crate not found after load");
        assert_eq!(prim.0, bsengine_scene::Primitive::Cube);
        assert_eq!(script.0, "assets/scripts/crate.js");

        let mut pl_q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::PointLight)>();
        let (_, pl) = pl_q
            .iter(app.world())
            .find(|(n, _)| n.0 == "Lamp")
            .expect("Lamp not found after load");
        assert!((pl.intensity - 2.5).abs() < 1e-4);
        assert!((pl.range - 15.0).abs() < 1e-4);
        assert!((pl.color.x - 1.0).abs() < 1e-4);
        assert!((pl.color.y - 0.5).abs() < 1e-4);

        let mut sl_q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::SpotLight)>();
        let (_, sl) = sl_q
            .iter(app.world())
            .find(|(n, _)| n.0 == "Spot")
            .expect("Spot not found after load");
        assert!((sl.intensity - 3.0).abs() < 1e-4);
        assert!((sl.range - 20.0).abs() < 1e-4);
        assert!((sl.inner_angle_degrees.0 - 15.0).abs() < 1e-2);
        assert!((sl.outer_angle_degrees.0 - 25.0).abs() < 1e-2);
    }
}

// Regression test for the Unity/Unreal-style "pressing Play resets the
// scene" behavior: the toolbar's Play button (bsengine-rhi-wgpu) pushes
// InspectorCmd::ReloadScene when transitioning Stopped -> Playing, and
// apply_inspector_cmds must turn that into a PendingSceneLoad so the
// existing handle_scene_load system (which properly despawns and
// respawns from the RON file) can pick it up. This can't exercise the
// full despawn/respawn round trip headlessly here — that system lives
// in bsengine-runtime, and bsengine-runtime can't combine EditorPlugin
// with ScriptingPlugin in one process (known V8 handle-scope conflict)
// — but it does prove the wiring this crate owns: ReloadScene ->
// PendingSceneLoad with the right path, catching a regression like the
// match arm being dropped or the path being wrong.
#[test]
fn inspector_cmd_reload_scene_inserts_pending_scene_load() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let mut inspector = app.world_mut().resource_mut::<InspectorState>();
        inspector.current_scene_path = Some("assets/scenes/level1.ron".to_string());
        inspector.cmd_queue.push(InspectorCmd::ReloadScene);
    }
    app.update();

    let pending = app
        .world()
        .get_resource::<bsengine_scene::PendingSceneLoad>();
    assert_eq!(
        pending.map(|p| p.path.clone()),
        Some("assets/scenes/level1.ron".to_string()),
        "ReloadScene should insert a PendingSceneLoad for the current scene path"
    );
}

#[test]
fn inspector_cmd_reload_scene_without_current_path_does_not_insert_pending_load() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let mut inspector = app.world_mut().resource_mut::<InspectorState>();
        assert_eq!(inspector.current_scene_path, None);
        inspector.cmd_queue.push(InspectorCmd::ReloadScene);
    }
    app.update();

    assert!(
        app.world()
            .get_resource::<bsengine_scene::PendingSceneLoad>()
            .is_none(),
        "ReloadScene with no current scene path should not insert PendingSceneLoad"
    );
}

#[test]
fn load_scene_spawns_gltf_asset_for_entity_with_gltf_field() {
    let path = std::env::temp_dir()
        .join("bsengine_test_load_scene_gltf.ron")
        .to_string_lossy()
        .to_string();
    std::fs::write(
        &path,
        r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Player", gltf: Some("models/hero.glb")),
        ])"#,
    )
    .unwrap();

    let mut app = new_app();
    app.insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute("load_scene", json!({"path": path}))
            .expect("load_scene not found");
    }
    app.update();

    let mut query = app
        .world_mut()
        .query::<(&Name, &bsengine_gltf::GltfAsset)>();
    let (name, gltf_asset) = query
        .iter(app.world())
        .find(|(n, _)| n.0 == "Player")
        .expect("expected Player entity with GltfAsset after load_scene");
    assert_eq!(name.0, "Player");
    assert_eq!(gltf_asset.path, "games/demo/models/hero.glb");
}

#[test]
fn mcp_prefab_write_saves_a_subtree_to_a_ron_file() {
    let dir = tempfile::tempdir().unwrap();
    let mut app = new_app();
    app.insert_resource(bsengine_core::ProjectDir(
        dir.path().to_string_lossy().to_string(),
    ));
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
                    {"name": "Turret"},
                    {"name": "Barrel"},
                ]}),
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
    let id_of = |name: &str| {
        all.iter()
            .find(|e| e["name"].as_str() == Some(name))
            .unwrap()["id"]
            .as_u64()
            .unwrap()
    };
    let (turret_id, barrel_id) = (id_of("Turret"), id_of("Barrel"));

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        mcp.0
            .lock()
            .unwrap()
            .execute(
                "set_parent",
                json!({"entity_id": barrel_id, "parent_id": turret_id}),
            )
            .unwrap();
    }
    app.update();
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute(
            "prefab_write",
            json!({"entity_id": turret_id, "name": "turret"}),
        )
        .expect("prefab_write not registered");
    assert!(out.is_ok(), "prefab_write failed: {:?}", out.error);
    let path = out.content["path"].as_str().unwrap().to_string();
    assert!(path.ends_with("assets/prefabs/turret.ron"), "{path}");

    let parsed: bsengine_scene::types::PrefabDescriptor =
        ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let names: Vec<&str> = parsed.entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(names.len(), 2, "{names:?}");
    assert!(names.contains(&"Turret"));
    assert!(names.contains(&"Barrel"));
}

#[test]
fn mcp_prefab_write_reports_an_error_for_an_unknown_entity_id() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let out = mcp
        .0
        .lock()
        .unwrap()
        .execute("prefab_write", json!({"entity_id": 999999, "name": "x"}))
        .expect("prefab_write not registered");
    assert!(!out.is_ok());
}

#[test]
fn apply_to_prefab_tool_queues_a_command() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    let out = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();
        mcp.execute("apply_to_prefab", json!({"entity_id": 42}))
    }
    .expect("apply_to_prefab not registered");
    assert!(out.is_ok(), "apply_to_prefab failed: {:?}", out.error);
    assert_eq!(out.content["status"], "queued");

    let queue = app
        .world()
        .resource::<crate::snapshot::PrefabApplyCommandQueueResource>();
    let queued = queue.0.lock().unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].entity_id, 42);
}

#[test]
fn apply_to_prefab_tool_errors_without_entity_id() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    let out = {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let mcp = mcp.0.lock().unwrap();
        mcp.execute("apply_to_prefab", json!({}))
    }
    .expect("apply_to_prefab not registered");
    assert!(!out.is_ok());
}

#[test]
fn apply_inspector_cmds_bridges_apply_to_prefab_into_the_prefab_apply_queue() {
    let mut app = new_app();
    app.add_plugins(EditorPlugin);

    {
        let mut inspector = app.world_mut().resource_mut::<InspectorState>();
        inspector
            .cmd_queue
            .push(InspectorCmd::ApplyToPrefab { entity_id: 42 });
    }

    app.update();

    let queue = app
        .world()
        .resource::<crate::snapshot::PrefabApplyCommandQueueResource>();
    let queued = queue.0.lock().unwrap();
    assert_eq!(queued.len(), 1);
    assert_eq!(queued[0].entity_id, 42);
}
