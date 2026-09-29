//! The editor systems driven directly, without MCP: snapshot, commands, history, prefab, inspector, camera.

use super::*;

#[test]
fn editor_plugin_builds_without_panic() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();
}

/// A preview request must take over the editor's view-projection, and the
/// orbit state must win when there is none.
///
/// Both halves are needed: "the view-projection changed" on its own is
/// satisfied by an override that is always on, which would make the
/// editor camera unusable and still pass.
#[test]
fn a_preview_request_overrides_the_orbit_camera() {
    use bsengine_core::{PreviewCamera, TimelinePreview};

    let mut app = new_app();
    app.insert_resource({
        // Field-by-field rather than functional-update syntax:
        // `InspectorState` has a private field, so `..Default::default()`
        // is barred outside `bsengine-core`.
        let mut insp = InspectorState::default();
        insp.editor_mode = true;
        insp.viewport_size = [800.0, 600.0];
        insp
    });
    app.add_systems(bevy_app::Update, super::super::update_editor_camera);

    app.update();
    let orbit = app
        .world()
        .resource::<InspectorState>()
        .editor_view_proj
        .expect("the orbit camera always publishes one");

    app.world_mut()
        .resource_mut::<InspectorState>()
        .timeline_preview = Some(TimelinePreview {
        camera: Some(PreviewCamera {
            position: [100.0, 50.0, 25.0],
            rotation: [0.0, 0.0, 0.0, 1.0],
            fov_y_degrees: None,
        }),
        clips: Vec::new(),
    });
    app.update();
    let previewing = app
        .world()
        .resource::<InspectorState>()
        .editor_view_proj
        .expect("still published while previewing");
    assert_ne!(
        orbit, previewing,
        "a preview request must take over the editor's view-projection"
    );

    app.world_mut()
        .resource_mut::<InspectorState>()
        .timeline_preview = None;
    app.update();
    let restored = app
        .world()
        .resource::<InspectorState>()
        .editor_view_proj
        .expect("published again once preview ends");
    assert_eq!(
        orbit, restored,
        "clearing the preview must return the user to the orbit viewpoint \
         they had, not leave the camera stranded at the cutscene"
    );
}

#[test]
fn editor_plugin_inserts_panel_registry() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    let registry = app.world().resource::<bsengine_core::EditorPanelRegistry>();
    assert!(registry.0.lock().unwrap().is_empty());
}

#[test]
fn build_entity_descriptors_resolves_parent_id_to_the_parents_name() {
    let entities = vec![
        EntityInfo {
            id: 1,
            name: Some("Body".to_string()),
            parent_id: None,
            ..Default::default()
        },
        EntityInfo {
            id: 2,
            name: Some("Wheel".to_string()),
            parent_id: Some(1),
            ..Default::default()
        },
    ];

    let descriptors = build_entity_descriptors(&entities);
    let wheel = descriptors
        .iter()
        .find(|d| d.name == "Wheel")
        .expect("Wheel should be in the output");
    assert_eq!(wheel.parent.as_deref(), Some("Body"));

    let body = descriptors
        .iter()
        .find(|d| d.name == "Body")
        .expect("Body should be in the output");
    assert_eq!(
        body.parent, None,
        "Body has no parent_id, so no parent name"
    );
}

#[test]
fn build_entity_descriptors_warns_and_drops_parent_link_when_parent_has_no_name() {
    // Ensures the tracing subscriber exists even when this test runs in
    // isolation (e.g. via a `cargo test <name>` filter), so the warn!
    // below is actually visible under `--nocapture`.
    bsengine_core::init_logging();

    let entities = vec![
        // e.g. a spawned point light: has an id but no Name component.
        EntityInfo {
            id: 1,
            name: None,
            parent_id: None,
            ..Default::default()
        },
        EntityInfo {
            id: 2,
            name: Some("Wheel".to_string()),
            parent_id: Some(1),
            ..Default::default()
        },
    ];

    let descriptors = build_entity_descriptors(&entities);
    let wheel = descriptors
        .iter()
        .find(|d| d.name == "Wheel")
        .expect("Wheel should be in the output");
    assert_eq!(
        wheel.parent, None,
        "parent entity has no Name, so the link can't be preserved in the scene file"
    );
}

#[test]
fn populate_inspector_copies_parent_tags_script_primitive() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let parent = app.world_mut().spawn(Name("Parent".to_string())).id();
    let child = app
        .world_mut()
        .spawn((
            Name("Child".to_string()),
            bsengine_core::Parent(parent),
            crate::snapshot::Tags(vec!["enemy".to_string(), "boss".to_string()]),
            bsengine_scene::ScriptPath("assets/scripts/child.js".to_string()),
            bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Sphere),
        ))
        .id();
    app.update();

    let insp = app.world().resource::<InspectorState>();
    let child_info = insp
        .entities
        .iter()
        .find(|e| e.id == child.index() as u64)
        .expect("child entity should be in inspector snapshot");
    assert_eq!(child_info.parent_id, Some(parent.index() as u64));
    assert_eq!(
        child_info.tags,
        vec!["enemy".to_string(), "boss".to_string()]
    );
    assert_eq!(
        child_info.script_path,
        Some("assets/scripts/child.js".to_string())
    );
    assert_eq!(child_info.primitive, Some("sphere".to_string()));
}

#[test]
fn primitive_kinds_const_round_trips_through_str_conversions() {
    // Every string in the canonical `PRIMITIVE_KINDS` list (the
    // lowercase string form of `bsengine_scene::Primitive` used at the
    // `InspectorEntityInfo`/`InspectorCmd::AttachPrimitiveMesh`
    // DTO boundary, since the Inspector no longer has a dedicated Mesh
    // dropdown -- `PrimitiveMesh` attaches through the generic Add
    // Component menu like any other component) must parse via
    // `str_to_primitive` and the parsed value must map back to the same
    // string via `primitive_to_str`.
    //
    // This used to concede that it could not catch a *new* `Primitive`
    // variant going unlisted. It can now: `every_variant` below holds an
    // exhaustive match, so adding a variant to `Primitive` stops this
    // crate compiling until the variant is also added to the list, and the
    // loop then proves it round-trips. That gap was not hypothetical --
    // adding `Cylinder` left `PRIMITIVE_KINDS` at 4 entries, which the
    // editor dropdown silently omits, and only the hardcoded count caught
    // it.
    fn every_variant() -> Vec<bsengine_scene::Primitive> {
        use bsengine_scene::Primitive as P;
        // Exhaustive on purpose. Do not add a `_` arm: this match failing
        // to compile IS the guard.
        match P::Cube {
            P::Cube | P::Sphere | P::Plane | P::Capsule | P::Cylinder => {}
        }
        vec![P::Cube, P::Sphere, P::Plane, P::Capsule, P::Cylinder]
    }

    let variants = every_variant();
    assert_eq!(
        bsengine_core::PRIMITIVE_KINDS.len(),
        variants.len(),
        "every `Primitive` variant needs an entry in `PRIMITIVE_KINDS`, or              the editor's primitive dropdown silently omits it"
    );
    for p in &variants {
        let name = super::super::primitive_to_str(p);
        assert!(
            bsengine_core::PRIMITIVE_KINDS.contains(&name.as_str()),
            "`Primitive::{p:?}` maps to {name:?}, which is not in                  PRIMITIVE_KINDS"
        );
    }
    for &kind in &bsengine_core::PRIMITIVE_KINDS {
        let parsed = super::super::str_to_primitive(kind)
            .unwrap_or_else(|| panic!("PRIMITIVE_KINDS entry {kind:?} did not parse"));
        assert_eq!(
            super::super::primitive_to_str(&parsed),
            kind,
            "str_to_primitive/primitive_to_str do not round-trip for {kind:?}"
        );
    }
}

