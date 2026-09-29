//! Applies queued `ReflectCommand`s: reflected component edits by type path.

use super::*;

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
pub(super) fn fixup_entity_fields(value: &mut dyn bevy_reflect::Reflect, world: &World) {
    if let Some(entity) = value.downcast_mut::<bevy_ecs::prelude::Entity>() {
        let wanted_index = entity.index();
        if let Some(live) = world
            .iter_entities()
            .find(|e| e.id().index() == wanted_index)
        {
            *entity = live.id();
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
            // each entry (`get_at_mut`) -- keys are immutable through this
            // trait, since mutating one in place would desync the map's
            // internal hash bucket for it. That's fine here: recursing into
            // the value covers the realistic shape (e.g. `HashMap<K,
            // Entity>`); a key itself being an `Entity` is an unusual design
            // this codebase doesn't use.
            for i in 0..m.len() {
                if let Some((_key, value)) = m.get_at_mut(i) {
                    fixup_entity_fields(value, world);
                }
            }
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
                let target = world
                    .iter_entities()
                    .find(|e| e.id().index() as u64 == entity_id);
                if let Some(entity) = target.map(|e| e.id()) {
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
                let target = world
                    .iter_entities()
                    .find(|e| e.id().index() as u64 == entity_id);
                if let Some(entity) = target.map(|e| e.id()) {
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
                let target = world
                    .iter_entities()
                    .find(|e| e.id().index() as u64 == entity_id);
                if let Some(entity) = target.map(|e| e.id()) {
                    let mut entity_mut = world.entity_mut(entity);
                    reflect_component.apply_or_insert(&mut entity_mut, value.as_ref(), &registry);
                }
            }
        }
    }
}
