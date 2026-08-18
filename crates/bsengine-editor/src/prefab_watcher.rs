//! Despawn-and-reinstantiate resync for prefab instances whose source file
//! changed on disk, plus the file watcher that detects those changes.
//!
//! [`resync_prefab_instances`] is the resync itself: given a project-relative
//! prefab path, it finds every live [`PrefabInstance`] root pointing at that
//! path, despawns each one's full subtree, and re-instantiates fresh from the
//! file's current content -- preserving the instance's original name,
//! transform, and parent. It takes a plain path and `&mut World`, so it can be
//! (and is, in this crate's tests) driven directly without any debouncer or
//! timing involved.
//!
//! This is a **destructive** sync: without override tracking (explicitly out
//! of scope -- see the design doc), any manual edit made directly to an
//! instance (added children, tweaked child transforms, attached components)
//! is lost the next time that instance's source prefab changes and gets
//! synced.

use bevy_ecs::prelude::{Entity, World};
use bsengine_core::{Parent, PrefabInstance, Transform};
use bsengine_scene::{Name, TransformDescriptor};

/// Despawns `root` and every entity transitively parented under it (via a
/// live [`Parent`] component chain), so a resync can safely re-instantiate a
/// fresh subtree in its place without leaving orphaned children behind.
///
/// Guards against a `Parent` cycle exactly like `save_entities_as_prefab`'s
/// BFS does (`crates/bsengine-editor/src/plugin.rs`): nothing that writes
/// `Parent` today checks for cycles, so a malformed live hierarchy reaching
/// this function must still terminate rather than loop forever.
fn despawn_subtree(world: &mut World, root: Entity) {
    let mut children_q = world.query::<(Entity, &Parent)>();
    let mut visited: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    let mut to_despawn = Vec::new();
    let mut frontier = vec![root];
    while let Some(cur) = frontier.pop() {
        if !visited.insert(cur) {
            continue;
        }
        to_despawn.push(cur);
        for (child, parent) in children_q.iter(world) {
            if parent.0 == cur {
                frontier.push(child);
            }
        }
    }
    for entity in to_despawn {
        world.despawn(entity);
    }
}