#[test]
fn reflect_command_attaches_and_removes_component_by_type_path() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::AttachComponentByType {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
    }
    app.update();

    assert!(
        app.world().get::<bsengine_core::Camera>(eid).is_some(),
        "Camera was not attached via ReflectCommand"
    );

    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::RemoveComponentByType {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
    }
    app.update();

    assert!(
        app.world().get::<bsengine_core::Camera>(eid).is_none(),
        "Camera was not removed via ReflectCommand"
    );
}

#[test]
fn reflect_command_apply_component_value_mutates_attached_component() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
        .id();
    app.update();

    let edited = bsengine_core::Camera {
        fov_y_degrees: 1.2345.into(),
        ..Default::default()
    };
    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
                value: Box::new(edited),
            });
    }
    app.update();

    let cam = app
        .world()
        .get::<bsengine_core::Camera>(eid)
        .expect("Camera should still be attached");
    assert!(
        (cam.fov_y_degrees.0 - 1.2345).abs() < f32::EPSILON,
        "Camera.fov_y_degrees should have been updated to the applied value"
    );
}

#[test]
fn reflect_command_apply_component_value_mutates_a_list_shaped_component() {
    // `Tags` (a `Vec<String>`-wrapping tuple struct) is the best
    // available `List`-shaped reflected component. `reflect_ui.rs`'s
    // List tests exercise the UI-only rendering in isolation (no ECS),
    // and the other `ApplyComponentValue` tests in this module only
    // cover `Struct`-shaped components (`Camera`, `Follow`) -- nothing
    // proves a `List`-shaped component round-trips through the full
    // command pipeline. This does: InspectorCmd::ApplyReflectedComponent
    // -> ReflectCommand::ApplyComponentValue ->
    // ReflectComponent::apply_or_insert.
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((Name("Target".to_string()), Tags(vec!["a".to_string()])))
        .id();
    app.update();

    // Simulates what the List-editing UI's "+" button would produce:
    // the existing value plus one appended element.
    let edited = Tags(vec!["a".to_string(), "b".to_string()]);
    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                entity_id: eid.index() as u64,
                type_path: "bsengine_editor::snapshot::Tags".to_string(),
                value: Box::new(edited),
            });
    }
    app.update();

    let tags = app
        .world()
        .get::<Tags>(eid)
        .expect("Tags should still be attached");
    assert_eq!(
        tags.0,
        vec!["a".to_string(), "b".to_string()],
        "Tags(Vec<String>) should round-trip through the full reflect command \
         pipeline end-to-end, not just the UI-only or snapshot-only layers"
    );
}

#[test]
fn apply_component_value_fixes_up_a_stale_entity_field_generation() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);

    // Spawn, despawn, and respawn at the same index so the live entity's
    // generation is now > 0 -- Entity::from_raw(index) (generation 0)
    // would NOT equal this live entity.
    let throwaway = app.world_mut().spawn(Name("Throwaway".to_string())).id();
    app.world_mut().despawn(throwaway);
    let target = app.world_mut().spawn(Name("Target".to_string())).id();
    assert_eq!(
        target.index(),
        throwaway.index(),
        "test setup requires the respawn to reuse the same index"
    );
    assert_ne!(
        target,
        bevy_ecs::prelude::Entity::from_raw(target.index()),
        "test setup requires a nonzero generation to actually exercise the fixup"
    );

    let follower = app.world_mut().spawn(Name("Follower".to_string())).id();
    app.update();

    let stale_follow =
        bsengine_core::Follow::new(bevy_ecs::prelude::Entity::from_raw(target.index()));
    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                entity_id: follower.index() as u64,
                type_path: "bsengine_core::follow::Follow".to_string(),
                value: Box::new(stale_follow),
            });
    }
    app.update();

    let applied = app
        .world()
        .get::<bsengine_core::Follow>(follower)
        .expect("Follow should have been applied");
    assert_eq!(
        applied.target, target,
        "the applied Follow.target must be fixed up to the live entity's real generation, \
         not the stale generation-0 placeholder the UI layer constructed"
    );
}

#[test]
fn apply_component_value_fixes_up_stale_entity_generations_inside_array_and_map_fields() {
    use bevy_ecs::prelude::{Component, ReflectComponent};
    use bevy_reflect::Reflect;

    // Test-only component whose reflect tree puts an `Entity` inside
    // both an `Array` (`[Entity; 2]`) and a `Map`
    // (`HashMap<String, Entity>`) field -- proving `fixup_entity_fields`'s
    // `Array`/`Map` arms actually recurse into their elements, rather
    // than silently falling through `_ => {}` like before this fix.
    #[derive(Component, Clone, Debug, Reflect)]
    #[reflect(Component)]
    struct EntityCollections {
        array: [bevy_ecs::prelude::Entity; 2],
        map: std::collections::HashMap<String, bevy_ecs::prelude::Entity>,
    }

    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.register_type::<EntityCollections>();

    // Spawn, despawn, and respawn at the same index so the live entity's
    // generation is now > 0 -- Entity::from_raw(index) (generation 0)
    // would NOT equal this live entity (same technique as
    // `apply_component_value_fixes_up_a_stale_entity_field_generation`).
    let throwaway = app.world_mut().spawn(Name("Throwaway".to_string())).id();
    app.world_mut().despawn(throwaway);
    let target = app.world_mut().spawn(Name("Target".to_string())).id();
    assert_eq!(
        target.index(),
        throwaway.index(),
        "test setup requires the respawn to reuse the same index"
    );
    assert_ne!(
        target,
        bevy_ecs::prelude::Entity::from_raw(target.index()),
        "test setup requires a nonzero generation to actually exercise the fixup"
    );

    let holder = app.world_mut().spawn(Name("Holder".to_string())).id();
    app.update();

    let stale = bevy_ecs::prelude::Entity::from_raw(target.index());
    let mut map = std::collections::HashMap::new();
    map.insert("target".to_string(), stale);
    let value = EntityCollections {
        array: [stale, stale],
        map,
    };

    let type_path = <EntityCollections as bevy_reflect::TypePath>::type_path().to_string();
    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::ApplyComponentValue {
                entity_id: holder.index() as u64,
                type_path,
                value: Box::new(value),
            });
    }
    app.update();

    let applied = app
        .world()
        .get::<EntityCollections>(holder)
        .expect("EntityCollections should have been applied");
    assert_eq!(
        applied.array[0], target,
        "Array element containing a stale Entity must be fixed up to the live generation"
    );
    assert_eq!(
        applied.array[1], target,
        "Array element containing a stale Entity must be fixed up to the live generation"
    );
    assert_eq!(
        applied.map.get("target"),
        Some(&target),
        "Map value containing a stale Entity must be fixed up to the live generation"
    );
}

#[test]
fn inspector_cmd_apply_reflected_component_reaches_reflect_queue() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
        .id();
    app.update();

    let edited = bsengine_core::Camera {
        fov_y_degrees: 0.5.into(),
        ..Default::default()
    };
    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::ApplyReflectedComponent {
            id: eid.index() as u64,
            type_path: "bsengine_core::camera::Camera".to_string(),
            value: Box::new(edited),
        });
    }
    app.update();

    let cam = app
        .world()
        .get::<bsengine_core::Camera>(eid)
        .expect("Camera should exist");
    assert!(
        (cam.fov_y_degrees.0 - 0.5).abs() < f32::EPSILON,
        "Camera should have been updated end-to-end via InspectorCmd"
    );
}

