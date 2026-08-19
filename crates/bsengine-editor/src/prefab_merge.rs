//! Field-level merge for prefab push-sync: given a live instance's current
//! state, the prefab content it was last synced against (baseline), and the
//! prefab's current content (new), decides -- per field, per entity -- which
//! value should end up live. See `docs/superpowers/specs/2026-08-19-prefab-override-tracking-design.md`
//! for the full design; this module implements its "diff/patch algorithm"
//! section.
//!
//! The core rule, applied throughout this module: a live value that still
//! equals the baseline adopts the new file's value; a live value that
//! differs from the baseline is an override and is left untouched. The one
//! exception -- whole-entity structural removal always wins regardless of
//! overrides -- lives in `resync_instance` (a later task), not in this
//! file's per-field logic.

use bevy_ecs::prelude::{Entity, World};
use bevy_reflect::TypeRegistry;

/// Snapshots a live entity's current state into an [`EntityDescriptor`],
/// covering this PR's representative field set (`transform`, `primitive`,
/// `emissive`/`color`/`opacity`, and the reflected `components` catalog) plus
/// `name`. Every other `EntityDescriptor` field is left at its `Default`
/// (`None`/`false`/empty) -- deliberately: those fields belong to a follow-up
/// PR's field groups and must never be treated as "live has explicitly
/// cleared this" by the merge logic that reads this snapshot's output.
pub(crate) fn snapshot_entity_as_descriptor(
    world: &World,
    registry: &TypeRegistry,
    entity: Entity,
) -> bsengine_scene::EntityDescriptor {
    let name = world
        .get::<bsengine_scene::Name>(entity)
        .map(|n| n.0.clone())
        .unwrap_or_default();

    let transform = world
        .get::<bsengine_core::Transform>(entity)
        .map(|t| bsengine_scene::TransformDescriptor {
            position: t.position.0.to_array(),
            rotation: t.rotation.0.to_array(),
            scale: t.scale.0.to_array(),
        });

    let primitive = world
        .get::<bsengine_scene::PrimitiveMesh>(entity)
        .map(|p| p.0.clone());

    let material = world.get::<bsengine_core::Material>(entity);
    let emissive = material.map(|m| m.emissive.0.to_array());
    let color = material.map(|m| m.base_color.0.to_array());
    let opacity = material.map(|m| m.opacity);

    let components = snapshot_extra_components(world, registry, entity);

    bsengine_scene::EntityDescriptor {
        name,
        transform,
        primitive,
        emissive,
        color,
        opacity,
        components,
        ..Default::default()
    }
}

/// Type IDs this module's snapshot must never surface as a generic
/// "components:" catalog entry: everything `excluded_from_extra_components`
/// already excludes (it's already handled by a dedicated `EntityDescriptor`
/// field, or holds a raw `Entity` reference), plus the two prefab-tracking
/// components themselves -- neither is meaningful as an attach/detach-able
/// gameplay component, and both are managed entirely by `resync_instance`'s
/// own top-level logic, not by the generic field merge.
fn merge_excluded_component_types() -> std::collections::HashSet<std::any::TypeId> {
    let mut excluded = crate::plugin::excluded_from_extra_components();
    excluded.insert(std::any::TypeId::of::<bsengine_core::PrefabInstance>());
    excluded.insert(std::any::TypeId::of::<bsengine_core::PrefabInstanceBaseline>());
    excluded
}

/// Serializes every registered reflected component on `entity`, other than
/// the ones `merge_excluded_component_types` excludes, into
/// `(type_path, ron)` pairs -- the same technique
/// `populate_snapshot_extra_components` (`bsengine-editor/src/plugin.rs`)
/// already uses for the Inspector's own live-entity snapshot, scoped here to
/// one entity instead of the whole world.
fn snapshot_extra_components(
    world: &World,
    registry: &TypeRegistry,
    entity: Entity,
) -> Vec<(String, String)> {
    let excluded = merge_excluded_component_types();
    let Some(entity_ref) = world.get_entity(entity) else {
        return Vec::new();
    };
    let mut components = Vec::new();
    for registration in registry.iter() {
        if excluded.contains(&registration.type_id()) {
            continue;
        }
        let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
        else {
            continue;
        };
        let Some(value) = reflect_component.reflect(entity_ref) else {
            continue;
        };
        let serializer = bevy_reflect::serde::TypedReflectSerializer::new(value, registry);
        match ron::ser::to_string(&serializer) {
            Ok(ron_str) => {
                components.push((registration.type_info().type_path().to_string(), ron_str));
            }
            Err(e) => tracing::warn!(
                "prefab live-sync: failed to snapshot component '{}' on entity {entity:?}: {e}",
                registration.type_info().type_path()
            ),
        }
    }
    components
}

