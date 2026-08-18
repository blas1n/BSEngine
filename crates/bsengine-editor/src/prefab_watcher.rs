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

use bevy_app::{App, Plugin, Startup, Update};
use bevy_ecs::prelude::{Commands, Entity, IntoSystemConfigs, Res, Resource, World};
use bsengine_core::{Parent, PrefabInstance, Transform};
use bsengine_scene::{Name, TransformDescriptor};
use notify_debouncer_full::{
    new_debouncer,
    notify::{RecommendedWatcher, RecursiveMode, Watcher},
    DebounceEventResult, Debouncer, FileIdMap,
};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::Mutex;
use std::time::Duration;
use tracing::{info, warn};

/// Every project-relative prefab path this prefab's own entities reference
/// via a `prefab:` field, transitively -- i.e. every nested prefab that is a
/// compositional part of `prefab` itself, at any depth, resolved to the same
/// bare-path spelling [`PrefabInstance::source_path`] stores.
///
/// Exists so [`despawn_subtree`] can tell "a nested prefab reference `prefab`
/// itself authored, directly or through further nesting" apart from "an
/// unrelated instance someone reparented underneath via `SetParent`" -- both
/// end up looking identical in live ECS state (a `Parent` link plus the
/// child's own, different-sourced, `PrefabInstance`; see
/// `resolve_prefab_reference`/`spawn_scene_entities` in `bsengine_scene::plugin`,
/// which attaches a resolved prefab reference's `Parent` through the exact
/// same `parent:`-field pass every other entity gets, with no special case
/// for prefab-reference entities), so only the prefab file's own text --
/// recursively -- can answer which is which.
///
/// Recurses through each nested reference's own file, bounded by a
/// visited-set (so a cyclic reference terminates rather than looping
/// forever -- mirrors `RESOLVING_PREFABS`/`ResolvingGuard`'s cycle
/// discipline in `bsengine_scene::plugin`), because the instantiation
/// machinery this is protecting against (`instantiate_prefab_from_path` ->
/// `resolve_prefab_reference` -> recursion) supports arbitrary-depth
/// nesting, not just one level -- proven by
/// `bsengine_scene::plugin::tests::nested_prefab_reference_instantiates_recursively`.
/// A file this function can't read or parse (e.g. it was deleted, or is
/// itself invalid) is simply not recursed into further -- its own direct
/// reference is still recorded, just not anything IT might nest, since
/// `resync_prefab_instances`'s own read/parse of the top-level file is
/// already the gate for whether a resync proceeds at all; a broken nested
/// file is a pre-existing condition this function doesn't need to diagnose.
fn nested_prefab_source_paths(
    root_source_path: &str,
    prefab: &bsengine_scene::types::PrefabDescriptor,
    project_dir: Option<&bsengine_core::ProjectDir>,
) -> std::collections::HashSet<String> {
    // Seeded into `result`, not just `visited`: `own_source_paths` (this
    // function's return value) must itself contain the resync's own source
    // path, since `despawn_subtree` treats anything NOT in the returned set
    // as foreign -- including a second live instance that shares the exact
    // same source path as the one being resynced (two placements of the
    // same prefab, one reparented under the other). Seeding only `visited`
    // (the recursion cycle-guard) and leaving `result` to be populated
    // purely by `collect_nested_prefab_source_paths` would silently omit
    // `root_source_path` from the returned set whenever `prefab` has no
    // `prefab:` references of its own to walk -- which is the common case,
    // not an edge case.
    let mut result = std::collections::HashSet::new();
    result.insert(root_source_path.to_string());
    let mut visited = std::collections::HashSet::new();
    visited.insert(root_source_path.to_string());
    collect_nested_prefab_source_paths(prefab, project_dir, &mut visited, &mut result);
    result
}

