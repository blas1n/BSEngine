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
}