/// The core rule: a live value that still equals the baseline hasn't been
/// touched since the last sync, so it's safe to adopt the new file's value.
/// A live value that differs from the baseline is an override and wins,
/// regardless of what the new file says.
fn resolve_field<T: Clone + PartialEq>(live: &T, baseline: &T, new: &T) -> T {
    if live == baseline {
        new.clone()
    } else {
        live.clone()
    }
}

/// Merges this PR's representative field set for one matched entity (present
/// in `live`, `baseline`, and `new` alike). `name` always comes from `live`
/// unchanged -- matching is by name already, so there's nothing to resolve
/// there. Every field this PR doesn't yet cover (see the plan's "Scope for
/// this plan" note) is left at `EntityDescriptor::default()`'s value on the
/// returned descriptor; callers must never treat that as "adopt an
/// explicit clear" for those fields -- `apply_merged_descriptor` (a later
/// task) only ever touches the fields this function actually resolves.
pub(crate) fn merge_entity_descriptor(
    live: &bsengine_scene::EntityDescriptor,
    baseline: &bsengine_scene::EntityDescriptor,
    new: &bsengine_scene::EntityDescriptor,
) -> bsengine_scene::EntityDescriptor {
    bsengine_scene::EntityDescriptor {
        name: live.name.clone(),
        transform: resolve_field(&live.transform, &baseline.transform, &new.transform),
        primitive: resolve_field(&live.primitive, &baseline.primitive, &new.primitive),
        emissive: resolve_field(&live.emissive, &baseline.emissive, &new.emissive),
        color: resolve_field(&live.color, &baseline.color, &new.color),
        opacity: resolve_field(&live.opacity, &baseline.opacity, &new.opacity),
        components: merge_components(&live.components, &baseline.components, &new.components),
        ..Default::default()
    }
}