#[test]
fn reflect_command_queue_pushes_undo_checkpoint() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    let history_len_before = app
        .world()
        .resource::<crate::snapshot::EditorHistoryResource>()
        .0
        .lock()
        .unwrap()
        .undo_stack
        .len();

    {
        let queue = app
            .world()
            .resource::<crate::snapshot::ReflectCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::ReflectCommand::AttachComponentByType {
                entity_id: eid.index() as u64,
                type_path: "bsengine_core::camera::Camera".to_string(),
            });
    }
    app.update();

    let history_len_after = app
        .world()
        .resource::<crate::snapshot::EditorHistoryResource>()
        .0
        .lock()
        .unwrap()
        .undo_stack
        .len();
    assert_eq!(
        history_len_after,
        history_len_before + 1,
        "ReflectCommand processing should push an undo checkpoint, same as EditorCommand does"
    );
}

#[test]
fn populate_reflected_component_snapshot_clones_attached_camera() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
        .id();
    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.selected_id = Some(eid.index() as u64);
    }
    app.update();

    let insp = app.world().resource::<InspectorState>();
    let camera_entry = insp
        .reflected_components
        .iter()
        .find(|(type_path, _)| type_path == "bsengine_core::camera::Camera")
        .expect("Camera should be in reflected_components after selecting the entity");
    let cloned_camera = camera_entry
        .1
        .as_ref()
        .downcast_ref::<bsengine_core::Camera>()
        .expect("cloned value should downcast back to Camera");
    assert!(
        (cloned_camera.fov_y_degrees.0 - 60.0).abs() < f32::EPSILON,
        "cloned Camera's fov_y_degrees should be 60.0 (degrees) — proves the field is wired \
         through the real reflection pipeline with the correct unit, not just internally \
         self-consistent"
    );
}

#[test]
fn populate_reflected_component_snapshot_includes_attached_tags() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((Name("Target".to_string()), Tags(vec!["enemy".to_string()])))
        .id();
    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.selected_id = Some(eid.index() as u64);
    }
    app.update();

    let insp = app.world().resource::<InspectorState>();
    let found = insp
        .reflected_components
        .iter()
        .find(|(type_path, _)| type_path == "bsengine_editor::snapshot::Tags");
    assert!(
        found.is_some(),
        "Tags must appear in the Reflected Fields list once selected, now that it's a Reflect component"
    );
}

#[test]
fn populate_reflected_component_snapshot_clears_when_nothing_selected() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((Name("Target".to_string()), bsengine_core::Camera::default()))
        .id();
    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.selected_id = Some(eid.index() as u64);
    }
    app.update();
    assert!(!app
        .world()
        .resource::<InspectorState>()
        .reflected_components
        .is_empty());

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.selected_id = None;
    }
    app.update();
    assert!(
        app.world()
            .resource::<InspectorState>()
            .reflected_components
            .is_empty(),
        "reflected_components should clear once nothing is selected"
    );
}

#[test]
fn populate_reflected_component_snapshot_includes_attached_primitive_mesh_and_script_path() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app
        .world_mut()
        .spawn((
            Name("Target".to_string()),
            bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Capsule),
            bsengine_scene::ScriptPath("assets/scripts/foo.js".to_string()),
        ))
        .id();
    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.selected_id = Some(eid.index() as u64);
    }
    app.update();

    let insp = app.world().resource::<InspectorState>();
    assert!(
        insp.reflected_components
            .iter()
            .any(|(p, _)| p == "bsengine_scene::types::PrimitiveMesh"),
        "PrimitiveMesh must appear in Reflected Fields"
    );
    assert!(
        insp.reflected_components
            .iter()
            .any(|(p, _)| p == "bsengine_scene::types::ScriptPath"),
        "ScriptPath must appear in Reflected Fields"
    );
}

#[test]
fn inspector_cmd_attach_component_by_type_reaches_reflect_queue() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::AttachComponentByType {
            id: eid.index() as u64,
            type_path: "bsengine_core::camera::Camera".to_string(),
        });
    }
    app.update();

    assert!(
        app.world().get::<bsengine_core::Camera>(eid).is_some(),
        "Camera was not attached end-to-end via InspectorCmd"
    );
}

#[test]
fn inspector_cmd_attach_component_by_type_inserts_a_real_default_primitive_mesh() {
    // Full verification would confirm `resolve_primitives`
    // (bsengine-runtime's system reacting to `Added<PrimitiveMesh>` by
    // inserting a derived `MeshRenderer`) still fires when the component
    // arrives via this reflected path rather than a typed
    // `commands.insert()`. That's not reachable from this crate's test
    // harness: `resolve_primitives` lives in `bsengine-runtime`, which
    // is a binary-only crate (no `[lib]` target -- see its Cargo.toml,
    // `[[bin]]` only) that itself depends on `bsengine-editor`, so there
    // is no way to add its plugin here without an unresolvable cycle
    // (and nothing to `use` even if there weren't one). See
    // `editor_command_detach_primitive_mesh_also_removes_derived_mesh_renderer`
    // above for the same limitation on the older typed-command path.
    //
    // This narrower test instead confirms the piece that *is* testable
    // here: `InspectorCmd::AttachComponentByType` (the unified Add
    // Component menu's mechanism) inserts a genuine, usable
    // `PrimitiveMesh` component via `ReflectComponent::insert`+
    // `ReflectDefault` -- not just a UI-only stub -- with its
    // `#[default]` variant (`Primitive::Cube`), the same starting point
    // a typed `commands.insert(PrimitiveMesh::default())` would produce.
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::AttachComponentByType {
            id: eid.index() as u64,
            type_path: "bsengine_scene::types::PrimitiveMesh".to_string(),
        });
    }
    app.update();

    let mesh = app
        .world()
        .get::<bsengine_scene::PrimitiveMesh>(eid)
        .expect(
            "PrimitiveMesh should have been attached via the reflected \
             AttachComponentByType path",
        );
    assert_eq!(
        mesh.0,
        bsengine_scene::Primitive::Cube,
        "the default PrimitiveMesh attached via ReflectDefault should be \
         Primitive::Cube per its #[default]"
    );
}

#[test]
fn editor_plugin_registers_reflected_component_types() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    let registry = app.world().resource::<bevy_ecs::reflect::AppTypeRegistry>();
    let registry = registry.read();
    assert!(
        registry
            .get(std::any::TypeId::of::<bsengine_core::Camera>())
            .is_some(),
        "Camera not registered in AppTypeRegistry"
    );
    assert!(
        registry
            .get(std::any::TypeId::of::<bsengine_core::PointLight>())
            .is_some(),
        "PointLight not registered in AppTypeRegistry"
    );
}

#[test]
fn editor_plugin_registers_lights_and_material_reflected_types() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    let registry = app.world().resource::<bevy_ecs::reflect::AppTypeRegistry>();
    let registry = registry.read();
    assert!(
        registry
            .get(std::any::TypeId::of::<bsengine_core::DirectionalLight>())
            .is_some(),
        "DirectionalLight not registered in AppTypeRegistry"
    );
    assert!(
        registry
            .get(std::any::TypeId::of::<bsengine_core::SpotLight>())
            .is_some(),
        "SpotLight not registered in AppTypeRegistry"
    );
    assert!(
        registry
            .get(std::any::TypeId::of::<bsengine_core::Material>())
            .is_some(),
        "Material not registered in AppTypeRegistry"
    );
}

#[test]
fn editor_snapshot_reflects_named_entities() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.world_mut().spawn(Name("Hero".to_string()));
    app.world_mut().spawn(Name("Camera".to_string()));
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    let names: Vec<_> = snapshot
        .entities
        .iter()
        .filter_map(|e| e.name.as_deref())
        .collect();
    assert!(names.contains(&"Hero"), "expected Hero in {:?}", names);
    assert!(names.contains(&"Camera"), "expected Camera in {:?}", names);
}

#[test]
fn editor_snapshot_includes_transform_position() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.world_mut().spawn((
        Name("Box".to_string()),
        Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
    ));
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    let entity = snapshot
        .entities
        .iter()
        .find(|e| e.name.as_deref() == Some("Box"))
        .expect("Box not found");
    let pos = entity.position.expect("Box has no position");
    assert!((pos[0] - 1.0).abs() < 1e-5);
    assert!((pos[1] - 2.0).abs() < 1e-5);
    assert!((pos[2] - 3.0).abs() < 1e-5);
}