/// Recursion helper for [`nested_prefab_source_paths`]. See its doc comment
/// for the cycle-guard and unreadable/unparseable-file rationale.
fn collect_nested_prefab_source_paths(
    prefab: &bsengine_scene::types::PrefabDescriptor,
    project_dir: Option<&bsengine_core::ProjectDir>,
    visited: &mut std::collections::HashSet<String>,
    result: &mut std::collections::HashSet<String>,
) {
    for entity in &prefab.entities {
        let Some(asset_ref) = entity.prefab.as_ref() else {
            continue;
        };
        let nested_path = asset_ref.path().to_string();
        if !visited.insert(nested_path.clone()) {
            continue;
        }
        result.insert(nested_path.clone());

        let resolved = bsengine_core::resolve_project_path(project_dir, &nested_path);
        let Ok(content) = std::fs::read_to_string(&resolved) else {
            continue;
        };
        let Ok(nested_prefab) = ron::from_str::<bsengine_scene::types::PrefabDescriptor>(&content)
        else {
            continue;
        };
        collect_nested_prefab_source_paths(&nested_prefab, project_dir, visited, result);
    }
}

/// Despawns `root` and every entity transitively parented under it (via a
/// live [`Parent`] component chain), so a resync can safely re-instantiate a
/// fresh subtree in its place without leaving orphaned children behind.
///
/// Guards against a `Parent` cycle exactly like `save_entities_as_prefab`'s
/// BFS does (`crates/bsengine-editor/src/plugin.rs`): nothing that writes
/// `Parent` today checks for cycles, so a malformed live hierarchy reaching
/// this function must still terminate rather than loop forever.
///
/// Also stops at (and does not despawn) any descendant carrying its own
/// [`PrefabInstance`] whose source path is not in `own_source_paths` -- that
/// entity isn't part of this resync and isn't ours to destroy just because it
/// happened to get reparented here (this codebase's `SetParent`/MCP
/// `set_parent` has no guard preventing exactly that). `own_source_paths` is
/// the resync's own source path plus every nested prefab reference the file
/// itself authors (see [`nested_prefab_source_paths`]) -- without that
/// second part, a legitimate nested `prefab:` reference (which produces the
/// exact same `Parent` + differently-sourced-`PrefabInstance` shape as a
/// reparented stranger) would be wrongly protected too, breaking the
/// intentional cascade a resync of the *outer* file is supposed to have on
/// its own nested instances.
///
/// A protected descendant's own ancestor chain up to (but not including) it
/// still gets despawned as normal; the foreign instance survives as an
/// entity, now with a stale `Parent` pointing at a despawned entity -- an
/// already-tolerated state elsewhere in this codebase (plain
/// `EditorCommand::Despawn` doesn't cascade to children either) -- and a
/// warning is logged so this is never silent.
fn despawn_subtree(
    world: &mut World,
    root: Entity,
    own_source_paths: &std::collections::HashSet<String>,
) {
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
                if let Some(instance) = world.get::<PrefabInstance>(child) {
                    if !own_source_paths.contains(&instance.source_path) {
                        tracing::warn!(
                            "prefab live-sync: entity carrying its own PrefabInstance \
                             (source '{}') was found reparented underneath a subtree being \
                             resynced; leaving it and its subtree untouched rather than \
                             despawning it as collateral damage -- it is now parented under \
                             a despawned entity and will need to be re-parented manually",
                            instance.source_path
                        );
                        continue;
                    }
                }
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
/// A missing, unparseable, or structurally invalid (wrong root count, a
/// duplicate entity name) file leaves every matching instance untouched
/// rather than despawning anything -- the existence/parse/structural-validity
/// check happens before any entity is touched, precisely so a bad edit can't
/// destroy a working instance on the way to failing.
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

    let own_source_paths =
        nested_prefab_source_paths(changed_source_path, &prefab, project_dir.as_ref());

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

        despawn_subtree(world, root, &own_source_paths);

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

/// Same debounce window as `bsengine-asset`'s `AssetWatcherPlugin`, for the
/// same reason: a save is rarely one write, and 200ms is long enough to
/// collapse a burst of them into one change without making an edit feel
/// slow to take effect.
const DEBOUNCE: Duration = Duration::from_millis(200);

/// Editor-only convenience: watches `<ProjectDir>/assets/prefabs` and
/// despawn-and-reinstantiates every live [`PrefabInstance`] whose source file
/// changes on disk, so an edit made to a prefab file while the editor is
/// running is reflected in every placed instance without a manual reload.
///
/// Not added to the headless runtime/game app -- this is a live-editing
/// feature, not gameplay behavior. Requires a `ProjectDir` resource inserted
/// before `Startup` runs; with no `ProjectDir`, an empty one, or a missing
/// `assets/prefabs` directory, the plugin logs once at info and starts no
/// watcher.
pub struct PrefabWatcherPlugin;

impl Plugin for PrefabWatcherPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, start_prefab_watcher)
            .add_systems(Update, drain_prefab_changes)
            .add_systems(Update, resync_changed_prefabs.after(drain_prefab_changes));
    }
}

