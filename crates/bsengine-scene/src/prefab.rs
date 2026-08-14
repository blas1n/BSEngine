//! Single-prefab instantiation: turns a [`PrefabDescriptor`](crate::types::PrefabDescriptor)
//! into real spawned entities by delegating entirely to
//! [`spawn_scene_entities`](crate::plugin::spawn_scene_entities) -- no component-attaching
//! logic is duplicated here.
//!
//! This module handles a single, non-nested prefab. Wiring scene files and nested prefabs
//! to call [`instantiate_prefab`] automatically is a later task.

use crate::plugin::{spawn_scene_entities, Name};
use crate::types::{EntityDescriptor, PrefabDescriptor, TransformDescriptor};
use bevy_ecs::prelude::{Entity, World};
use std::sync::atomic::{AtomicU64, Ordering};

static PREFAB_INSTANCE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns a process-wide, strictly-increasing suffix. Every call is
/// guaranteed distinct from every other call, regardless of which of the
/// three instantiation paths (scene file, runtime script/MCP, editor) draws
/// it -- an atomic, not a thread-local, specifically so uniqueness holds
/// even if two instantiations happen to run on different worker threads.
pub fn next_instance_suffix() -> u64 {
    PREFAB_INSTANCE_COUNTER.fetch_add(1, Ordering::Relaxed)
}