/// Same rule as `resolve_field`, applied per reflected-component type path
/// rather than to one value: a key whose live value still matches its
/// baseline value adopts the new file's value for that key (which may be
/// absent -- the source removed that component -- in which case the merged
/// result omits the key too, so `apply_merged_descriptor` removes it). A key
/// with no baseline entry at all (the user attached a component the prefab
/// never had, or it predates override tracking) is always kept as-is,
/// whatever `new` says. A key present in baseline but missing from `live`
/// (the user detached a prefab-provided component) stays removed -- the
/// removal itself is the override, and is not undone just because `new`
/// still has that key.
fn merge_components(
    live: &[(String, String)],
    baseline: &[(String, String)],
    new: &[(String, String)],
) -> Vec<(String, String)> {
    // A plain `fn` item (rather than a closure) so the elided lifetime ties
    // the returned map's borrows to the single input slice, per fn lifetime
    // elision rules -- a closure with this same signature fails to compile
    // because closures don't get that elision and each occurrence of `&str`
    // is inferred as its own independent lifetime.
    fn to_map(pairs: &[(String, String)]) -> std::collections::HashMap<&str, &str> {
        pairs.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect()
    }
    let live_map = to_map(live);
    let baseline_map = to_map(baseline);
    let new_map = to_map(new);

    let mut all_keys: Vec<&str> = live_map
        .keys()
        .chain(baseline_map.keys())
        .chain(new_map.keys())
        .copied()
        .collect();
    all_keys.sort_unstable();
    all_keys.dedup();

    let mut result = Vec::new();
    for key in all_keys {
        let resolved = match baseline_map.get(key) {
            Some(&b) => match live_map.get(key) {
                Some(&l) if l == b => new_map.get(key).copied(),
                Some(&l) => Some(l),
                None => None,
            },
            None => match live_map.get(key) {
                Some(&l) => Some(l),
                None => new_map.get(key).copied(),
            },
        };
        if let Some(v) = resolved {
            result.push((key.to_string(), v.to_string()));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use bsengine_app::new_app;

    // `TypeRegistry` itself is not `Clone` (only the `Arc<RwLock<..>>`
    // wrapper `TypeRegistryArc` is); return the cheap-to-clone Arc wrapper
    // and let each call site take its own read lock, the same pattern
    // `populate_snapshot_extra_components` in `plugin.rs` uses.
    fn registry(world: &World) -> bevy_reflect::TypeRegistryArc {
        world
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .0
            .clone()
    }

    #[test]
    fn snapshot_captures_transform_primitive_and_material() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Body".to_string()),
                bsengine_core::Transform {
                    position: glam::Vec3::new(1.0, 2.0, 3.0).into(),
                    ..Default::default()
                },
                bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Sphere),
                bsengine_core::Material {
                    opacity: 0.5,
                    ..Default::default()
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(desc.name, "Body");
        assert_eq!(
            desc.transform.as_ref().unwrap().position,
            [1.0, 2.0, 3.0]
        );
        assert_eq!(desc.primitive, Some(bsengine_scene::Primitive::Sphere));
        assert_eq!(desc.opacity, Some(0.5));
    }

    #[test]
    fn snapshot_omits_fields_the_entity_has_no_component_for() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Empty".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(desc.transform, None);
        assert_eq!(desc.primitive, None);
        assert_eq!(desc.emissive, None);
        assert_eq!(desc.color, None);
        assert_eq!(desc.opacity, None);
    }

    #[test]
    fn snapshot_captures_an_arbitrary_reflected_component_in_the_components_catalog() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        app.register_type::<bsengine_core::Shield>();
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Hero".to_string()),
                bsengine_core::Shield::default(),
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert!(
            desc.components
                .iter()
                .any(|(type_path, _)| type_path.ends_with("Shield")),
            "components: {:?}",
            desc.components
        );
    }

    #[test]
    fn resolve_field_adopts_new_when_live_still_matches_baseline() {
        assert_eq!(resolve_field(&"live", &"live", &"new"), "new");
    }

    #[test]
    fn resolve_field_keeps_live_when_it_differs_from_baseline() {
        assert_eq!(resolve_field(&"overridden", &"baseline", &"new"), "overridden");
    }

    #[test]
    fn merge_entity_descriptor_adopts_unoverridden_fields_and_keeps_overridden_ones() {
        let baseline = bsengine_scene::EntityDescriptor {
            name: "Barrel".to_string(),
            primitive: Some(bsengine_scene::Primitive::Cube),
            color: Some([1.0, 0.0, 0.0]),
            ..Default::default()
        };
        let new = bsengine_scene::EntityDescriptor {
            name: "Barrel".to_string(),
            primitive: Some(bsengine_scene::Primitive::Sphere), // author changed the shape
            color: Some([1.0, 0.0, 0.0]),                       // unchanged
            ..Default::default()
        };
        let live = bsengine_scene::EntityDescriptor {
            name: "Barrel".to_string(),
            primitive: Some(bsengine_scene::Primitive::Cube), // matches baseline -> adopt new
            color: Some([0.0, 1.0, 0.0]),                     // user recolored it -> keep live
            ..Default::default()
        };

        let merged = merge_entity_descriptor(&live, &baseline, &new);

        assert_eq!(
            merged.primitive,
            Some(bsengine_scene::Primitive::Sphere),
            "unoverridden field must pick up the new file's value"
        );
        assert_eq!(
            merged.color,
            Some([0.0, 1.0, 0.0]),
            "overridden field must keep the user's live value, ignoring the new file"
        );
    }

    #[test]
    fn merge_components_adopts_a_new_value_for_an_unoverridden_component() {
        let baseline = vec![("pkg::Shield".to_string(), "(hp: 10)".to_string())];
        let new = vec![("pkg::Shield".to_string(), "(hp: 20)".to_string())];
        let live = baseline.clone(); // untouched since sync

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(merged, vec![("pkg::Shield".to_string(), "(hp: 20)".to_string())]);
    }

    #[test]
    fn merge_components_keeps_a_user_modified_component_value() {
        let baseline = vec![("pkg::Shield".to_string(), "(hp: 10)".to_string())];
        let new = vec![("pkg::Shield".to_string(), "(hp: 20)".to_string())];
        let live = vec![("pkg::Shield".to_string(), "(hp: 999)".to_string())]; // user edited it

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(merged, vec![("pkg::Shield".to_string(), "(hp: 999)".to_string())]);
    }

    #[test]
    fn merge_components_always_keeps_a_user_attached_component_with_no_baseline_entry() {
        let baseline: Vec<(String, String)> = vec![];
        let new: Vec<(String, String)> = vec![];
        let live = vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())];

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(merged, vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())]);
    }

    #[test]
    fn merge_components_adopts_a_brand_new_component_the_prefab_author_added() {
        let baseline: Vec<(String, String)> = vec![];
        let new = vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())];
        let live: Vec<(String, String)> = vec![];

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(merged, vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())]);
    }

    #[test]
    fn merge_components_adopts_a_removal_when_unoverridden() {
        let baseline = vec![("pkg::Shield".to_string(), "(hp: 10)".to_string())];
        let new: Vec<(String, String)> = vec![]; // author removed the component
        let live = baseline.clone(); // untouched since sync

        let merged = merge_components(&live, &baseline, &new);
        assert!(merged.is_empty());
    }

    #[test]
    fn merge_components_does_not_resurrect_a_component_the_user_detached() {
        let baseline = vec![("pkg::Shield".to_string(), "(hp: 10)".to_string())];
        let new = baseline.clone(); // source unchanged
        let live: Vec<(String, String)> = vec![]; // user detached it

        let merged = merge_components(&live, &baseline, &new);
        assert!(
            merged.is_empty(),
            "a component the user removed must not come back just because the source still has it"
        );
    }
}