/// Live watch state. Absent when watching is disabled -- see
/// [`PrefabWatcherPlugin`].
#[derive(Resource)]
struct PrefabWatcher {
    /// Held purely for its `Drop`, which stops the watch. See
    /// `bsengine_asset::watcher::AssetWatcher` for why this is a `Mutex`
    /// used only as a `Send + Sync` shim, never actually locked for
    /// synchronisation.
    _debouncer: Mutex<Debouncer<RecommendedWatcher, FileIdMap>>,
    /// Receiving end of the watcher thread's channel. Only ever `try_recv`'d.
    events: Mutex<Receiver<DebounceEventResult>>,
    /// `current_dir().join(<ProjectDir>)` -- the prefix every path `notify`
    /// reports carries. Stripping it off a reported path and keeping the
    /// remainder (which already reads e.g. `assets/prefabs/turret.ron`, since
    /// the watch root is `<ProjectDir>/assets/prefabs`) recovers exactly the
    /// project-relative spelling `PrefabInstance::source_path` stores.
    strip_base: PathBuf,
}

/// Paths this resync system has been told to look at since the last time it
/// ran, deduped by path. Populated by [`drain_prefab_changes`] (a regular
/// system, so it can read `PrefabWatcher` via `Res`), consumed by
/// [`resync_changed_prefabs`] (an exclusive system, needed because
/// [`resync_prefab_instances`] mutates the `World` arbitrarily) -- the same
/// split this crate already uses for `PrefabCommandQueueResource` /
/// `process_prefab_commands`.
#[derive(Resource, Default)]
struct PendingPrefabResync(Vec<String>);

/// Starts the watcher, or explains once why it is not starting. Mirrors
/// `bsengine_asset::watcher::start_asset_watcher` closely -- see that
/// function's doc comments for the reasoning behind each guard.
fn start_prefab_watcher(
    mut commands: Commands,
    project_dir: Option<Res<bsengine_core::ProjectDir>>,
) {
    let Some(project_dir) = project_dir.map(|p| p.0.clone()).filter(|p| !p.is_empty()) else {
        info!("prefab live-sync: no project directory set, not watching");
        return;
    };

    let watch_root = PathBuf::from(format!("{project_dir}/assets/prefabs"));
    if !watch_root.is_dir() {
        info!(
            "prefab live-sync: {} does not exist, not watching",
            watch_root.display()
        );
        return;
    }

    // Deliberately NOT canonicalize() -- see AssetWatcherPlugin's identical
    // comment: notify reports the CWD-absolutised spelling, never the
    // canonical one.
    let strip_base = std::env::current_dir()
        .unwrap_or_default()
        .join(&project_dir);

    let (tx, rx) = mpsc::channel();
    let mut debouncer = match new_debouncer(DEBOUNCE, None, tx) {
        Ok(d) => d,
        Err(e) => {
            warn!("prefab live-sync: cannot start the file watcher ({e}), not watching");
            return;
        }
    };
    if let Err(e) = debouncer
        .watcher()
        .watch(&watch_root, RecursiveMode::Recursive)
    {
        warn!(
            "prefab live-sync: cannot watch {} ({e}), not watching",
            watch_root.display()
        );
        return;
    }

    info!("prefab live-sync: watching {}", watch_root.display());
    commands.insert_resource(PrefabWatcher {
        _debouncer: Mutex::new(debouncer),
        events: Mutex::new(rx),
        strip_base,
    });
}

/// Rebuilds the project-relative spelling `PrefabInstance::source_path`
/// stores (e.g. `"assets/prefabs/turret.ron"`) from the absolutised path
/// `notify` reported. Returns `None` for anything not a `.ron` file, or a
/// path that can't be expressed relative to `strip_base`.
fn reconstruct_source_path(changed: &Path, strip_base: &Path) -> Option<String> {
    let extension = changed.extension()?.to_str()?.to_ascii_lowercase();
    if extension != "ron" {
        return None;
    }
    let relative = changed.strip_prefix(strip_base).ok()?;
    let relative = relative.to_str()?.replace('\\', "/");
    if relative.is_empty() {
        return None;
    }
    Some(relative)
}