#[test]
fn inspector_set_selection_updates_selection_resource() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id_a = app.world_mut().spawn(Name("A".to_string())).id().index() as u64;
    let id_b = app.world_mut().spawn(Name("B".to_string())).id().index() as u64;
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::SetSelection {
            ids: vec![id_a, id_b],
        });
    app.update();

    let selection = app
        .world()
        .resource::<EditorSelectionResource>()
        .0
        .lock()
        .unwrap();
    assert!(selection.contains(&id_a));
    assert!(selection.contains(&id_b));
    assert_eq!(selection.len(), 2);
}

#[test]
fn editor_snapshot_reflects_multi_selection() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id_a = app.world_mut().spawn(Name("A".to_string())).id().index() as u64;
    let id_b = app.world_mut().spawn(Name("B".to_string())).id().index() as u64;
    let id_c = app.world_mut().spawn(Name("C".to_string())).id().index() as u64;
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::SetSelection {
            ids: vec![id_a, id_b],
        });
    app.update();
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    let selected = |id: u64| {
        snapshot
            .entities
            .iter()
            .find(|e| e.id == id)
            .unwrap()
            .selected
    };
    assert!(selected(id_a));
    assert!(selected(id_b));
    assert!(!selected(id_c));
}

#[test]
fn inspector_set_selection_replaces_previous_selection() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id_a = app.world_mut().spawn(Name("A".to_string())).id().index() as u64;
    let id_b = app.world_mut().spawn(Name("B".to_string())).id().index() as u64;
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::SetSelection { ids: vec![id_a] });
    app.update();
    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::SetSelection { ids: vec![id_b] });
    app.update();

    let selection = app
        .world()
        .resource::<EditorSelectionResource>()
        .0
        .lock()
        .unwrap();
    assert!(!selection.contains(&id_a));
    assert!(selection.contains(&id_b));
    assert_eq!(selection.len(), 1);
}

#[test]
fn undo_restores_despawned_entity() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id = app
        .world_mut()
        .spawn((
            Name("Box".to_string()),
            Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
        ))
        .id()
        .index() as u64;
    app.update();

    {
        let queue_res = app.world().resource::<EditorCommandQueueResource>();
        queue_res
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::Despawn { entity_id: id });
    }
    app.update();
    app.update();

    {
        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        assert!(!snapshot
            .entities
            .iter()
            .any(|e| e.name.as_deref() == Some("Box")));
    }

    app.world_mut()
        .resource_mut::<InspectorState>()
        .request_undo = true;
    app.update();
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    let restored = snapshot
        .entities
        .iter()
        .find(|e| e.name.as_deref() == Some("Box"))
        .expect("Box should be restored by undo");
    let pos = restored.position.expect("restored Box has a transform");
    assert!((pos[0] - 1.0).abs() < 1e-5);
    assert!((pos[1] - 2.0).abs() < 1e-5);
    assert!((pos[2] - 3.0).abs() < 1e-5);
}

#[test]
fn redo_reapplies_despawn_after_undo() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id = app.world_mut().spawn(Name("Box".to_string())).id().index() as u64;
    app.update();

    {
        let queue_res = app.world().resource::<EditorCommandQueueResource>();
        queue_res
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::Despawn { entity_id: id });
    }
    app.update();
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .request_undo = true;
    app.update();
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .request_redo = true;
    app.update();
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    assert!(
        !snapshot
            .entities
            .iter()
            .any(|e| e.name.as_deref() == Some("Box")),
        "redo should reapply the despawn"
    );
}

#[test]
fn undo_reverts_position_change() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id = app
        .world_mut()
        .spawn((
            Name("Box".to_string()),
            Transform::from_position(Vec3::new(1.0, 0.0, 0.0)),
        ))
        .id()
        .index() as u64;
    app.update();

    {
        let queue_res = app.world().resource::<EditorCommandQueueResource>();
        queue_res
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::SetPosition {
                entity_id: id,
                x: 9.0,
                y: 9.0,
                z: 9.0,
            });
    }
    app.update();
    app.update();

    {
        let snapshot = app
            .world()
            .resource::<EditorSnapshotResource>()
            .0
            .lock()
            .unwrap();
        let pos = snapshot
            .entities
            .iter()
            .find(|e| e.id == id)
            .and_then(|e| e.position)
            .expect("Box has a transform");
        assert!((pos[0] - 9.0).abs() < 1e-5);
    }

    app.world_mut()
        .resource_mut::<InspectorState>()
        .request_undo = true;
    app.update();
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    let pos = snapshot
        .entities
        .iter()
        .find(|e| e.id == id)
        .and_then(|e| e.position)
        .expect("Box still has a transform after undo");
    assert!(
        (pos[0] - 1.0).abs() < 1e-5,
        "expected x reverted to 1.0, got {}",
        pos[0]
    );
}

#[test]
fn undo_with_empty_history_does_not_panic() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .request_undo = true;
    app.update();
    app.update();
}

#[test]
fn inspector_duplicate_cmd_spawns_a_copy() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let id = app
        .world_mut()
        .spawn((
            Name("Box".to_string()),
            Transform::from_position(Vec3::new(1.0, 2.0, 3.0)),
        ))
        .id()
        .index() as u64;
    app.update();

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::Duplicate { id });
    app.update();
    app.update();

    let snapshot = app
        .world()
        .resource::<EditorSnapshotResource>()
        .0
        .lock()
        .unwrap();
    assert!(
        snapshot
            .entities
            .iter()
            .any(|e| e.name.as_deref() == Some("Box (copy)")),
        "expected a duplicated entity named 'Box (copy)'"
    );
    assert_eq!(
        snapshot
            .entities
            .iter()
            .filter(|e| e.name.as_deref() == Some("Box"))
            .count(),
        1,
        "original Box should still exist exactly once"
    );
}

#[test]
fn update_editor_snapshot_and_populate_inspector_report_is_prefab_instance() {
    let mut app = new_app();
    app.add_plugins(EditorPlugin);

    let with_instance = app
        .world_mut()
        .spawn((
            Name("HasInstance".to_string()),
            bsengine_core::PrefabInstance {
                source_path: "assets/prefabs/x.ron".to_string(),
            },
        ))
        .id();
    let without_instance = app.world_mut().spawn(Name("NoInstance".to_string())).id();

    app.update(); // update_editor_snapshot + populate_inspector capture both

    let inspector = app.world().resource::<InspectorState>();
    let with_info = inspector
        .entities
        .iter()
        .find(|e| e.id == with_instance.index() as u64)
        .unwrap();
    let without_info = inspector
        .entities
        .iter()
        .find(|e| e.id == without_instance.index() as u64)
        .unwrap();
    assert!(
        with_info.is_prefab_instance,
        "entity with PrefabInstance must report true"
    );
    assert!(
        !without_info.is_prefab_instance,
        "entity without PrefabInstance must report false"
    );
}

#[test]
fn inspector_cmd_set_visible_writes_the_component_the_renderer_actually_reads() {
    // Regression test: `EditorCommand::SetVisible` used to insert a
    // separate, unreflected `bsengine_editor::snapshot::Visible(bool)`
    // that bsengine-render's culling and bsengine-scripting's
    // `Bsengine.setVisible`/`getVisible` never read (they only look at
    // `bsengine_core::Visible.is_visible`) -- so toggling "Visible" in
    // the Inspector had no effect on what was actually rendered.
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let mut insp = app.world_mut().resource_mut::<InspectorState>();
        insp.cmd_queue.push(InspectorCmd::SetVisible {
            id: eid.index() as u64,
            visible: false,
        });
    }
    app.update();
    assert_eq!(
        app.world().get::<bsengine_core::Visible>(eid),
        Some(&bsengine_core::Visible { is_visible: false }),
        "SetVisible must write bsengine_core::Visible, the type bsengine-render and \
         bsengine-scripting actually read -- not an editor-only phantom component"
    );
}