/// Resyncs every live [`PrefabInstance`] root whose `source_path` matches
/// `changed_source_path` (a project-relative path, e.g.
/// `"assets/prefabs/turret.ron"`): despawns each instance's full subtree and
/// re-instantiates it fresh from the file's current content, preserving each
/// instance's original name, transform, and parent.
///
/// A missing or unparseable file leaves every matching instance untouched
/// rather than despawning anything -- the existence/parse check happens
/// before any entity is touched, precisely so a bad edit can't destroy a
/// working instance on the way to failing.
pub(crate) fn resync_prefab_instances(world: &mut World, changed_source_path: &str) {
    let roots: Vec<Entity> = {
        let mut q = world.query::<(Entity, &PrefabInstance)>();
        q.iter(world)
            .filter(|(_, instance)| instance.source_path == changed_source_path)
            .map(|(entity, _)| entity)
            .collect()
    };
    if roots.is_empty() {
        return;
    }

    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();
    let resolved_path =
        bsengine_core::resolve_project_path(project_dir.as_ref(), changed_source_path);

    if !std::path::Path::new(&resolved_path).is_file() {
        tracing::warn!(
            "prefab live-sync: '{resolved_path}' no longer exists on disk; leaving {} \
             existing instance(s) of '{changed_source_path}' untouched",
            roots.len()
        );
        return;
    }
    let content = match std::fs::read_to_string(&resolved_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(
                "prefab live-sync: '{resolved_path}' could not be read ({e}); leaving \
                 {} existing instance(s) of '{changed_source_path}' untouched",
                roots.len()
            );
            return;
        }
    };
    let prefab = match ron::from_str::<bsengine_scene::types::PrefabDescriptor>(&content) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(
                "prefab live-sync: '{resolved_path}' failed to parse ({e}); leaving {} \
                 existing instance(s) of '{changed_source_path}' untouched",
                roots.len()
            );
            return;
        }
    };
    if let Err(e) = bsengine_scene::validate_prefab_descriptor(&prefab) {
        tracing::warn!(
            "prefab live-sync: '{resolved_path}' is not a valid instantiable prefab ({e}); \
             leaving {} existing instance(s) of '{changed_source_path}' untouched",
            roots.len()
        );
        return;
    }

    for root in roots {
        if world.get_entity(root).is_none() {
            tracing::warn!(
                "prefab live-sync: an instance root for '{changed_source_path}' was already \
                 despawned (likely a descendant of another matching instance that was resynced \
                 first); skipping it"
            );
            continue;
        }
        let Some(name) = world.get::<Name>(root).map(|n| n.0.clone()) else {
            continue;
        };
        let transform_override = world.get::<Transform>(root).map(|t| TransformDescriptor {
            position: t.position.0.to_array(),
            rotation: t.rotation.0.to_array(),
            scale: t.scale.0.to_array(),
        });
        let parent = world.get::<Parent>(root).map(|p| p.0);

        despawn_subtree(world, root);

        if let Err(e) = bsengine_scene::instantiate_prefab_from_path(
            world,
            &resolved_path,
            Some(&name),
            transform_override,
            parent,
        ) {
            tracing::warn!(
                "prefab live-sync: failed to resync instance '{name}' of \
                 '{changed_source_path}': {e}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_app::App;
    use bsengine_app::new_app;
    use bsengine_core::ProjectDir;

    fn register(app: &mut App) {
        app.register_type::<PrefabInstance>();
        app.register_type::<Transform>();
        app.register_type::<bsengine_core::GlobalTransform>();
        app.register_type::<Parent>();
    }

    /// Writes `<dir>/assets/prefabs/<name>.ron` and returns the
    /// project-relative path it should be addressed by.
    fn write_prefab(dir: &std::path::Path, name: &str, content: &str) -> String {
        let prefabs_dir = dir.join("assets").join("prefabs");
        std::fs::create_dir_all(&prefabs_dir).unwrap();
        std::fs::write(prefabs_dir.join(format!("{name}.ron")), content).unwrap();
        format!("assets/prefabs/{name}.ron")
    }

    const TURRET_V1: &str = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "Body", primitive: Some(Cube)),
        EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Cube)),
    ])"#;

    const TURRET_V2: &str = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "Body", primitive: Some(Cube)),
        EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Cube)),
        EntityDescriptor(name: "Scope", parent: Some("Body"), primitive: Some(Cube)),
    ])"#;

    #[test]
    fn resync_preserves_the_instances_name_transform_and_parent() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path = write_prefab(dir.path(), "turret", TURRET_V1);

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let anchor = app.world_mut().spawn(Name("Anchor".to_string())).id();
        let resolved = bsengine_core::resolve_project_path(Some(&project_dir), &source_path);
        let old_root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("MyTurret"),
            Some(TransformDescriptor {
                position: [1.0, 2.0, 3.0],
                ..Default::default()
            }),
            Some(anchor),
        )
        .unwrap();

        write_prefab(dir.path(), "turret", TURRET_V2);
        resync_prefab_instances(app.world_mut(), &source_path);

        let mut q = app.world_mut().query::<(Entity, &Name)>();
        let new_root = q
            .iter(app.world())
            .find(|(_, n)| n.0 == "MyTurret")
            .map(|(e, _)| e)
            .expect("the resynced instance must keep its exact original name");
        assert_ne!(
            new_root, old_root,
            "resync must spawn a fresh entity, not reuse the old id"
        );

        let transform = app.world().get::<Transform>(new_root).unwrap();
        assert!((transform.position.0.x - 1.0).abs() < 1e-5);
        assert!((transform.position.0.y - 2.0).abs() < 1e-5);
        assert!((transform.position.0.z - 3.0).abs() < 1e-5);

        let parent = app.world().get::<Parent>(new_root).unwrap();
        assert_eq!(
            parent.0, anchor,
            "resync must keep the instance parented where it was"
        );

        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(
            names.iter().any(|n| n.starts_with("Scope#")),
            "the resynced subtree must reflect the file's new content \
             (Scope is new in v2): {names:?}"
        );
    }

    #[test]
    fn resync_updates_every_simultaneous_instance_of_the_same_prefab() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path = write_prefab(dir.path(), "turret", TURRET_V1);

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let resolved = bsengine_core::resolve_project_path(Some(&project_dir), &source_path);
        bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("TurretA"),
            None,
            None,
        )
        .unwrap();
        bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("TurretB"),
            None,
            None,
        )
        .unwrap();

        write_prefab(dir.path(), "turret", TURRET_V2);
        resync_prefab_instances(app.world_mut(), &source_path);

        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(names.contains(&"TurretA".to_string()));
        assert!(names.contains(&"TurretB".to_string()));
        let scope_count = names.iter().filter(|n| n.starts_with("Scope#")).count();
        assert_eq!(
            scope_count, 2,
            "both instances must pick up the new Scope child, got names: {names:?}"
        );
    }

    #[test]
    fn resync_leaves_instances_untouched_when_the_source_file_is_deleted() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path = write_prefab(dir.path(), "turret", TURRET_V1);

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let resolved = bsengine_core::resolve_project_path(Some(&project_dir), &source_path);
        let root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        std::fs::remove_file(&resolved).unwrap();
        resync_prefab_instances(app.world_mut(), &source_path);

        assert!(
            app.world().get_entity(root).is_some(),
            "a deleted source file must leave the existing instance untouched, not despawned"
        );
    }

    #[test]
    fn resync_leaves_instances_untouched_when_the_source_file_fails_to_parse() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path = write_prefab(dir.path(), "turret", TURRET_V1);

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let resolved = bsengine_core::resolve_project_path(Some(&project_dir), &source_path);
        let root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        write_prefab(dir.path(), "turret", "not valid ron at all {{{");
        resync_prefab_instances(app.world_mut(), &source_path);

        assert!(
            app.world().get_entity(root).is_some(),
            "an unparseable source file must leave the existing instance untouched"
        );
        assert_eq!(
            app.world().get::<Name>(root).unwrap().0,
            "MyTurret",
            "the untouched instance must be entirely unchanged"
        );
    }

    #[test]
    fn resync_leaves_instances_untouched_when_the_source_file_is_structurally_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path = write_prefab(dir.path(), "turret", TURRET_V1);

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let resolved = bsengine_core::resolve_project_path(Some(&project_dir), &source_path);
        let root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        // Valid RON, but structurally uninstantiable: two root entities
        // (neither names a `parent:`), same shape as bsengine-scene's own
        // `instantiate_prefab_rejects_multiple_roots` test fixture.
        write_prefab(
            dir.path(),
            "turret",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "RootA", primitive: Some(Cube)),
                EntityDescriptor(name: "RootB", primitive: Some(Cube)),
            ])"#,
        );
        resync_prefab_instances(app.world_mut(), &source_path);

        assert!(
            app.world().get_entity(root).is_some(),
            "a structurally-invalid-but-parseable source file (e.g. two root entities) must \
             leave the existing instance untouched, not despawn it only to fail re-instantiation"
        );
        assert_eq!(
            app.world().get::<Name>(root).unwrap().0,
            "MyTurret",
            "the untouched instance must be entirely unchanged"
        );
    }

    const OUTER_V1: &str = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "Outer", primitive: Some(Cube)),
        EntityDescriptor(name: "Sibling", parent: Some("Outer"), primitive: Some(Cube)),
        EntityDescriptor(name: "Nested", parent: Some("Outer"), prefab: Some("assets/prefabs/nested.ron")),
    ])"#;

    const OUTER_V2: &str = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "Outer", primitive: Some(Cube)),
        EntityDescriptor(name: "Sibling", parent: Some("Outer"), primitive: Some(Cube)),
        EntityDescriptor(name: "Nested", parent: Some("Outer"), prefab: Some("assets/prefabs/nested.ron")),
        EntityDescriptor(name: "ExtraChild", parent: Some("Outer"), primitive: Some(Cube)),
    ])"#;

    const NESTED_V1: &str = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "NestedRoot", primitive: Some(Cube)),
    ])"#;

    const NESTED_V2: &str = r#"PrefabDescriptor(entities: [
        EntityDescriptor(name: "NestedRoot", primitive: Some(Cube)),
        EntityDescriptor(name: "NestedChild", parent: Some("NestedRoot"), primitive: Some(Cube)),
    ])"#;

    /// Sets up `dir/assets/prefabs/{outer,nested}.ron` (v1 each) and
    /// instantiates `outer.ron` once as `"OuterInstance"`. Returns the
    /// project-relative paths of both files.
    fn setup_nested_fixture(app: &mut App, project_dir: &ProjectDir) -> (String, String) {
        let dir = std::path::Path::new(&project_dir.0);
        let outer_path = write_prefab(dir, "outer", OUTER_V1);
        let nested_path = write_prefab(dir, "nested", NESTED_V1);

        let resolved_outer = bsengine_core::resolve_project_path(Some(project_dir), &outer_path);
        bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved_outer,
            Some("OuterInstance"),
            None,
            None,
        )
        .unwrap();

        (outer_path, nested_path)
    }

    #[test]
    fn resync_of_a_nested_prefabs_file_touches_only_that_nested_subtree() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);
        let (_outer_path, nested_path) = setup_nested_fixture(&mut app, &project_dir);

        let sibling_before = {
            let mut q = app.world_mut().query::<(Entity, &Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Sibling#"))
                .map(|(e, _)| e)
                .expect("Sibling should have spawned as part of the outer instance")
        };
        let nested_root_before = {
            let mut q = app.world_mut().query::<(Entity, &PrefabInstance)>();
            q.iter(app.world())
                .find(|(_, i)| i.source_path == nested_path)
                .map(|(e, _)| e)
                .expect("the nested prefab reference should have its own PrefabInstance")
        };

        write_prefab(dir.path(), "nested", NESTED_V2);
        resync_prefab_instances(app.world_mut(), &nested_path);

        assert!(
            app.world().get_entity(sibling_before).is_some(),
            "editing only the nested prefab's file must leave the outer instance's \
             unrelated sibling entity untouched"
        );

        let nested_root_after = {
            let mut q = app.world_mut().query::<(Entity, &PrefabInstance)>();
            q.iter(app.world())
                .find(|(_, i)| i.source_path == nested_path)
                .map(|(e, _)| e)
                .expect("a resynced nested instance must still exist")
        };
        assert_ne!(
            nested_root_after, nested_root_before,
            "the nested subtree must have been despawned and re-instantiated"
        );
        assert_eq!(
            app.world().get::<Parent>(nested_root_after).map(|p| p.0),
            app.world().get::<Parent>(sibling_before).map(|p| p.0),
            "the resynced nested instance must be reparented back under the same outer entity"
        );

        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(
            names.iter().any(|n| n.starts_with("NestedChild#")),
            "the resynced nested subtree must reflect nested.ron's new content: {names:?}"
        );
    }

    #[test]
    fn resync_of_an_outer_prefabs_file_resyncs_the_whole_instance_including_nested() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);
        let (outer_path, nested_path) = setup_nested_fixture(&mut app, &project_dir);

        let outer_root_before = {
            let mut q = app.world_mut().query::<(Entity, &Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0 == "OuterInstance")
                .map(|(e, _)| e)
                .expect("OuterInstance should have spawned")
        };

        write_prefab(dir.path(), "outer", OUTER_V2);
        resync_prefab_instances(app.world_mut(), &outer_path);

        let outer_root_after = {
            let mut q = app.world_mut().query::<(Entity, &Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0 == "OuterInstance")
                .map(|(e, _)| e)
                .expect("a resynced outer instance must still exist under the same name")
        };
        assert_ne!(
            outer_root_after, outer_root_before,
            "the whole outer subtree must have been despawned and re-instantiated"
        );

        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(
            names.iter().any(|n| n.starts_with("ExtraChild#")),
            "the resynced outer subtree must reflect outer.ron's new content: {names:?}"
        );

        let nested_instance_count = {
            let mut q = app.world_mut().query::<&PrefabInstance>();
            q.iter(app.world())
                .filter(|i| i.source_path == nested_path)
                .count()
        };
        assert_eq!(
            nested_instance_count, 1,
            "resyncing the outer file must re-resolve its nested prefab reference exactly once"
        );
    }
}