/// Drains everything the watcher thread has posted, reconstructs each
/// changed path's project-relative spelling, dedupes, and queues the result
/// in [`PendingPrefabResync`] for [`resync_changed_prefabs`] to act on.
/// Never waits -- see `bsengine_asset::watcher::drain_asset_changes` for the
/// identical poisoned-lock / disconnected-channel handling this mirrors.
fn drain_prefab_changes(mut commands: Commands, watcher: Option<Res<PrefabWatcher>>) {
    let Some(watcher) = watcher else {
        return;
    };

    let mut changed: Vec<String> = Vec::new();
    {
        let events = match watcher.events.lock() {
            Ok(events) => events,
            Err(_) => {
                warn!(
                    "prefab live-sync: the change queue was poisoned by an earlier panic; \
                     live-sync has stopped until the app is restarted"
                );
                commands.remove_resource::<PrefabWatcher>();
                return;
            }
        };
        loop {
            match events.try_recv() {
                Ok(Ok(batch)) => {
                    for event in batch.iter() {
                        for path in event.event.paths.iter() {
                            let Some(source_path) =
                                reconstruct_source_path(path, &watcher.strip_base)
                            else {
                                continue;
                            };
                            if !changed.iter().any(|c| c == &source_path) {
                                changed.push(source_path);
                            }
                        }
                    }
                }
                Ok(Err(errors)) => warn!("prefab live-sync: watcher error: {errors:?}"),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    warn!(
                        "prefab live-sync: the file watcher thread has stopped; live-sync \
                         has stopped until the app is restarted"
                    );
                    commands.remove_resource::<PrefabWatcher>();
                    break;
                }
            }
        }
    }

    if !changed.is_empty() {
        commands.insert_resource(PendingPrefabResync(changed));
    }
}