#[test]
fn editor_command_attach_and_detach_script() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue.0.lock().unwrap().push(EditorCommand::AttachScript {
            entity_id: eid.index() as u64,
            path: "assets/scripts/foo.js".to_string(),
        });
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<bsengine_scene::ScriptPath>(eid)
            .map(|s| s.0.clone()),
        Some("assets/scripts/foo.js".to_string())
    );

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue.0.lock().unwrap().push(EditorCommand::DetachScript {
            entity_id: eid.index() as u64,
        });
    }
    app.update();
    assert!(app.world().get::<bsengine_scene::ScriptPath>(eid).is_none());
}

#[test]
fn editor_command_attach_and_detach_primitive_mesh() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::AttachPrimitiveMesh {
                entity_id: eid.index() as u64,
                primitive: bsengine_scene::Primitive::Capsule,
            });
    }
    app.update();
    assert_eq!(
        app.world()
            .get::<bsengine_scene::PrimitiveMesh>(eid)
            .map(|p| p.0.clone()),
        Some(bsengine_scene::Primitive::Capsule)
    );

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::DetachPrimitiveMesh {
                entity_id: eid.index() as u64,
            });
    }
    app.update();
    assert!(app
        .world()
        .get::<bsengine_scene::PrimitiveMesh>(eid)
        .is_none());
}

#[test]
fn editor_command_detach_primitive_mesh_also_removes_derived_mesh_renderer() {
    // `resolve_primitives` (bsengine-runtime) reacts to `Added<PrimitiveMesh>`
    // by inserting a derived `MeshRenderer` -- not exercised by this test's
    // plugin set, so it's simulated directly here. `DetachPrimitiveMesh`
    // must remove both, or the entity keeps rendering a stale mesh after
    // the Inspector's "Remove" button supposedly cleared it.
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    let eid = app.world_mut().spawn(Name("Target".to_string())).id();
    app.update();

    app.world_mut()
        .entity_mut(eid)
        .insert(bsengine_scene::PrimitiveMesh(
            bsengine_scene::Primitive::Capsule,
        ))
        .insert(bsengine_render::MeshRenderer { mesh_id: 1 });

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(EditorCommand::DetachPrimitiveMesh {
                entity_id: eid.index() as u64,
            });
    }
    app.update();
    assert!(
        app.world()
            .get::<bsengine_scene::PrimitiveMesh>(eid)
            .is_none(),
        "PrimitiveMesh should be removed"
    );
    assert!(
        app.world()
            .get::<bsengine_render::MeshRenderer>(eid)
            .is_none(),
        "derived MeshRenderer must also be removed, or the entity keeps rendering a stale mesh"
    );
}

#[test]
fn spawn_mesh_asset_command_spawns_entity_with_name_and_gltf_asset() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue.0.lock().unwrap().push(EditorCommand::SpawnMeshAsset {
            name: "Rock".to_string(),
            path: "assets/models/rock.glb".to_string(),
        });
    }
    app.update();

    let mut query = app
        .world_mut()
        .query::<(&Name, &bsengine_gltf::GltfAsset)>();
    let (name, gltf_asset) = query
        .iter(app.world())
        .next()
        .expect("expected one entity with Name + GltfAsset");
    assert_eq!(name.0, "Rock");
    assert_eq!(gltf_asset.path, "assets/models/rock.glb");
}

#[test]
fn spawn_mesh_asset_command_resolves_path_against_project_dir() {
    let mut app = new_app();
    app.insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    {
        let queue = app.world().resource::<EditorCommandQueueResource>();
        queue.0.lock().unwrap().push(EditorCommand::SpawnMeshAsset {
            name: "Rock".to_string(),
            path: "assets/models/rock.glb".to_string(),
        });
    }
    app.update();

    let mut query = app
        .world_mut()
        .query::<(&Name, &bsengine_gltf::GltfAsset)>();
    let (name, gltf_asset) = query
        .iter(app.world())
        .next()
        .expect("expected one entity with Name + GltfAsset");
    assert_eq!(name.0, "Rock");
    assert_eq!(gltf_asset.path, "games/demo/assets/models/rock.glb");
}

/// Verifies the property List-append/enum-variant-switch actually
/// depend on -- these three types have a `ReflectDefault` in the real
/// app's registry, not just in reflect_ui.rs's own unit-test-local
/// registries. A pass here doesn't uniquely attribute to this file's
/// explicit `register_type` calls for these three types specifically:
/// bevy_reflect's `register_type_dependencies` already walks
/// `Transform`'s/`ScriptPath`'s own fields and would supply the same
/// `ReflectDefault`s transitively even without them (confirmed by
/// temporarily removing the explicit calls and re-running this test).
/// Both paths are acceptable; this test guards the end state either way.
#[test]
fn app_type_registry_has_default_for_string_and_glam_wrappers() {
    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.update();

    let registry = app
        .world()
        .resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .read();
    for (type_id, label) in [
        (std::any::TypeId::of::<String>(), "String"),
        (
            std::any::TypeId::of::<bsengine_core::ReflectVec3>(),
            "ReflectVec3",
        ),
        (
            std::any::TypeId::of::<bsengine_core::ReflectQuat>(),
            "ReflectQuat",
        ),
    ] {
        assert!(
            registry
                .get_type_data::<bevy_reflect::std_traits::ReflectDefault>(type_id)
                .is_some(),
            "{label} must have a registered ReflectDefault for the List-append and \
             enum-variant-switch UI features to work in the real app"
        );
    }
}

#[test]
fn process_prefab_commands_instantiates_a_queued_prefab() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
    std::fs::write(
        root.join("assets/prefabs/enemy.ron"),
        r#"PrefabDescriptor(entities: [EntityDescriptor(name: "Enemy", primitive: Some(Cube))])"#,
    )
    .unwrap();

    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.insert_resource(bsengine_core::ProjectDir(
        root.to_string_lossy().to_string(),
    ));

    {
        let queue = app
            .world()
            .resource::<crate::snapshot::PrefabCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::PrefabInstantiateCommand {
                path: "assets/prefabs/enemy.ron".to_string(),
                name: None,
                x: 1.0,
                y: 0.0,
                z: 0.0,
                parent_id: None,
            });
    }

    app.update();

    let mut q = app.world_mut().query::<&Name>();
    let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
    assert!(
        names.iter().any(|n| n.starts_with("Enemy#")),
        "queued prefab command should have been instantiated by the next update, names: {names:?}"
    );
}

#[test]
fn process_prefab_commands_pushes_undo_checkpoint() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
    std::fs::write(
        root.join("assets/prefabs/enemy.ron"),
        r#"PrefabDescriptor(entities: [EntityDescriptor(name: "Enemy", primitive: Some(Cube))])"#,
    )
    .unwrap();

    let mut app = new_app();
    app.add_plugins(McpPlugin);
    app.add_plugins(EditorPlugin);
    app.insert_resource(bsengine_core::ProjectDir(
        root.to_string_lossy().to_string(),
    ));
    app.update();

    let history_len_before = app
        .world()
        .resource::<crate::snapshot::EditorHistoryResource>()
        .0
        .lock()
        .unwrap()
        .undo_stack
        .len();

    {
        let queue = app
            .world()
            .resource::<crate::snapshot::PrefabCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::PrefabInstantiateCommand {
                path: "assets/prefabs/enemy.ron".to_string(),
                name: None,
                x: 1.0,
                y: 0.0,
                z: 0.0,
                parent_id: None,
            });
    }
    app.update();

    let history_len_after = app
        .world()
        .resource::<crate::snapshot::EditorHistoryResource>()
        .0
        .lock()
        .unwrap()
        .undo_stack
        .len();
    assert_eq!(
        history_len_after,
        history_len_before + 1,
        "process_prefab_commands should push an undo checkpoint, same as ReflectCommand/EditorCommand do"
    );
}