/// Instantiates a prefab's entity subtree into `world`.
///
/// `root_name`: `None` draws a fresh unique suffix and applies it to every
/// entity's name (`"{original}#{suffix}"`), including the root. `Some(n)`
/// uses `n` verbatim as the root's name (no suffix) -- the caller is
/// responsible for `n`'s uniqueness, exactly like any hand-authored scene
/// entity name. Non-root entities in the prefab always get the shared
/// suffix regardless of `root_name`, since they only need to be unique
/// *within this one instantiation* (to keep `parent:` resolution correct),
/// not globally.
///
/// `root_transform`: `Some(t)` overrides the prefab's own authored root
/// transform (this is how a scene file's instantiation point places the
/// instance); `None` keeps whatever the prefab file itself authored.
///
/// `parent`: if given, the instantiated root is parented under this
/// existing entity via a real `Parent` component, in addition to (not
/// instead of) whatever internal parent chain the prefab's own entities
/// have among themselves.
///
/// Returns the spawned root `Entity`, or an error if the prefab doesn't
/// have exactly one root entity (zero or multiple entities with no
/// `parent:`).
pub fn instantiate_prefab(
    world: &mut World,
    prefab: &PrefabDescriptor,
    root_name: Option<&str>,
    root_transform: Option<TransformDescriptor>,
    parent: Option<Entity>,
) -> Result<Entity, String> {
    let roots: Vec<&EntityDescriptor> = prefab
        .entities
        .iter()
        .filter(|e| e.parent.is_none())
        .collect();

    // Every entity name in the prefab must be unique, checked before any
    // rewriting happens. Without this, the rewrite step below determines
    // "is this the root?" by string equality against `original_root_name`
    // (needed anyway, so a parent: reference that names the root rewrites
    // to the same final name the root itself gets) -- so a *non-root*
    // entity that happens to share the root's exact name (e.g. a
    // copy-pasted entity block where `name:` was never changed) would
    // silently be treated as the root too: both get renamed to
    // `final_root_name`, both would receive `root_transform` if given, and
    // whichever of the two `spawn_scene_entities` or our own post-spawn
    // lookup happens to land on second is anyone's guess. Rejecting
    // duplicate names outright removes the ambiguity at the source instead
    // of trying to disambiguate it after the fact.
    let mut seen_names = std::collections::HashSet::new();
    for e in &prefab.entities {
        if !seen_names.insert(e.name.as_str()) {
            return Err(format!("prefab has a duplicate entity name: '{}'", e.name));
        }
    }

    if roots.is_empty() {
        return Err("prefab has no root entity (every entity names a parent)".to_string());
    }
    if roots.len() > 1 {
        let names: Vec<&str> = roots.iter().map(|e| e.name.as_str()).collect();
        return Err(format!(
            "prefab has {} root entities, expected exactly 1: {}",
            roots.len(),
            names.join(", ")
        ));
    }
    let original_root_name = roots[0].name.clone();

    let suffix = next_instance_suffix();
    let final_root_name = root_name
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("{original_root_name}#{suffix}"));

    // Rewrite every entity's name -- and every parent: reference, which
    // names another entity *in this same original list* -- through one
    // shared mapping, so the prefab's internal parent chain keeps
    // resolving correctly against the renamed entities. This has to be a
    // single function used for both roles: a parent: reference that names
    // the original root must map to `final_root_name`, exactly like the
    // root entity's own name does -- mapping it through the generic
    // suffix instead (as a naive first version of this might) would
    // silently orphan every direct child of the root, since nothing in
    // the rewritten list would carry the plain suffixed root name any more.
    let rewrite_name = |name: &str| -> String {
        if name == original_root_name {
            final_root_name.clone()
        } else {
            format!("{name}#{suffix}")
        }
    };
    let rewritten: Vec<EntityDescriptor> = prefab
        .entities
        .iter()
        .map(|e| {
            let mut e = e.clone();
            let is_root = e.name == original_root_name;
            e.name = rewrite_name(&e.name);
            e.parent = e.parent.as_ref().map(|p| rewrite_name(p));
            if is_root {
                if let Some(t) = root_transform.clone() {
                    e.transform = Some(t);
                }
            }
            e
        })
        .collect();

    spawn_scene_entities(world, &rewritten);

    let root_entity = {
        let mut q = world.query::<(Entity, &Name)>();
        q.iter(world)
            .find(|(_, n)| n.0 == final_root_name)
            .map(|(e, _)| e)
    };
    let Some(root_entity) = root_entity else {
        // spawn_scene_entities always spawns every entity it's given
        // (barring a bug elsewhere) -- this should be unreachable, but
        // fail loudly rather than panic if it ever isn't.
        return Err(format!(
            "internal error: prefab root '{final_root_name}' was not found after spawning"
        ));
    };

    if let Some(parent_entity) = parent {
        world
            .entity_mut(root_entity)
            .insert(bsengine_core::Parent(parent_entity));
    }

    Ok(root_entity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::PrefabDescriptor;
    use bsengine_app::new_app;

    fn two_entity_prefab() -> PrefabDescriptor {
        ron::from_str(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Wheel", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        )
        .unwrap()
    }

    #[test]
    fn instantiate_prefab_spawns_root_and_child_with_a_shared_unique_suffix() {
        let mut app = new_app();
        let prefab = two_entity_prefab();

        let root = instantiate_prefab(app.world_mut(), &prefab, None, None, None)
            .expect("a well-formed 2-entity prefab should instantiate");

        let mut q = app
            .world_mut()
            .query::<(&crate::plugin::Name, Option<&bsengine_core::Parent>)>();
        let rows: Vec<(String, Option<Entity>)> = q
            .iter(app.world())
            .map(|(n, p)| (n.0.clone(), p.map(|p| p.0)))
            .collect();

        // Both spawned names must share the exact same suffix (proves one
        // shared id was drawn for the whole instance, not one per entity).
        let body_row = rows
            .iter()
            .find(|(n, _)| n.starts_with("Body#"))
            .expect("Body should have spawned with a numeric suffix");
        let wheel_row = rows
            .iter()
            .find(|(n, _)| n.starts_with("Wheel#"))
            .expect("Wheel should have spawned with a numeric suffix");
        let body_suffix = body_row.0.strip_prefix("Body#").unwrap();
        let wheel_suffix = wheel_row.0.strip_prefix("Wheel#").unwrap();
        assert_eq!(
            body_suffix, wheel_suffix,
            "root and child must share one instance suffix"
        );

        let mut root_name_q = app.world_mut().query::<&crate::plugin::Name>();
        assert_eq!(
            root_name_q.get(app.world(), root).unwrap().0,
            body_row.0,
            "instantiate_prefab must return the actual spawned root entity"
        );
        assert_eq!(
            wheel_row.1,
            Some(root),
            "Wheel's Parent must point at the spawned Body root, by its real Entity id"
        );
    }

    #[test]
    fn instantiate_prefab_honors_an_explicit_root_name_with_no_suffix() {
        let mut app = new_app();
        let prefab = two_entity_prefab();

        let root = instantiate_prefab(app.world_mut(), &prefab, Some("Boss"), None, None)
            .expect("instantiation should succeed");

        let mut q = app.world_mut().query::<&crate::plugin::Name>();
        assert_eq!(
            q.get(app.world(), root).unwrap().0,
            "Boss",
            "an explicit root_name override must be used verbatim, no suffix added \
             (the caller is responsible for its own uniqueness, exactly like any \
             other hand-authored scene entity name)"
        );
    }

    #[test]
    fn instantiate_prefab_parents_the_root_under_a_given_entity() {
        let mut app = new_app();
        let anchor = app
            .world_mut()
            .spawn(crate::plugin::Name("Anchor".to_string()))
            .id();
        let prefab = two_entity_prefab();

        let root = instantiate_prefab(app.world_mut(), &prefab, None, None, Some(anchor))
            .expect("instantiation should succeed");

        let parent = app.world().get::<bsengine_core::Parent>(root);
        assert_eq!(
            parent.map(|p| p.0),
            Some(anchor),
            "when a parent Entity is given, the instantiated root must be parented under it"
        );
    }

    #[test]
    fn instantiate_prefab_rejects_zero_roots() {
        let mut app = new_app();
        let bad: PrefabDescriptor = ron::from_str(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "A", parent: Some("B")),
                EntityDescriptor(name: "B", parent: Some("A")),
            ])"#,
        )
        .unwrap();

        let result = instantiate_prefab(app.world_mut(), &bad, None, None, None);
        assert!(
            result.is_err(),
            "a prefab with no root entity must fail to instantiate"
        );
    }

    #[test]
    fn instantiate_prefab_rejects_multiple_roots() {
        let mut app = new_app();
        let bad: PrefabDescriptor = ron::from_str(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "RootA"),
                EntityDescriptor(name: "RootB"),
            ])"#,
        )
        .unwrap();

        let result = instantiate_prefab(app.world_mut(), &bad, None, None, None);
        assert!(
            result.is_err(),
            "a prefab with two root entities must fail to instantiate"
        );
    }

    #[test]
    fn instantiate_prefab_rejects_a_duplicate_entity_name() {
        let mut app = new_app();
        // Root-count validation alone lets this through (only one entity has
        // no `parent:`), but the second "Body" -- a non-root entity that
        // happens to share the root's exact name, e.g. a copy-pasted block
        // where `name:` was never updated -- must still be caught, since
        // nothing downstream can tell the two apart once names collide.
        let bad: PrefabDescriptor = ron::from_str(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Body", parent: Some("Wheel"), primitive: Some(Cube)),
            ])"#,
        )
        .unwrap();

        let result = instantiate_prefab(app.world_mut(), &bad, None, None, None);
        assert!(
            result.is_err(),
            "a prefab with two entities sharing the same name must fail to instantiate, \
             not silently produce two entities with an identical Name component"
        );
    }

    #[test]
    fn instantiate_prefab_overrides_only_the_roots_transform() {
        let mut app = new_app();
        let prefab: PrefabDescriptor = ron::from_str(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube), transform: Some((position: (1.0, 2.0, 3.0)))),
                EntityDescriptor(name: "Wheel", parent: Some("Body"), primitive: Some(Cube), transform: Some((position: (0.5, 0.0, 0.0)))),
            ])"#,
        )
        .unwrap();

        let override_transform = TransformDescriptor {
            position: [5.0, 0.0, 0.0],
            ..Default::default()
        };

        let root = instantiate_prefab(
            app.world_mut(),
            &prefab,
            None,
            Some(override_transform),
            None,
        )
        .expect("instantiation should succeed");

        let root_position = app
            .world()
            .get::<bsengine_core::Transform>(root)
            .expect("root should have a Transform")
            .position
            .0;
        assert_eq!(
            root_position,
            glam::Vec3::new(5.0, 0.0, 0.0),
            "root_transform must override the root's own authored transform"
        );

        let mut q = app
            .world_mut()
            .query::<(&crate::plugin::Name, &bsengine_core::Transform)>();
        let wheel_position = q
            .iter(app.world())
            .find(|(n, _)| n.0.starts_with("Wheel#"))
            .map(|(_, t)| t.position.0)
            .expect("Wheel should have spawned with its own Transform");
        assert_eq!(
            wheel_position,
            glam::Vec3::new(0.5, 0.0, 0.0),
            "root_transform override must be scoped to the root only; the child keeps its \
             own prefab-authored local transform"
        );
    }
}