/// Consumes [`PendingPrefabResync`] (if any) and resyncs every path in it.
/// Exclusive because [`resync_prefab_instances`] despawns and re-instantiates
/// entities arbitrarily, which needs `&mut World` directly.
fn resync_changed_prefabs(world: &mut World) {
    let Some(mut pending) = world.get_resource_mut::<PendingPrefabResync>() else {
        return;
    };
    let paths = std::mem::take(&mut pending.0);
    world.remove_resource::<PendingPrefabResync>();
    for path in paths {
        resync_prefab_instances(world, &path);
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

    #[test]
    fn resync_of_an_outer_file_does_not_leak_a_zombie_grand_nested_instance() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        // A 3-level nesting chain: outer.ron -> middle.ron -> inner.ron. The
        // instantiation machinery (instantiate_prefab_from_path ->
        // resolve_prefab_reference -> recursion) supports arbitrary-depth
        // nesting -- see
        // bsengine_scene::plugin::tests::nested_prefab_reference_instantiates_recursively
        // -- so live-sync's own protection against collateral despawn has to
        // recognize the *whole* chain as part of outer's own composition,
        // not just its direct child, or a grand-nested instance (inner.ron
        // here) gets wrongly protected from outer's despawn, orphaned under
        // a despawned entity, while outer's re-instantiation creates a
        // second, fresh one -- leaking a zombie.
        write_prefab(
            dir.path(),
            "inner",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "InnerRoot", primitive: Some(Cube)),
            ])"#,
        );
        write_prefab(
            dir.path(),
            "middle",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "MiddleRoot", primitive: Some(Cube)),
                EntityDescriptor(name: "InnerRef", parent: Some("MiddleRoot"), prefab: Some("assets/prefabs/inner.ron")),
            ])"#,
        );
        let outer_path = write_prefab(
            dir.path(),
            "outer",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Outer", primitive: Some(Cube)),
                EntityDescriptor(name: "MiddleRef", parent: Some("Outer"), prefab: Some("assets/prefabs/middle.ron")),
            ])"#,
        );

        let resolved_outer = bsengine_core::resolve_project_path(Some(&project_dir), &outer_path);
        bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved_outer,
            Some("OuterInstance"),
            None,
            None,
        )
        .unwrap();

        write_prefab(
            dir.path(),
            "outer",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Outer", primitive: Some(Cube)),
                EntityDescriptor(name: "MiddleRef", parent: Some("Outer"), prefab: Some("assets/prefabs/middle.ron")),
                EntityDescriptor(name: "ExtraChild", parent: Some("Outer"), primitive: Some(Cube)),
            ])"#,
        );
        resync_prefab_instances(app.world_mut(), &outer_path);

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

        let inner_instance_count = {
            let mut q = app.world_mut().query::<&PrefabInstance>();
            q.iter(app.world())
                .filter(|i| i.source_path == "assets/prefabs/inner.ron")
                .count()
        };
        assert_eq!(
            inner_instance_count, 1,
            "resyncing the outer file must not leave a zombie grand-nested inner.ron \
             instance behind alongside the freshly re-resolved one: {names:?}"
        );
    }

    #[test]
    fn resync_does_not_destroy_an_unrelated_prefab_instance_reparented_underneath_it() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path_a = write_prefab(dir.path(), "turret", TURRET_V1);
        let source_path_b = write_prefab(
            dir.path(),
            "widget",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "WidgetRoot", primitive: Some(Cube)),
            ])"#,
        );

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let resolved_a = bsengine_core::resolve_project_path(Some(&project_dir), &source_path_a);
        let instance_a_root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved_a,
            Some("InstanceA"),
            None,
            None,
        )
        .unwrap();

        let resolved_b = bsengine_core::resolve_project_path(Some(&project_dir), &source_path_b);
        let instance_b_root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved_b,
            Some("InstanceB"),
            None,
            None,
        )
        .unwrap();

        // Simulate reparenting InstanceB underneath InstanceA the way the
        // Hierarchy panel / EditorCommand::SetParent would (neither guards
        // against this today) -- a direct Parent insert is the simplest way
        // to construct this state in a unit test.
        app.world_mut()
            .entity_mut(instance_b_root)
            .insert(Parent(instance_a_root));

        write_prefab(dir.path(), "turret", TURRET_V2);
        resync_prefab_instances(app.world_mut(), &source_path_a);

        assert!(
            app.world().get_entity(instance_b_root).is_some(),
            "an unrelated PrefabInstance reparented underneath the instance being resynced \
             must survive, not be despawned as collateral damage"
        );
        assert_eq!(
            app.world()
                .get::<PrefabInstance>(instance_b_root)
                .map(|i| i.source_path.clone()),
            Some(source_path_b.clone()),
            "the surviving unrelated instance's PrefabInstance must be unchanged"
        );

        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(
            names.iter().any(|n| n.starts_with("Scope#")),
            "InstanceA must still have been resynced despite InstanceB being found \
             underneath it: {names:?}"
        );
    }

    #[test]
    fn resync_warns_rather_than_panics_when_a_matching_root_is_a_live_descendant_of_another() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());
        let source_path = write_prefab(dir.path(), "turret", TURRET_V1);

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);

        let resolved = bsengine_core::resolve_project_path(Some(&project_dir), &source_path);
        let instance_a_root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("InstanceA"),
            None,
            None,
        )
        .unwrap();
        let instance_b_root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("InstanceB"),
            None,
            None,
        )
        .unwrap();

        // Two instances of the *same* source path, with B a live descendant
        // of A -- exercises the `world.get_entity(root).is_none()` branch at
        // the top of the `for root in roots` loop (B is swept up as a normal
        // part of A's real despawn_subtree, since both share
        // `protected_source_path`, so by the time the loop reaches B's own
        // entry in `roots` it is already gone).
        app.world_mut()
            .entity_mut(instance_b_root)
            .insert(Parent(instance_a_root));

        write_prefab(dir.path(), "turret", TURRET_V2);
        resync_prefab_instances(app.world_mut(), &source_path);

        // Must not panic (implicit -- reaching this point proves it).
        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(
            names.contains(&"InstanceA".to_string()),
            "InstanceA must have been resynced successfully: {names:?}"
        );
        assert!(
            !names.contains(&"InstanceB".to_string()),
            "InstanceB was a live descendant of InstanceA and got swept up in A's real \
             despawn (same source path, so not protected); it must not reappear: {names:?}"
        );

        let instance_count = {
            let mut q = app.world_mut().query::<&PrefabInstance>();
            q.iter(app.world())
                .filter(|i| i.source_path == source_path)
                .count()
        };
        assert_eq!(
            instance_count, 1,
            "exactly one live instance of the shared source path should remain: {names:?}"
        );
    }

    #[test]
    fn resync_updates_a_prefab_used_both_standalone_and_nested_in_another_instance() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());

        let mut app = new_app();
        app.insert_resource(project_dir.clone());
        register(&mut app);
        let (_outer_path, nested_path) = setup_nested_fixture(&mut app, &project_dir);

        // A second, independent top-level instance of the *same* nested.ron
        // -- not reached through outer.ron's `prefab:` reference at all.
        let resolved_nested = bsengine_core::resolve_project_path(Some(&project_dir), &nested_path);
        let standalone_root = bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved_nested,
            Some("StandaloneNested"),
            None,
            None,
        )
        .unwrap();

        let sibling_before = {
            let mut q = app.world_mut().query::<(Entity, &Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Sibling#"))
                .map(|(e, _)| e)
                .expect("Sibling should have spawned as part of the outer instance")
        };

        write_prefab(dir.path(), "nested", NESTED_V2);
        resync_prefab_instances(app.world_mut(), &nested_path);

        assert!(
            app.world().get_entity(sibling_before).is_some(),
            "editing the shared nested.ron must leave the outer instance's unrelated \
             sibling entity untouched"
        );
        assert!(
            app.world().get_entity(standalone_root).is_none(),
            "the standalone top-level nested instance must have been despawned and \
             re-instantiated fresh, not left as the old entity"
        );

        let names: Vec<String> = app
            .world_mut()
            .query::<&Name>()
            .iter(app.world())
            .map(|n| n.0.clone())
            .collect();
        assert!(
            names.iter().any(|n| n == "StandaloneNested"),
            "the resynced standalone instance must still exist under the same name: {names:?}"
        );
        let new_child_count = names
            .iter()
            .filter(|n| n.starts_with("NestedChild#"))
            .count();
        assert_eq!(
            new_child_count, 2,
            "both the nested-inside-outer instance and the standalone instance must pick up \
             nested.ron's new content: {names:?}"
        );

        let instance_count = {
            let mut q = app.world_mut().query::<&PrefabInstance>();
            q.iter(app.world())
                .filter(|i| i.source_path == nested_path)
                .count()
        };
        assert_eq!(
            instance_count, 2,
            "exactly two live instances of nested.ron should remain: one nested inside \
             outer, one standalone: {names:?}"
        );
    }

    #[test]
    fn nested_prefab_source_paths_terminates_on_a_cyclic_nested_reference() {
        let dir = tempfile::tempdir().unwrap();
        let project_dir = ProjectDir(dir.path().to_string_lossy().to_string());

        // a.ron nests b.ron; b.ron nests a.ron right back -- a direct cycle.
        // instantiate_prefab_from_path's own RESOLVING_PREFABS guard refuses
        // to actually instantiate this (see
        // bsengine_scene::plugin::tests::cyclic_prefab_reference_fails_loudly_instead_of_recursing_forever),
        // but nested_prefab_source_paths only reads and parses files -- it
        // never instantiates anything -- so it needs its own, independent
        // cycle guard. This proves it terminates rather than hanging or
        // overflowing the stack, and that it doesn't drop either side of
        // the cycle from the returned set.
        let a_path = write_prefab(
            dir.path(),
            "a",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "ARoot", primitive: Some(Cube)),
                EntityDescriptor(name: "BRef", parent: Some("ARoot"), prefab: Some("assets/prefabs/b.ron")),
            ])"#,
        );
        write_prefab(
            dir.path(),
            "b",
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "BRoot", primitive: Some(Cube)),
                EntityDescriptor(name: "ARef", parent: Some("BRoot"), prefab: Some("assets/prefabs/a.ron")),
            ])"#,
        );

        let resolved_a = bsengine_core::resolve_project_path(Some(&project_dir), &a_path);
        let content = std::fs::read_to_string(&resolved_a).unwrap();
        let a_descriptor: bsengine_scene::types::PrefabDescriptor = ron::from_str(&content).unwrap();

        let paths = nested_prefab_source_paths(&a_path, &a_descriptor, Some(&project_dir));

        assert_eq!(
            paths,
            std::collections::HashSet::from([a_path.clone(), "assets/prefabs/b.ron".to_string()]),
            "a direct cycle must terminate with exactly the two files actually involved, \
             not hang, overflow the stack, or silently drop either side of the cycle"
        );
    }

    /// Runs frames until `done`, or panics with `what` after 20 seconds.
    /// Bounded by wall clock, not a frame count, since what's being waited on
    /// is a filesystem notification on another thread -- mirrors
    /// `bsengine_asset::watcher`'s tests' identical `run_until` helper.
    fn run_until(app: &mut App, what: &str, mut done: impl FnMut(&mut App) -> bool) {
        let deadline = std::time::Instant::now() + Duration::from_secs(20);
        while std::time::Instant::now() < deadline {
            app.update();
            if done(app) {
                return;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("{what} did not happen within 20s");
    }

    #[test]
    fn prefab_watcher_plugin_resyncs_a_placed_instance_when_its_source_file_changes() {
        // A relative ProjectDir, under the crate's CWD -- the shape the
        // engine actually uses, and the one that actually exercises
        // reconstruct_source_path's stripping logic (an absolute ProjectDir
        // would make the reported path already equal the engine form,
        // leaving the reconstruction untested -- same rationale as
        // AssetWatcherPlugin's own end-to-end test).
        let project = format!(
            "bsengine-prefab-watch-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .subsec_nanos()
        );
        let root = PathBuf::from(&project);
        let prefabs_dir = root.join("assets").join("prefabs");
        std::fs::create_dir_all(&prefabs_dir).unwrap();
        struct Cleanup(PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }
        let _cleanup = Cleanup(root.clone());

        std::fs::write(prefabs_dir.join("turret.ron"), TURRET_V1).unwrap();

        let mut app = new_app();
        app.insert_resource(ProjectDir(project.clone()));
        register(&mut app);
        app.add_plugins(PrefabWatcherPlugin);

        let resolved = format!("{project}/assets/prefabs/turret.ron");
        bsengine_scene::instantiate_prefab_from_path(
            app.world_mut(),
            &resolved,
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        run_until(&mut app, "the watcher to start", |app| {
            app.world().get_resource::<PrefabWatcher>().is_some()
        });

        // Let the OS backend settle before writing, then discard whatever
        // the initial load/watch start stirred up -- mirrors
        // AssetWatcherPlugin's own tests' identical settle-then-discard step.
        std::thread::sleep(DEBOUNCE * 3);
        for _ in 0..5 {
            app.update();
        }

        std::fs::write(prefabs_dir.join("turret.ron"), TURRET_V2).unwrap();

        run_until(
            &mut app,
            "the placed instance to pick up Scope from the edited file",
            |app| {
                let mut q = app.world_mut().query::<&Name>();
                q.iter(app.world()).any(|n| n.0.starts_with("Scope#"))
            },
        );

        // Not `(Entity, &Name, &Transform)`: neither TURRET_V1 nor TURRET_V2
        // gives any entity an explicit `transform:` field, and the initial
        // instantiate call above passes no transform_override either, so the
        // root never carries a Transform component before or after resync --
        // there is nothing here to preserve. Transform preservation across a
        // resync is already covered by
        // `resync_preserves_the_instances_name_transform_and_parent`, whose
        // fixture does give the root an explicit transform_override; this
        // test's job is only to prove the real plugin -- watcher thread,
        // debounce, drain, exclusive resync system -- actually drives
        // resync_prefab_instances end to end.
        let mut q = app.world_mut().query::<(Entity, &Name)>();
        let resynced = q
            .iter(app.world())
            .find(|(_, n)| n.0 == "MyTurret")
            .expect("the instance must still be named MyTurret after resyncing");
        assert!(
            app.world().get::<PrefabInstance>(resynced.0).is_some(),
            "the resynced root must still carry PrefabInstance"
        );
    }
}