#[test]
fn process_prefab_apply_commands_applies_a_queued_entity() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("assets/prefabs/queued.ron");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    let source = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "Root", primitive: Some(Cube)),
        EntityDescriptor(
            name: "Lamp",
            parent: Some("Root"),
            point_light: Some((color: (1.0, 1.0, 1.0), intensity: 1.0, range: 10.0)),
        ),
    ])"#;
    std::fs::write(&path, source).unwrap();

    let mut app = new_app();
    bsengine_scene::register_gameplay_reflect_types(&mut app);
    app.add_plugins(EditorPlugin);

    let baseline: bsengine_scene::types::PrefabDescriptor = ron::from_str(source).unwrap();
    let root = bsengine_scene::instantiate_prefab(
        app.world_mut(),
        &baseline,
        path.to_str().unwrap(),
        Some("MyQueued"),
        None,
        None,
    )
    .unwrap();
    app.world_mut()
        .entity_mut(root)
        .insert(bsengine_core::PrefabInstanceBaseline {
            synced_ron: source.to_string(),
        });

    let lamp = {
        let mut q = app
            .world_mut()
            .query::<(bevy_ecs::prelude::Entity, &bsengine_scene::Name)>();
        q.iter(app.world())
            .find(|(_, n)| n.0.starts_with("Lamp#"))
            .map(|(e, _)| e)
            .unwrap()
    };
    {
        let mut pl = app
            .world_mut()
            .get_mut::<bsengine_core::PointLight>(lamp)
            .unwrap();
        pl.intensity = 5.0;
    }

    let root_id = root.index() as u64;
    {
        let queue = app
            .world()
            .resource::<crate::snapshot::PrefabApplyCommandQueueResource>();
        queue
            .0
            .lock()
            .unwrap()
            .push(crate::snapshot::PrefabApplyCommand { entity_id: root_id });
    }

    app.update();

    let written = std::fs::read_to_string(&path).unwrap();
    let parsed: bsengine_scene::types::PrefabDescriptor = ron::from_str(&written).unwrap();
    let lamp_out = parsed.entities.iter().find(|e| e.name == "Lamp").unwrap();
    assert_eq!(
        lamp_out.point_light.as_ref().unwrap().intensity,
        5.0,
        "queuing a PrefabApplyCommand and running one app.update() must apply it"
    );
}

#[test]
fn save_entities_as_prefab_collects_the_subtree_and_writes_a_ron_file() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());

    let entities = vec![
        entity_info(1, "Root", None),
        entity_info(2, "Child", Some(1)),
        entity_info(3, "Grandchild", Some(2)),
        entity_info(4, "Unrelated", None),
    ];

    let path = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap();
    assert!(
        path.ends_with("assets/prefabs/boss.ron"),
        "unexpected path: {path}"
    );

    let content = std::fs::read_to_string(&path).unwrap();
    let parsed: bsengine_scene::types::PrefabDescriptor = ron::from_str(&content).unwrap();
    let names: Vec<&str> = parsed.entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names.len(),
        3,
        "expected Root+Child+Grandchild, not Unrelated: {names:?}"
    );
    assert!(names.contains(&"Root"));
    assert!(names.contains(&"Child"));
    assert!(names.contains(&"Grandchild"));

    let root_desc = parsed.entities.iter().find(|e| e.name == "Root").unwrap();
    assert!(
        root_desc.parent.is_none(),
        "prefab root must have no parent:, got {:?}",
        root_desc.parent
    );
}

#[test]
fn save_entities_as_prefab_drops_the_roots_link_to_a_parent_outside_the_subtree() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![
        entity_info(1, "Outside", None),
        entity_info(2, "SelectedRoot", Some(1)),
        entity_info(3, "Child", Some(2)),
    ];
    let path = save_entities_as_prefab(&entities, 2, "sub", Some(&project_dir)).unwrap();
    let parsed: bsengine_scene::types::PrefabDescriptor =
        ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    assert_eq!(
        parsed.entities.len(),
        2,
        "the entity outside the selected subtree must not be included: {:?}",
        parsed.entities.iter().map(|e| &e.name).collect::<Vec<_>>()
    );
    let root_desc = parsed
        .entities
        .iter()
        .find(|e| e.name == "SelectedRoot")
        .unwrap();
    assert!(root_desc.parent.is_none());
}

#[test]
fn save_entities_as_prefab_auto_suffixes_on_a_name_collision() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![entity_info(1, "Boss", None)];

    let first = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap();
    let second = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap();
    assert_ne!(first, second);
    assert!(
        second.ends_with("assets/prefabs/boss#2.ron"),
        "unexpected: {second}"
    );
    assert!(std::path::Path::new(&first).exists());
    assert!(std::path::Path::new(&second).exists());
}

#[test]
fn save_entities_as_prefab_rejects_a_root_with_no_name() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![EntityInfo {
        id: 1,
        name: None,
        ..Default::default()
    }];
    let err = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap_err();
    assert!(
        err.contains('1'),
        "error should mention the entity id: {err}"
    );
}

#[test]
fn save_entities_as_prefab_rejects_an_unnamed_descendant() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![
        entity_info(1, "Root", None),
        EntityInfo {
            id: 2,
            name: None,
            parent_id: Some(1),
            ..Default::default()
        },
    ];
    let err = save_entities_as_prefab(&entities, 1, "boss", Some(&project_dir)).unwrap_err();
    assert!(
        err.contains('2'),
        "error should mention the unnamed descendant's id: {err}"
    );
}

#[test]
fn save_entities_as_prefab_rejects_an_unknown_root_id() {
    let entities = vec![entity_info(1, "Root", None)];
    let err = save_entities_as_prefab(&entities, 999, "boss", None).unwrap_err();
    assert!(err.contains("999"));
}

#[test]
fn save_entities_as_prefab_rejects_a_name_with_path_traversal_characters() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![entity_info(1, "Root", None)];

    let err = save_entities_as_prefab(&entities, 1, "../evil", Some(&project_dir)).unwrap_err();
    assert!(
        err.contains("../evil"),
        "error should mention the rejected name: {err}"
    );

    let err2 = save_entities_as_prefab(&entities, 1, "sub/evil", Some(&project_dir)).unwrap_err();
    assert!(
        err2.contains("sub/evil"),
        "error should mention the rejected name: {err2}"
    );

    // Nothing should have been written anywhere, including outside the
    // temp project dir (walk up from the temp dir's parent to make sure
    // no "evil" file leaked out).
    let outside = dir.path().parent().unwrap().join("evil");
    assert!(
        !outside.exists(),
        "path traversal must not escape the project dir"
    );
    assert!(
        std::fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing should have been written into the project dir either"
    );
}

#[test]
fn save_entities_as_prefab_rejects_an_empty_name() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![entity_info(1, "Root", None)];

    let err = save_entities_as_prefab(&entities, 1, "", Some(&project_dir)).unwrap_err();
    assert!(err.contains("empty"), "unexpected error: {err}");
    assert!(
        std::fs::read_dir(dir.path()).unwrap().next().is_none(),
        "nothing should have been written for an empty name"
    );
}

