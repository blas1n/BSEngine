//! Applies queued `ReflectCommand`s: reflected component edits by type path.

use super::*;

/// The live entity whose index is `index`, if one is alive -- what the editor
/// and MCP mean by an entity id, since they name entities by index alone.
///
/// Was `world.query::<bevy_ecs::world::EntityRef>().iter(world).find(|e| e.id().index() == index)`, a walk over
/// every entity; `iter_entities` is deprecated in Bevy 0.17. `Entities` already
/// knows the current generation of every index, so this is a lookup. Whether the
/// index is *occupied* is a separate question: a freed index's generation has
/// already been bumped, so `resolve_from_index` hands back an entity that
/// `Entities::contains` (a generation compare) calls present. `get_entity`
/// asks whether it has a location, which is what the walk's "it is in the
/// world" meant.
pub(super) fn live_entity(world: &World, index: u64) -> Option<Entity> {
    let index = Entity::from_raw_u32(u32::try_from(index).ok()?)?.index();
    let entity = world.entities().resolve_from_index(index);
    world.get_entity(entity).ok().map(|e| e.id())
}

/// Recursively walks `value`'s reflect tree and, for every leaf field whose
/// declared type is `bevy_ecs::Entity`, replaces it with the live entity
/// that currently has the same `.index()` (if any still does). The
/// Inspector UI (`reflect_ui.rs`) only ever knows a raw `u64` index for an
/// `Entity`-typed field -- it edits a *detached clone*, with no `World`
/// access to look up the entity's real generation -- so it writes
/// `Entity::from_raw(index)` (generation 0) as a placeholder. Applying that
/// as-is could silently target the wrong (or a since-despawned-and-reused)
/// entity slot; this runs once, here, where `World` access is actually
/// available, before the value is applied to a live component.
pub(super) fn fixup_entity_fields(value: &mut dyn bevy_reflect::PartialReflect, world: &World) {
    if let Some(entity) = value.try_downcast_mut::<bevy_ecs::prelude::Entity>() {
        if let Some(live) = live_entity(world, entity.index_u32() as u64) {
            *entity = live;
        }
        return;
    }
    match value.reflect_mut() {
        bevy_reflect::ReflectMut::Struct(s) => {
            for i in 0..s.field_len() {
                if let Some(field) = s.field_at_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::TupleStruct(ts) => {
            for i in 0..ts.field_len() {
                if let Some(field) = ts.field_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::Enum(e) => {
            for i in 0..e.field_len() {
                if let Some(field) = e.field_at_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::List(l) => {
            for i in 0..l.len() {
                if let Some(field) = l.get_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::Array(a) => {
            for i in 0..a.len() {
                if let Some(field) = a.get_mut(i) {
                    fixup_entity_fields(field, world);
                }
            }
        }
        bevy_reflect::ReflectMut::Map(m) => {
            // `Map` only exposes a mutable reference to the *value* half of
            // each entry -- keys are immutable through this trait, since
            // mutating one in place would desync the map's internal hash
            // bucket for it. That's fine here: recursing into the value covers
            // the realistic shape (e.g. `HashMap<K, Entity>`); a key itself
            // being an `Entity` is an unusual design this codebase doesn't use.
            // Since Bevy 0.17 the only mutable walk over the values is
            // `retain` (`get_at_mut` is gone), which keeps every entry here.
            m.retain(&mut |_key, value| {
                fixup_entity_fields(value, world);
                true
            });
        }
        _ => {}
    }
}

pub(super) fn process_reflect_commands(world: &mut World) {
    let cmds: Vec<ReflectCommand> = {
        let Some(queue_res) = world.get_resource::<ReflectCommandQueueResource>() else {
            return;
        };
        let mut queue = queue_res.0.lock().unwrap();
        queue.drain(..).collect()
    };
    if cmds.is_empty() {
        return;
    }

    if let (Some(snapshot_res), Some(history_res)) = (
        world.get_resource::<EditorSnapshotResource>(),
        world.get_resource::<EditorHistoryResource>(),
    ) {
        let checkpoint = snapshot_res.0.lock().unwrap().clone();
        let mut history = history_res.0.lock().unwrap();
        history.undo_stack.push(checkpoint);
        history.redo_stack.clear();
        if history.undo_stack.len() > MAX_UNDO_HISTORY {
            history.undo_stack.remove(0);
        }
    }

    let Some(app_registry) = world
        .get_resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .cloned()
    else {
        return;
    };
    let registry = app_registry.read();

    for cmd in cmds {
        match cmd {
            ReflectCommand::AttachComponentByType {
                entity_id,
                type_path,
            } => {
                let Some(registration) = registry.get_with_type_path(&type_path) else {
                    tracing::warn!("reflect: unknown type path '{type_path}'");
                    continue;
                };
                let Some(reflect_component) =
                    registration.data::<bevy_ecs::reflect::ReflectComponent>()
                else {
                    tracing::warn!("reflect: '{type_path}' is not a registered Component");
                    continue;
                };
                let Some(reflect_default) =
                    registration.data::<bevy_reflect::std_traits::ReflectDefault>()
                else {
                    tracing::warn!("reflect: '{type_path}' has no registered Default");
                    continue;
                };
                let default_value = reflect_default.default();
                let target = live_entity(world, entity_id);
                if let Some(entity) = target {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.insert(&mut entity_mut, default_value.as_ref(), &registry);
                }
            }
            ReflectCommand::RemoveComponentByType {
                entity_id,
                type_path,
            } => {
                let Some(registration) = registry.get_with_type_path(&type_path) else {
                    tracing::warn!("reflect: unknown type path '{type_path}'");
                    continue;
                };
                let Some(reflect_component) =
                    registration.data::<bevy_ecs::reflect::ReflectComponent>()
                else {
                    continue;
                };
                let target = live_entity(world, entity_id);
                if let Some(entity) = target {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.remove(&mut entity_mut);
                }
            }
            ReflectCommand::ApplyComponentValue {
                entity_id,
                type_path,
                mut value,
            } => {
                let Some(registration) = registry.get_with_type_path(&type_path) else {
                    tracing::warn!("reflect: unknown type path '{type_path}'");
                    continue;
                };
                let Some(reflect_component) =
                    registration.data::<bevy_ecs::reflect::ReflectComponent>()
                else {
                    tracing::warn!("reflect: '{type_path}' is not a registered Component");
                    continue;
                };
                fixup_entity_fields(value.as_mut(), world);
                let target = live_entity(world, entity_id);
                if let Some(entity) = target {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.apply_or_insert_mapped(
                        &mut entity_mut,
                        value.as_ref(),
                        &registry,
                        &mut (),
                        bevy_ecs::relationship::RelationshipHookMode::Run,
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_entity_finds_the_current_generation_and_nothing_dead() {
        // The walk it replaced could only ever return a live entity, with
        // whatever generation it had; the lookup has to agree on all four
        // shapes an editor id can arrive in.
        let mut world = World::new();
        let alive = world.spawn_empty().id();
        assert_eq!(live_entity(&world, alive.index_u32() as u64), Some(alive));

        let dead = world.spawn_empty().id();
        world.despawn(dead);
        assert_eq!(
            live_entity(&world, dead.index_u32() as u64),
            None,
            "a freed row is not alive"
        );

        // The index comes back with a new generation; the id an editor holds
        // is the bare index, which now means the new occupant. Made to happen
        // rather than waited for: Bevy 0.19's allocator hands a freed index
        // back whenever it likes (not the next spawn, not after a flush), so
        // the fixture despawns without freeing and spawns the next
        // generation at the same index itself.
        let old = world.spawn_empty().id();
        world.despawn_no_free(old);
        let reused = world
            .spawn_empty_at(Entity::from_index_and_generation(
                old.index(),
                old.generation().after_versions(1),
            ))
            .expect("the index is free for its next generation")
            .id();
        assert_eq!(reused.index(), old.index(), "premise: the same index");
        assert_ne!(reused, old, "premise: under a new generation");
        assert_eq!(live_entity(&world, old.index_u32() as u64), Some(reused));

        assert_eq!(live_entity(&world, 1_000_000), None, "a row never reserved");
        assert_eq!(live_entity(&world, u32::MAX as u64), None);
        assert_eq!(live_entity(&world, u64::MAX), None);
    }
}
