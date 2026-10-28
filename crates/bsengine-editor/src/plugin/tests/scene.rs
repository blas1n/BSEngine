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
fn mcp_clear_scene_with_a_hierarchy_removes_everything_once() {
    // ClearScene despawns every entity it sees, children included, while a
    // parent's despawn already takes its children: the second despawn of a
    // child is of an entity that is gone. That has to be harmless -- no
    // panic, nothing left -- whichever order the two arrive in.
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let root = app
        .world_mut()
        .spawn(bsengine_scene::Name("Root".to_string()))
        .id();
    let mid = app
        .world_mut()
        .spawn((
            bsengine_scene::Name("Mid".to_string()),
            bsengine_core::ChildOf(root),
        ))
        .id();
    app.world_mut().spawn((
        bsengine_scene::Name("Leaf".to_string()),
        bsengine_core::ChildOf(mid),
    ));
    app.update();

    {
        let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
        let stats = mcp
            .0
            .lock()
            .unwrap()
            .execute("get_scene_stats", json!({}))
            .unwrap();
        assert_eq!(
            stats.content["total_entities"], 3,
            "premise: the tree is there"
        );
        let result = mcp
            .0
            .lock()
            .unwrap()
            .execute("clear_scene", json!({}))
            .expect("clear_scene not found");
        assert!(result.is_ok());
    }
    app.update(); // process: all despawned
    app.update(); // snapshot

    let mcp = app.world().resource::<bsengine_mcp::McpRegistryResource>();
    let stats = mcp
        .0
        .lock()
        .unwrap()
        .execute("get_scene_stats", json!({}))
        .unwrap();
    assert_eq!(
        stats.content["total_entities"], 0,
        "nothing survives a clear"
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
                        "entity_id": eid.index_u32() as u64,
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
                        "entity_id": eid.index_u32() as u64,
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

    // Save: a camera, a primitive+scripted cube, a point light, a sun and a spot light.
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
                // Not the default, so the round trip has to carry it.
                bake_mode: bsengine_core::LightBakeMode::Realtime,
                color: Vec3::new(1.0, 0.5, 0.2).into(),
                intensity: 2.5,
                range: 15.0,
                cookie: "assets/textures/lantern.png".to_string(),
            },
            Transform::from_position(Vec3::new(0.0, 3.0, 0.0)),
            bsengine_core::GlobalTransform::default(),
        ));
        // A sun with a mode that is not the default: the directional light is
        // the one whose save path a mutation once dropped unobserved, as the
        // fixture had no directional light at all.
        app.world_mut().spawn((
            Name("Sun".to_string()),
            bsengine_core::DirectionalLight {
                bake_mode: bsengine_core::LightBakeMode::Baked,
                ..Default::default()
            },
            Transform::default(),
            bsengine_core::GlobalTransform::default(),
        ));
        app.world_mut().spawn((
            Name("Spot".to_string()),
            bsengine_core::SpotLight {
                bake_mode: bsengine_core::LightBakeMode::Baked,
                color: Vec3::new(0.2, 0.8, 1.0).into(),
                intensity: 3.0,
                range: 20.0,
                inner_angle_degrees: 15.0.into(),
                outer_angle_degrees: 25.0.into(),
                cookie: "assets/textures/window.png".to_string(),
                // Off, so the round trip carries the value that is not the default.
                shadows: false,
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
        assert_eq!(
            r.content["entity_count"], 5,
            "camera, crate, lamp, sun, spot"
        );
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
        // Through the descriptor's own field, for the reason the spot
        // light's cookie is: `PointLight` is not in the generic list either.
        assert_eq!(pl.cookie, "assets/textures/lantern.png");
        assert_eq!(pl.bake_mode, bsengine_core::LightBakeMode::Realtime);

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
        // Through the descriptor's own field: `SpotLight` is excluded from
        // the generic reflected list, so nothing else would carry it.
        assert_eq!(sl.cookie, "assets/textures/window.png");
        assert!(!sl.shadows, "the light that casts none still casts none");
        assert_eq!(sl.bake_mode, bsengine_core::LightBakeMode::Baked);
        let mut sun_q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::DirectionalLight)>();
        let (_, sun) = sun_q
            .iter(app.world())
            .find(|(n, _)| n.0 == "Sun")
            .expect("Sun not found after load");
        assert_eq!(sun.bake_mode, bsengine_core::LightBakeMode::Baked);
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

/// A scene loaded in the editor gets its baked lightmaps, as the runtime's
/// loads do: the entity the data names has its `Lightmap`, with the data's
/// texture and rectangle, once the spawned entities exist.
#[test]
fn load_scene_applies_the_scenes_baked_lightmaps() {
    let dir =
        std::env::temp_dir().join(format!("bsengine_editor_lightmaps_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let scene = dir.join("level.ron");
    let scene_path = scene.to_string_lossy().replace('\\', "/");
    let text = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Floor"), EntityDescriptor(name: "Wall")])"#;
    std::fs::write(&scene, text).unwrap();
    let data = bsengine_scene::lightmaps::LightmapData {
        scene_fingerprint: bsengine_scene::lightmaps::fingerprint(text),
        entries: vec![bsengine_scene::lightmaps::LightmapEntry {
            entity: "Floor".into(),
            texture: "assets/lightmaps/level/lightmap_0.png".into(),
            scale_offset: [0.5, 0.25, 0.125, 0.0],
            direction: String::new(),
        }],
        probes: None,
    };
    std::fs::write(
        bsengine_scene::lightmaps::data_path(&scene_path),
        ron::ser::to_string(&data).unwrap(),
    )
    .unwrap();

    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();
    let r = app
        .world()
        .resource::<bsengine_mcp::McpRegistryResource>()
        .0
        .lock()
        .unwrap()
        .execute("load_scene", json!({"path": scene_path}))
        .unwrap();
    assert!(r.is_ok(), "{:?}", r.error);
    app.update();
    app.update();

    let mut q = app
        .world_mut()
        .query::<(&Name, Option<&bsengine_core::Lightmap>)>();
    let found: Vec<(String, Option<[f32; 4]>, Option<String>)> = q
        .iter(app.world())
        .map(|(n, l)| {
            (
                n.0.clone(),
                l.map(|l| l.scale_offset),
                l.map(|l| l.texture_path.clone()),
            )
        })
        .collect();
    let floor = found
        .iter()
        .find(|f| f.0 == "Floor")
        .expect("premise: Floor loaded");
    assert_eq!(floor.1, Some([0.5, 0.25, 0.125, 0.0]), "{found:?}");
    assert_eq!(
        floor.2.as_deref(),
        Some("assets/lightmaps/level/lightmap_0.png")
    );
    let wall = found
        .iter()
        .find(|f| f.0 == "Wall")
        .expect("premise: Wall loaded");
    assert_eq!(wall.1, None, "the data does not name the wall");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The real `bsengine-runtime`, built once for the bake tests below -- the
/// button runs it as a child, so nothing less shows that it works.
fn runtime_bin() -> &'static std::path::PathBuf {
    static PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PATH.get_or_init(|| {
        let output = std::process::Command::new(env!("CARGO"))
            .args([
                "build",
                "-p",
                "bsengine-runtime",
                "--bin",
                "bsengine-runtime",
                "--message-format=json",
            ])
            .output()
            .expect("cargo build runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        for line in String::from_utf8_lossy(&output.stdout).lines() {
            let Ok(msg) = serde_json::from_str::<serde_json::Value>(line) else {
                continue;
            };
            if msg["reason"] == "compiler-artifact" && msg["target"]["name"] == "bsengine-runtime" {
                if let Some(exe) = msg["executable"].as_str() {
                    return std::path::PathBuf::from(exe);
                }
            }
        }
        panic!("no bsengine-runtime executable in cargo's output")
    })
}

/// A project with a static floor and crate, plus `extra` entities; returns
/// its directory and the scene's path.
fn bake_project(tag: &str, extra: &str) -> (std::path::PathBuf, String) {
    let dir =
        std::env::temp_dir().join(format!("bsengine_bake_button_{tag}_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("assets/scenes")).unwrap();
    std::fs::write(
        dir.join("project.toml"),
        "[project]\nname = \"Bake Button\"\nentry_scene = \"assets/scenes/main.ron\"\n\n[window]\nwidth = 64\nheight = 64\n",
    )
    .unwrap();
    let static_ = r#"components: [("bsengine_core::lightmap_static::LightmapStatic", "(resolution_scale: 1.0)")]"#;
    std::fs::write(
        dir.join("assets/scenes/main.ron"),
        format!(
            r#"SceneDescriptor(entities: [
    EntityDescriptor(name: "Floor", primitive: Some(Plane), transform: Some((position: (0.0, 0.0, 0.0), scale: (4.0, 1.0, 4.0))), {static_}),
    EntityDescriptor(name: "Crate", primitive: Some(Cube), transform: Some((position: (0.0, 0.5, 0.0))), {static_}),
    {extra}
])"#
        ),
    )
    .unwrap();
    let scene = dir
        .join("assets/scenes/main.ron")
        .to_string_lossy()
        .replace('\\', "/");
    (dir, scene)
}

/// Loads `scene` in an editor whose bake runs the real runtime, presses Bake,
/// and runs frames until the bake is over; returns the app.
fn press_bake(scene: &str) -> bevy_app::App {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.insert_resource(super::super::lightmap_bake::LightmapBakerBinary(
        runtime_bin().clone(),
    ));
    app.update();
    let r = app
        .world()
        .resource::<bsengine_mcp::McpRegistryResource>()
        .0
        .lock()
        .unwrap()
        .execute("load_scene", json!({"path": scene}))
        .unwrap();
    assert!(r.is_ok(), "{:?}", r.error);
    app.update();
    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(bsengine_core::InspectorCmd::BakeLightmaps);
    app.update();
    assert!(
        app.world()
            .resource::<InspectorState>()
            .lightmap_bake_running,
        "premise: pressing Bake started a bake"
    );
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300);
    while app
        .world()
        .resource::<InspectorState>()
        .lightmap_bake_running
    {
        assert!(
            std::time::Instant::now() < deadline,
            "the bake never finished"
        );
        std::thread::sleep(std::time::Duration::from_millis(100));
        app.update();
    }
    app
}

/// The button, end to end: it bakes the saved scene, says so, and the scene in
/// the editor is lit by the result without being reloaded.
#[test]
fn the_bake_button_bakes_the_scene_and_lights_it_in_the_editor() {
    let (dir, scene) = bake_project("ok", "");
    let mut app = press_bake(&scene);
    let status = app
        .world()
        .resource::<InspectorState>()
        .lightmap_bake_status
        .clone()
        .unwrap_or_default();
    assert!(status.starts_with("Baked 2 renderer(s)"), "{status}");
    // Which backend `auto` chose depends on the machine; that it is named
    // does not -- the report carries it.
    assert!(
        status.contains(" on the GPU;") || status.contains(" on the CPU;"),
        "the status names the backend that ran: {status}"
    );
    assert!(
        dir.join("assets/scenes/main.lightmaps").is_file(),
        "the data was written"
    );
    let mut q = app
        .world_mut()
        .query::<(&Name, Option<&bsengine_core::Lightmap>)>();
    let lit: Vec<(String, bool)> = q
        .iter(app.world())
        .map(|(n, l)| (n.0.clone(), l.is_some()))
        .collect();
    assert!(
        lit.contains(&("Floor".to_string(), true)) && lit.contains(&("Crate".to_string(), true)),
        "both static entities are lit in the editor: {lit:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// A bake that cannot go ahead says why, and leaves the button usable.
#[test]
fn the_bake_button_reports_why_a_bake_failed() {
    let static_ = r#"components: [("bsengine_core::lightmap_static::LightmapStatic", "(resolution_scale: 1.0)")]"#;
    let (dir, scene) = bake_project(
        "dup",
        // With a transform: an entity without one is drawn by nothing and
        // baked by nothing, so a first version of this fixture made no
        // duplicate at all and the bake succeeded.
        &format!(
            r#"EntityDescriptor(name: "Crate", primitive: Some(Cube), transform: Some((position: (2.0, 0.5, 0.0))), {static_}),"#
        ),
    );
    let app = press_bake(&scene);
    let insp = app.world().resource::<InspectorState>();
    let status = insp.lightmap_bake_status.clone().unwrap_or_default();
    assert!(
        status.starts_with("Bake failed") && status.contains("Crate"),
        "{status}"
    );
    assert!(!insp.lightmap_bake_running, "the button is usable again");
    let _ = std::fs::remove_dir_all(&dir);
}