#[test]
fn save_entities_as_prefab_terminates_when_the_snapshot_has_a_parent_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());
    let entities = vec![entity_info(1, "A", Some(2)), entity_info(2, "B", Some(1))];

    // The primary point of this test is that it returns at all: an
    // unguarded BFS over a parent_id cycle would loop forever and hang
    // the test binary. But returning isn't enough on its own to catch a
    // regression cleanly -- a future change that reintroduces the
    // missing guard would hang cargo test itself rather than fail it,
    // which is a bad failure mode to rely on alone. So this also pins
    // down the actual bounded result: both cycle members ended up in
    // the subtree exactly once each (matches HierarchyPanel::push_dfs's
    // same guard -- a visited id is skipped, not re-collected).
    let path = save_entities_as_prefab(&entities, 1, "cyclic", Some(&project_dir))
        .expect("a 2-entity mutual-parent cycle should still resolve, just bounded to visiting each member once");
    let parsed: bsengine_scene::types::PrefabDescriptor =
        ron::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    let names: Vec<&str> = parsed.entities.iter().map(|e| e.name.as_str()).collect();
    assert_eq!(
        names.len(),
        2,
        "each cycle member should be collected exactly once, not zero or repeated: {names:?}"
    );
    assert!(names.contains(&"A"));
    assert!(names.contains(&"B"));
}

#[test]
fn a_prefab_saved_from_live_entities_round_trips_through_instantiate_prefab() {
    let dir = tempfile::tempdir().unwrap();
    let project_dir = bsengine_core::ProjectDir(dir.path().to_string_lossy().to_string());

    let entities = vec![EntityInfo {
        id: 1,
        name: Some("Turret".to_string()),
        position: Some([2.0, 0.0, 0.0]),
        primitive: Some(bsengine_scene::Primitive::Cube),
        ..Default::default()
    }];

    let path = save_entities_as_prefab(&entities, 1, "turret", Some(&project_dir)).unwrap();

    let mut app = new_app();
    let root =
        bsengine_scene::instantiate_prefab_from_path(app.world_mut(), &path, None, None, None)
            .expect("a prefab saved by save_entities_as_prefab must re-instantiate cleanly");

    let name = app.world().get::<Name>(root).unwrap().0.clone();
    assert!(
        name.starts_with("Turret#"),
        "expected an auto-suffixed instance name, got {name}"
    );
    let transform = app.world().get::<Transform>(root).unwrap();
    assert!(
        (transform.position.x - 2.0).abs() < 1e-4,
        "position did not round-trip: {:?}",
        transform.position
    );
}

/// The Particles panel's rows come from the live emitters, after the
/// entity snapshot is rebuilt: an alive count that lagged a frame, or
/// that was wiped by `populate_inspector`, would show every effect as
/// empty.
#[test]
fn the_inspector_snapshot_carries_each_emitters_live_count() {
    let (mut app, burst_only, continuous) = app_with_two_emitters();
    app.update();
    let insp = app.world().resource::<InspectorState>();
    let find = |e: bevy_ecs::entity::Entity| {
        insp.entities
            .iter()
            .find(|i| i.id == e.index() as u64)
            .and_then(|i| i.particles)
            .expect("an entity with an emitter must carry a particle snapshot")
    };
    assert_eq!(find(burst_only).alive, 3);
    assert_eq!(find(burst_only).rate, 0.0);
    assert_eq!(find(burst_only).burst_count, 7);
    assert_eq!(find(continuous).rate, 12.0);
    assert!(
        insp.entities
            .iter()
            .filter(|i| i.particles.is_some())
            .count()
            == 2,
        "only the two emitters carry one"
    );
}

/// Burst reaches the one emitter it names and no other; the queued
/// count is the emitter's own, as a script's burst would be.
#[test]
fn particle_burst_queues_a_burst_on_the_named_emitter_only() {
    let (mut app, burst_only, continuous) = app_with_two_emitters();
    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::ParticleBurst {
            id: burst_only.index() as u64,
        });
    app.update();
    assert_eq!(emitter(&app, burst_only).pending_burst, 7);
    assert_eq!(
        emitter(&app, continuous).pending_burst,
        0,
        "the other emitter is untouched"
    );
}

/// Restart's two meanings of "start": a continuous effect comes back
/// empty and emitting, a burst-only effect comes back with its burst
/// queued -- and `None` does it to every emitter.
#[test]
fn particle_restart_drops_live_particles_and_requeues_a_burst_only_effect() {
    let (mut app, burst_only, continuous) = app_with_two_emitters();
    assert_eq!(
        emitter(&app, burst_only).live.len(),
        3,
        "premise: live particles to drop"
    );

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::ParticleRestart {
            id: Some(continuous.index() as u64),
        });
    app.update();
    let smoke = emitter(&app, continuous);
    assert!(smoke.live.is_empty(), "restart drops the live particles");
    assert_eq!(smoke.spawn_debt, 0.0, "and the fractional carry");
    assert_eq!(smoke.pending_burst, 0, "a continuous effect gets no burst");
    assert_eq!(
        emitter(&app, burst_only).live.len(),
        3,
        "the other emitter is untouched"
    );

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::ParticleRestart { id: None });
    app.update();
    let sparks = emitter(&app, burst_only);
    assert!(sparks.live.is_empty(), "Restart All reaches every emitter");
    assert_eq!(
        sparks.pending_burst, 7,
        "a burst-only effect's start is its burst, so it is queued again"
    );
}

/// Selecting an asset walks the project into `asset_references`, through
/// the real system: a model two scenes name lists both, a leaf lists no
/// dependencies, and the texture nothing names is `reached: false`. The
/// walk is the packager's, so the second scene is found through the
/// script that names it, not because a test listed it.
#[test]
fn selecting_an_asset_lists_what_references_it_and_what_it_references() {
    let probe = ProjectProbe::new("walk");
    let mut app = new_app();
    app.add_plugins(EditorPlugin);
    app.insert_resource(bsengine_core::ProjectDir(
        probe.dir.to_string_lossy().to_string(),
    ));

    app.world_mut()
        .resource_mut::<InspectorState>()
        .select_asset("assets/models/hero.glb");
    app.update();
    {
        let insp = app.world().resource::<InspectorState>();
        let refs = insp
            .asset_references
            .as_ref()
            .expect("one update after selecting must walk the project");
        assert_eq!(refs.path, "assets/models/hero.glb");
        assert_eq!(refs.error, None);
        assert!(refs.reached, "the entry scene names the model");
        assert_eq!(
            refs.referencers,
            vec!["assets/scenes/level2.ron", "assets/scenes/main.ron"],
            "both scenes, the second reached only through the script"
        );
        assert!(refs.dependencies.is_empty(), "a model is a leaf");
    }

    app.world_mut()
        .resource_mut::<InspectorState>()
        .select_asset("assets/scripts/hero.js");
    app.update();
    {
        let refs = app
            .world()
            .resource::<InspectorState>()
            .asset_references
            .clone()
            .expect("re-walked for the new selection");
        assert_eq!(refs.path, "assets/scripts/hero.js");
        assert_eq!(refs.referencers, vec!["assets/scenes/main.ron"]);
        assert_eq!(
            refs.dependencies,
            vec!["assets/scenes/level2.ron"],
            "a quoted path in a script is a dependency"
        );
    }

    app.world_mut()
        .resource_mut::<InspectorState>()
        .select_asset("assets/textures/unused.png");
    app.update();
    let refs = app
        .world()
        .resource::<InspectorState>()
        .asset_references
        .clone()
        .expect("walked");
    assert!(!refs.reached, "nothing names the texture");
    assert!(refs.referencers.is_empty());
    assert_eq!(refs.error, None);
}

/// The whole-project graph is walked when asked and only then: the
/// request fills it with the packager's edges, unreached assets and
/// dangling references, and a scene edited on disk afterwards is not
/// seen until the next request -- which a scene save makes.
#[test]
fn the_asset_graph_is_walked_on_request_and_again_after_a_scene_save() {
    let probe = ProjectProbe::new("graph");
    probe.write(
        "assets/scenes/level2.ron",
        r#"(entities: [(name: "Hero", gltf: Some(Path("assets/models/hero.glb")), texture: Some("assets/textures/gone.png"))])"#,
    );
    let mut app = new_app();
    app.add_plugins(EditorPlugin);
    app.insert_resource(bsengine_core::ProjectDir(
        probe.dir.to_string_lossy().to_string(),
    ));

    app.update();
    assert!(
        app.world()
            .resource::<InspectorState>()
            .asset_graph
            .is_none(),
        "premise: nothing asked, nothing walked"
    );

    app.world_mut()
        .resource_mut::<InspectorState>()
        .asset_graph_refresh = true;
    app.update();
    let insp = app.world().resource::<InspectorState>();
    assert!(!insp.asset_graph_refresh, "the request is consumed");
    let graph = insp.asset_graph.clone().expect("walked on request");
    assert_eq!(graph.error, None);
    let edge = |a: &str, b: &str| (a.to_string(), b.to_string());
    assert!(
        graph
            .edges
            .contains(&edge("assets/scripts/hero.js", "assets/scenes/level2.ron")),
        "the packager's edges, script mentions included; got {:?}",
        graph.edges
    );
    assert_eq!(graph.unreferenced, vec!["assets/textures/unused.png"]);
    assert_eq!(
        graph.missing,
        vec![edge("assets/scenes/level2.ron", "assets/textures/gone.png")]
    );

    // The scene changes on disk; without a request the graph is what
    // it was.
    probe.write(
        "assets/scenes/level2.ron",
        r#"(entities: [(name: "Hero", texture: Some("assets/textures/unused.png"))])"#,
    );
    app.update();
    assert_eq!(
        app.world()
            .resource::<InspectorState>()
            .asset_graph
            .as_ref()
            .map(|g| g.unreferenced.clone()),
        Some(vec!["assets/textures/unused.png".to_string()]),
        "no request, no re-walk"
    );

    // A save of any scene is a request: the saved file here is one the
    // walk does not reach (the editor writes only what it holds), but
    // the request it makes re-reads the whole project.
    app.world()
        .resource::<EditorCommandQueueResource>()
        .0
        .lock()
        .unwrap()
        .push(EditorCommand::SaveScene {
            path: probe
                .dir
                .join("assets/scenes/saved.ron")
                .to_string_lossy()
                .to_string(),
        });
    app.update();
    let graph = app
        .world()
        .resource::<InspectorState>()
        .asset_graph
        .clone()
        .expect("still there");
    assert_eq!(
        graph.unreferenced,
        vec!["assets/scenes/saved.ron"],
        "after the save the walk sees level2.ron now naming the texture, and the \
         freshly saved scene -- which nothing names -- as the one unreached asset"
    );
    assert!(
        graph.missing.is_empty(),
        "and the dangling reference is gone"
    );
}

/// A project the walk cannot start in records why, once, instead of
/// leaving the snapshot empty and walking again every frame.
#[test]
fn a_project_without_a_manifest_records_the_error_in_the_snapshot() {
    let probe = ProjectProbe::new("no-manifest");
    std::fs::remove_file(probe.dir.join("project.toml")).unwrap();
    let mut app = new_app();
    app.add_plugins(EditorPlugin);
    app.insert_resource(bsengine_core::ProjectDir(
        probe.dir.to_string_lossy().to_string(),
    ));

    app.world_mut()
        .resource_mut::<InspectorState>()
        .select_asset("assets/models/hero.glb");
    app.update();
    let refs = app
        .world()
        .resource::<InspectorState>()
        .asset_references
        .clone()
        .expect("the failure is a snapshot, not an absence");
    assert!(
        refs.error
            .as_deref()
            .is_some_and(|e| e.contains("project.toml")),
        "the error names the manifest: {:?}",
        refs.error
    );
    assert!(!refs.reached);
}

/// The Inspector's whole import-settings loop, through the real
/// systems: selecting an asset reads its sidecar into the snapshot,
/// Apply writes the sidecar and the snapshot comes back *from disk* with
/// `recorded: true`, in the same frame -- `populate_asset_import_snapshot`
/// is ordered after the command drain for exactly that. The premise
/// (the first read reports the defaults, unrecorded) is asserted so a
/// write that silently failed cannot pass as "still the defaults".
#[test]
fn selecting_an_asset_reads_its_import_settings_and_apply_writes_them_back() {
    use bsengine_asset::identity::{sidecar_path, Sidecar};
    use bsengine_core::{ImportSettings, TextureFilter, TextureImportSettings};

    let probe = TextureProbe::new("round-trip");
    let mut app = new_app();
    app.add_plugins(EditorPlugin);

    app.world_mut()
        .resource_mut::<InspectorState>()
        .select_asset(probe.path.clone());
    app.update();
    {
        let insp = app.world().resource::<InspectorState>();
        let snapshot = insp
            .asset_import
            .as_ref()
            .expect("one update after selecting must read the sidecar");
        assert_eq!(snapshot.path, probe.path);
        assert!(!snapshot.recorded, "premise: nothing is recorded yet");
        assert_eq!(
            snapshot.settings,
            ImportSettings::Texture(TextureImportSettings::default())
        );
        assert_eq!(snapshot.edit, snapshot.settings);
        assert!(insp.asset_import_error.is_none());
    }

    let tuned = ImportSettings::Texture(TextureImportSettings {
        srgb: false,
        filter: TextureFilter::Nearest,
        ..Default::default()
    });
    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::WriteImportSettings {
            path: probe.path.clone(),
            settings: tuned,
        });
    app.update();

    let on_disk = Sidecar::read(sidecar_path(&probe.path))
        .expect("readable")
        .expect("Apply must write a sidecar beside the texture");
    assert_eq!(on_disk.texture_import().filter, TextureFilter::Nearest);
    assert!(!on_disk.texture_import().srgb);

    let insp = app.world().resource::<InspectorState>();
    let snapshot = insp
        .asset_import
        .as_ref()
        .expect("the snapshot must be re-read in the frame the write landed");
    assert!(
        snapshot.recorded,
        "the re-read snapshot must say the settings are now recorded"
    );
    assert_eq!(snapshot.settings, tuned);
    assert_eq!(snapshot.edit, tuned, "nothing is pending after Apply");
    assert!(insp.asset_import_error.is_none());
}

/// The scan's rule reaches the Inspector: a sidecar that will not parse
/// is reported, not overwritten. Both the read and a later Apply leave
/// the bytes exactly as they were.
#[test]
fn a_broken_sidecar_is_reported_to_the_inspector_and_never_overwritten() {
    use bsengine_asset::identity::sidecar_path;
    use bsengine_core::{ImportSettings, TextureImportSettings};

    let probe = TextureProbe::new("broken");
    let meta = sidecar_path(&probe.path);
    std::fs::write(&meta, b"(guid: 7)").unwrap();

    let mut app = new_app();
    app.add_plugins(EditorPlugin);
    app.world_mut()
        .resource_mut::<InspectorState>()
        .select_asset(probe.path.clone());
    app.update();
    {
        let insp = app.world().resource::<InspectorState>();
        assert!(insp.asset_import.is_none());
        let error = insp
            .asset_import_error
            .as_deref()
            .expect("a broken sidecar must be reported");
        assert!(error.contains("could not be read"), "{error}");
    }

    app.world_mut()
        .resource_mut::<InspectorState>()
        .cmd_queue
        .push(InspectorCmd::WriteImportSettings {
            path: probe.path.clone(),
            settings: ImportSettings::Texture(TextureImportSettings::default()),
        });
    app.update();
    assert_eq!(
        std::fs::read(&meta).unwrap(),
        b"(guid: 7)",
        "Apply must not overwrite a sidecar it could not read"
    );
    assert!(app
        .world()
        .resource::<InspectorState>()
        .asset_import_error
        .is_some());
}
