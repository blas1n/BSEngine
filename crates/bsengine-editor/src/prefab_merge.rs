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

/// Splits `live_name` at its last `'#'` into `(base, suffix)`, requiring the
/// suffix to be non-empty and all-ASCII-digit -- the exact shape
/// `instantiate_prefab`'s `rewrite_name` closure produces
/// (`format!("{name}#{suffix}")` with `suffix: u64`). Returns `None` for
/// anything else, e.g. a hand-authored name that happens to contain `'#'`
/// followed by non-digits, or no `'#'` at all.
fn strip_instance_suffix(live_name: &str) -> Option<(&str, &str)> {
    let (base, tail) = live_name.rsplit_once('#')?;
    if !tail.is_empty() && tail.bytes().all(|b| b.is_ascii_digit()) {
        Some((base, tail))
    } else {
        None
    }
}

/// Recovers this instance's shared naming suffix from any one of its
/// children -- every child of one `instantiate_prefab` call shares the exact
/// same suffix by construction, so the first one found is as good as any.
/// `None` if `children` is empty (a single-entity prefab, nothing to match)
/// or if, unexpectedly, none of them have a suffixed `Name`.
fn instance_suffix(world: &World, children: &[Entity]) -> Option<String> {
    children.iter().find_map(|&child| {
        let name = world.get::<bsengine_scene::Name>(child)?;
        strip_instance_suffix(&name.0).map(|(_, suffix)| suffix.to_string())
    })
}

/// Snapshots a live entity's current state into an [`EntityDescriptor`],
/// covering this PR's representative field set (`transform`, `primitive`,
/// `emissive`/`color`/`opacity`, `rigidbody`/`collider`/`linear_damping`/
/// `angular_damping`, and the reflected `components` catalog) plus `name`.
/// Every other `EntityDescriptor` field is left at its `Default`
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

    let transform = world.get::<bsengine_core::Transform>(entity).map(|t| {
        bsengine_scene::TransformDescriptor {
            position: t.position.0.to_array(),
            rotation: t.rotation.0.to_array(),
            scale: t.scale.0.to_array(),
        }
    });

    let primitive = world
        .get::<bsengine_scene::PrimitiveMesh>(entity)
        .map(|p| p.0.clone());

    let material = world.get::<bsengine_core::Material>(entity);
    let emissive = material.map(|m| m.emissive.0.to_array());
    let color = material.map(|m| m.base_color.0.to_array());
    let opacity = material.map(|m| m.opacity);

    let physics_body = world.get::<bsengine_scene::PhysicsBodyDesc>(entity);
    let rigidbody = physics_body.map(|p| p.rigidbody.clone());
    let collider = physics_body.map(|p| p.collider.clone());
    let linear_damping = physics_body.and_then(|p| p.linear_damping);
    let angular_damping = physics_body.and_then(|p| p.angular_damping);

    let components = snapshot_extra_components(world, registry, entity);

    bsengine_scene::EntityDescriptor {
        name,
        transform,
        primitive,
        emissive,
        color,
        opacity,
        rigidbody,
        collider,
        linear_damping,
        angular_damping,
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
/// own top-level logic, not by the generic field merge -- plus
/// `PhysicsBodyDesc`, for the same "already has a dedicated field" reason as
/// `Material`/`Transform`/`PrimitiveMesh` in `excluded_from_extra_components`
/// itself: it's `#[reflect(Component)]` (unlike its `RigidBodyDesc`/
/// `ColliderDesc` field types, which aren't components and so never surface
/// here regardless), so without this exclusion it would be captured twice --
/// once via `rigidbody`/`collider`/`linear_damping`/`angular_damping`, and
/// again as a raw `components:` entry -- and `apply_merged_descriptor`'s
/// generic `apply_merged_components` pass would silently resurrect whatever
/// stale value that second copy carried immediately after the dedicated
/// physics-body block above it removed or rewrote the component, since both
/// blocks touch the same entity in the same call.
fn merge_excluded_component_types() -> std::collections::HashSet<std::any::TypeId> {
    let mut excluded = crate::plugin::excluded_from_extra_components();
    excluded.insert(std::any::TypeId::of::<bsengine_core::PrefabInstance>());
    excluded.insert(std::any::TypeId::of::<bsengine_core::PrefabInstanceBaseline>());
    excluded.insert(std::any::TypeId::of::<bsengine_scene::PhysicsBodyDesc>());
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
/// there. Covers `transform`/`primitive`/`emissive`/`color`/`opacity` and the
/// physics field group (`rigidbody`/`collider`/`linear_damping`/
/// `angular_damping`), each resolved independently via `resolve_field`. Every
/// field this PR doesn't yet cover (see the plan's "Scope for this plan"
/// note) is left at `EntityDescriptor::default()`'s value on the returned
/// descriptor; callers must never treat that as "adopt an explicit clear"
/// for those fields -- `apply_merged_descriptor` (a later task) only ever
/// touches the fields this function actually resolves.
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
        rigidbody: resolve_field(&live.rigidbody, &baseline.rigidbody, &new.rigidbody),
        collider: resolve_field(&live.collider, &baseline.collider, &new.collider),
        linear_damping: resolve_field(
            &live.linear_damping,
            &baseline.linear_damping,
            &new.linear_damping,
        ),
        angular_damping: resolve_field(
            &live.angular_damping,
            &baseline.angular_damping,
            &new.angular_damping,
        ),
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
        pairs
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect()
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

/// Makes `entity`'s live components match `merged` for this PR's
/// representative field set, using `live` only to know what's currently
/// attached (so removal is a real `remove::<T>()` call rather than a no-op
/// insert of "nothing"). Idempotent: re-applying the same `merged` twice is
/// harmless, since every field write is "make it equal this value," not "add
/// this diff" -- this is what lets a later task's resync orchestrator call
/// this unconditionally for every matched entity rather than first checking
/// whether anything actually changed.
pub(crate) fn apply_merged_descriptor(
    world: &mut World,
    entity: Entity,
    registry: &TypeRegistry,
    live: &bsengine_scene::EntityDescriptor,
    merged: &bsengine_scene::EntityDescriptor,
) {
    match &merged.transform {
        Some(t) => {
            world.entity_mut(entity).insert((
                bsengine_core::Transform {
                    position: glam::Vec3::from(t.position).into(),
                    rotation: glam::Quat::from_xyzw(
                        t.rotation[0],
                        t.rotation[1],
                        t.rotation[2],
                        t.rotation[3],
                    )
                    .into(),
                    scale: glam::Vec3::from(t.scale).into(),
                },
                bsengine_core::GlobalTransform::default(),
            ));
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_core::Transform>();
        }
    }

    match &merged.primitive {
        Some(p) => {
            world
                .entity_mut(entity)
                .insert(bsengine_scene::PrimitiveMesh(p.clone()));
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_scene::PrimitiveMesh>();
        }
    }

    if merged.emissive.is_some() || merged.color.is_some() || merged.opacity.is_some() {
        // `Material` also carries `texture_id`/`metallic`/`roughness`, none of
        // which this PR's field set tracks (texture is a follow-up PR's
        // field group). Blindly inserting a fresh `Material { ..Default::default() }`
        // would silently wipe those out-of-scope fields to their defaults --
        // e.g. destroying a `texture:`-driven `texture_id` -- every time this
        // entity's Material is touched by a resync for an unrelated reason
        // (opacity is always "tracked," so this branch fires whenever a
        // Material exists at all). Read the entity's current Material first
        // and only overwrite the three fields this merge actually resolved.
        let mut material = world
            .get::<bsengine_core::Material>(entity)
            .cloned()
            .unwrap_or_default();
        if let Some(e) = merged.emissive {
            material.emissive = glam::Vec3::from(e).into();
        }
        if let Some(c) = merged.color {
            material.base_color = glam::Vec3::from(c).into();
        }
        if let Some(o) = merged.opacity {
            material.opacity = o;
        }
        world.entity_mut(entity).insert(material);
    } else {
        world.entity_mut(entity).remove::<bsengine_core::Material>();
    }

    // Mirrors `spawn_scene_entities`'s own gate for constructing this
    // component in the first place: both `rigidbody` and `collider` must be
    // present, or there's no complete physics body to describe. Unlike
    // Material, every field `PhysicsBodyDesc` has is one of the four fields
    // this merge tracks, so a full reconstruction is safe -- there's no
    // fifth, out-of-scope field to accidentally wipe.
    match (&merged.rigidbody, &merged.collider) {
        (Some(rigidbody), Some(collider)) => {
            world
                .entity_mut(entity)
                .insert(bsengine_scene::PhysicsBodyDesc {
                    rigidbody: rigidbody.clone(),
                    collider: collider.clone(),
                    linear_damping: merged.linear_damping,
                    angular_damping: merged.angular_damping,
                });
        }
        _ => {
            world
                .entity_mut(entity)
                .remove::<bsengine_scene::PhysicsBodyDesc>();
        }
    }

    apply_merged_components(
        world,
        entity,
        registry,
        &live.components,
        &merged.components,
    );
}

/// Reflection-based attach/detach for the `components:` catalog: removes
/// every live key `merged` no longer has, then inserts/overwrites every key
/// `merged` does have. Mirrors the two existing hand-rolled reflection
/// call-sites this codebase already has for the same primitives --
/// `spawn_scene_entities`'s `components:` loop (`apply_or_insert`) and
/// `process_reflect_commands`'s `RemoveComponentByType`/`AttachComponentByType`
/// handlers (`bsengine-editor/src/plugin.rs`) -- rather than inventing a
/// third calling convention.
fn apply_merged_components(
    world: &mut World,
    entity: Entity,
    registry: &TypeRegistry,
    live_components: &[(String, String)],
    merged_components: &[(String, String)],
) {
    let merged_keys: std::collections::HashSet<&str> =
        merged_components.iter().map(|(k, _)| k.as_str()).collect();

    for (type_path, _) in live_components {
        if merged_keys.contains(type_path.as_str()) {
            continue;
        }
        let Some(registration) = registry.get_with_type_path(type_path) else {
            continue;
        };
        let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
        else {
            continue;
        };
        let mut entity_mut = world.entity_mut(entity);
        reflect_component.remove(&mut entity_mut);
    }

    for (type_path, value_ron) in merged_components {
        let Some(registration) = registry.get_with_type_path(type_path) else {
            tracing::warn!(
                "prefab live-sync: entity {entity:?} merged component references unknown \
                 reflected type path '{type_path}'"
            );
            continue;
        };
        let Some(reflect_component) = registration.data::<bevy_ecs::reflect::ReflectComponent>()
        else {
            tracing::warn!(
                "prefab live-sync: entity {entity:?} type path '{type_path}' is not a \
                 registered Component"
            );
            continue;
        };
        let de = bevy_reflect::serde::TypedReflectDeserializer::new(registration, registry);
        match ron::de::Deserializer::from_str(value_ron) {
            Ok(mut deserializer) => {
                match serde::de::DeserializeSeed::deserialize(de, &mut deserializer) {
                    Ok(value) => {
                        let mut entity_mut = world.entity_mut(entity);
                        reflect_component.apply_or_insert(
                            &mut entity_mut,
                            value.as_ref(),
                            registry,
                        );
                    }
                    Err(e) => tracing::warn!(
                        "prefab live-sync: entity {entity:?} merged component '{type_path}' \
                         RON value doesn't match its shape: {e}"
                    ),
                }
            }
            Err(e) => tracing::warn!(
                "prefab live-sync: entity {entity:?} merged component '{type_path}' RON parse \
                 error: {e}"
            ),
        }
    }
}

/// Collects every live entity transitively parented under `root` that
/// belongs to *this* instance's own raw-name diffing -- i.e. every
/// descendant is still recorded (including a nested `prefab:` reference's
/// own root entity itself, which -- like any other entity -- has a raw name
/// the structural-diff loop needs to be able to look up), but the walk never
/// descends *past* a descendant carrying its own `PrefabInstance`,
/// unconditionally, regardless of whether that descendant's source path
/// happens to be a member of `own_source_paths`. This is deliberately
/// *stricter* than `prefab_watcher::despawn_subtree`'s traversal, which this
/// function used to reuse verbatim: `despawn_subtree` needs to keep
/// descending into a legitimate nested `prefab:` reference this same outer
/// file authors (so a resync of the outer file can cascade into it when the
/// reference itself changed), but this function's job is only to discover
/// *this* instance's own raw-name-matchable entities for the structural-diff
/// loop -- which, per the design spec's nested-boundary exclusion, never
/// needs to reach *inside* any nested instance's own subtree, own-authored
/// or not (though it does still need the boundary entity itself, to diff its
/// own `prefab:` field against baseline/new -- see `nested_reference_changed`).
/// Reusing `despawn_subtree`'s gate here didn't cause a live bug (traversal
/// order + suffix-matching happened to avoid collisions), but it was relying
/// on an implicit, non-obvious invariant instead of a boundary the
/// function's own purpose actually requires -- see the final whole-branch
/// review's "Minor" finding on this function. Note `root` itself always
/// carries a `PrefabInstance` too (it's this whole instance's own tracking
/// component) but must never be treated as a stop-boundary for its own
/// children -- the `cur != root` guard on the stop-check exists precisely so
/// this function still walks root's immediate structure instead of returning
/// empty. Takes `&mut World`, not `&World`, purely because `World::query`
/// itself requires it in this bevy_ecs version (0.14) -- exactly like
/// `despawn_subtree` right next to it, which is `&mut World` for the same
/// structural reason despite also never writing through it until its own
/// final despawn pass.
fn collect_own_descendants(world: &mut World, root: Entity) -> Vec<Entity> {
    let mut children_q = world.query::<(Entity, &bsengine_core::Parent)>();
    let mut visited = std::collections::HashSet::new();
    let mut result = Vec::new();
    let mut frontier = vec![root];
    while let Some(cur) = frontier.pop() {
        if !visited.insert(cur) {
            continue;
        }
        if cur != root {
            result.push(cur);
        }
        if cur != root && world.get::<bsengine_core::PrefabInstance>(cur).is_some() {
            continue; // record the boundary entity, but never descend past it
        }
        for (child, parent) in children_q.iter(world) {
            if parent.0 != cur {
                continue;
            }
            frontier.push(child);
        }
    }
    result
}

/// Whether a matched entity is a nested prefab reference whose reference
/// itself changed between `baseline` and `new` -- if so, it must be
/// despawned and freshly re-resolved rather than field-merged (its own
/// fields, like `primitive`, mean nothing; what matters is `prefab:` and the
/// instantiation-time `transform:` override `resolve_prefab_reference`
/// consumes). An unchanged reference is left alone entirely: the nested
/// subtree is independently owned by its own `PrefabInstance`/baseline and
/// resyncs only when *its* file changes, exactly as today.
fn nested_reference_changed(
    baseline: &bsengine_scene::EntityDescriptor,
    new: &bsengine_scene::EntityDescriptor,
) -> bool {
    let baseline_path = baseline.prefab.as_ref().map(|p| p.path());
    let new_path = new.prefab.as_ref().map(|p| p.path());
    baseline_path != new_path || baseline.transform != new.transform
}

/// Returns the shared instance suffix to stamp onto a freshly-spawned
/// entity's display name, drawing a fresh one via
/// `bsengine_scene::next_instance_suffix()` the first time this instance
/// needs one -- i.e. `*suffix` is still `None` because `collect_own_descendants`
/// found no existing non-root live entity to recover one from (a
/// single-root instance gaining its very first child during this very
/// resync). Every entity spawned within one `resync_instance` call must
/// share one suffix, exactly like a normal `instantiate_prefab` call gives
/// its whole subtree one shared suffix -- otherwise a later resync's
/// `instance_suffix` recovery (which trusts "any one" live child to speak
/// for the whole instance) would find an inconsistent value depending on
/// which child it happened to look at, and every differently-suffixed
/// sibling would silently fail `strip_instance_suffix`'s tail match and be
/// treated as if the user had deleted it.
fn suffixed_name(raw_name: &str, suffix: &mut Option<String>) -> String {
    let s = suffix.get_or_insert_with(|| bsengine_scene::next_instance_suffix().to_string());
    format!("{raw_name}#{s}")
}

/// Materializes an entity from `descriptor` that has no live counterpart yet
/// -- either because `new` just introduced it, or because a matched entity's
/// nested-reference status changed and its old subtree was already despawned
/// by the caller. Resolves it as a nested prefab reference when
/// `descriptor.prefab` is set (the same way `spawn_scene_entities` branches),
/// otherwise spawns it plainly via `spawn_single_entity`. Centralized here so
/// both call sites below get prefab-reference handling for free instead of
/// one of them silently treating a `prefab:` field as a plain field (which
/// `spawn_single_entity` explicitly does not understand -- see its own doc
/// comment).
///
/// `spawned_name` is the already-suffixed display name to give the spawned
/// entity (see `suffixed_name`) -- never `descriptor.name` (the file's raw,
/// unsuffixed spelling) directly, mirroring how every other non-root entity
/// in an instance is named. For a nested reference this becomes the nested
/// prefab's own root name verbatim (`instantiate_prefab`'s `root_name`
/// contract), exactly like `spawn_scene_entities`'s own `entity.prefab`
/// branch passes its (already-suffixed, since it's a non-root entity of the
/// outer file) `entity.name` straight through.
fn spawn_or_resolve_entity(
    world: &mut World,
    spawned_name: &str,
    descriptor: &bsengine_scene::EntityDescriptor,
) -> Option<Entity> {
    match &descriptor.prefab {
        Some(prefab_ref) => match bsengine_scene::instantiate_prefab_reference(
            world,
            spawned_name,
            prefab_ref,
            descriptor.transform.clone(),
        ) {
            Ok(fresh) => Some(fresh),
            Err(e) => {
                tracing::warn!(
                    "prefab live-sync: entity '{spawned_name}' references a prefab that failed \
                     to instantiate during resync: {e}"
                );
                None
            }
        },
        None => {
            let mut renamed = descriptor.clone();
            renamed.name = spawned_name.to_string();
            Some(bsengine_scene::spawn_single_entity(world, &renamed))
        }
    }
}

/// Patches one prefab instance rooted at `root` in place: merges the root's
/// own representative fields, then walks every non-root entity named in
/// `baseline`/`new`, applying the structural + field-level rules described
/// in this module's doc comment and the design spec. Returns an error only
/// if `baseline` or `new` isn't a validly-structured prefab (exactly one
/// root) -- the caller (`resync_prefab_instances`) already validates `new`
/// before calling this, so in practice only a corrupt baseline can trigger
/// this.
pub(crate) fn resync_instance(
    world: &mut World,
    root: Entity,
    baseline: &bsengine_scene::types::PrefabDescriptor,
    new: &bsengine_scene::types::PrefabDescriptor,
    own_source_paths: &std::collections::HashSet<String>,
) -> Result<(), String> {
    let registry = world
        .resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .clone();
    let registry = registry.read();

    let baseline_root = bsengine_scene::validate_prefab_descriptor(baseline)?;
    let new_root = bsengine_scene::validate_prefab_descriptor(new)?;

    let root_live = snapshot_entity_as_descriptor(world, &registry, root);
    let mut root_merged = merge_entity_descriptor(&root_live, baseline_root, new_root);
    // The root's placement was never meant to come from the prefab asset in
    // the first place -- this predates override tracking (see the existing
    // `transform_override` capture/reapply in `resync_prefab_instances`
    // before this feature) and the design spec calls it out explicitly:
    // Name/Transform/Parent are never diffed for the root, unconditionally,
    // regardless of override status. `merge_entity_descriptor` has no way to
    // know it's being called for a root vs. a regular entity, so it applies
    // the general per-field rule to `transform` like any other field; this
    // overwrites that back to the live value every time, after the fact,
    // rather than teaching the general-purpose merge function about roots.
    root_merged.transform = root_live.transform.clone();
    apply_merged_descriptor(world, root, &registry, &root_live, &root_merged);

    let own_descendants = collect_own_descendants(world, root);
    let mut suffix = instance_suffix(world, &own_descendants);

    let mut resolved_by_raw_name: std::collections::HashMap<String, Entity> =
        std::collections::HashMap::new();
    if let Some(suffix) = &suffix {
        for &e in &own_descendants {
            if let Some(name) = world.get::<bsengine_scene::Name>(e) {
                if let Some((raw, tail)) = strip_instance_suffix(&name.0) {
                    if tail == suffix {
                        resolved_by_raw_name.insert(raw.to_string(), e);
                    }
                }
            }
        }
    }

    let baseline_by_name: std::collections::HashMap<&str, &bsengine_scene::EntityDescriptor> =
        baseline
            .entities
            .iter()
            .filter(|e| e.name != baseline_root.name)
            .map(|e| (e.name.as_str(), e))
            .collect();
    let new_by_name: std::collections::HashMap<&str, &bsengine_scene::EntityDescriptor> = new
        .entities
        .iter()
        .filter(|e| e.name != new_root.name)
        .map(|e| (e.name.as_str(), e))
        .collect();

    let mut all_names: Vec<&str> = baseline_by_name
        .keys()
        .chain(new_by_name.keys())
        .copied()
        .collect();
    all_names.sort_unstable();
    all_names.dedup();

    let mut spawned_needing_parent: Vec<(Entity, Option<String>)> = Vec::new();

    for raw_name in all_names {
        let in_baseline = baseline_by_name.get(raw_name).copied();
        let in_new = new_by_name.get(raw_name).copied();
        // `.filter(...)` re-validates liveness rather than trusting the map's
        // stored value: `resolved_by_raw_name` is only ever pruned for the
        // *specific* raw_name a `despawn_subtree` call was triggered for
        // (see the two `resolved_by_raw_name.remove(raw_name)` calls below),
        // but `despawn_subtree` itself cascades and despawns every live
        // descendant of that entity too -- including ones that are
        // *themselves* separately-declared names in this same prefab (a
        // 3+-level hierarchy, e.g. a removed "Barrel" whose live child
        // "Scope" is still a distinct key in `all_names`). Without this
        // check, such a descendant's map entry is stale by the time this
        // loop reaches its own raw_name, and the `(Some(_), Some(_),
        // Some(live_e))` arm below would call `apply_merged_descriptor` on a
        // dead `Entity`, which panics inside `World::entity_mut`. Filtering
        // here instead routes it into the existing `(Some(_), Some(_),
        // None) => already gone, respected` arm -- exactly the outcome the
        // design spec's structural-removal rule already intends, since this
        // entity's removal cascaded precisely like an explicit deletion.
        let live_entity = resolved_by_raw_name
            .get(raw_name)
            .copied()
            .filter(|&e| world.get_entity(e).is_some());

        match (in_baseline, in_new, live_entity) {
            (Some(_), None, Some(live_e)) => {
                crate::prefab_watcher::despawn_subtree(world, live_e, own_source_paths);
                resolved_by_raw_name.remove(raw_name);
            }
            (Some(_), None, None) | (Some(_), Some(_), None) => {
                // Already gone (cascaded from a removed ancestor, or the
                // user deleted it directly) -- respected either way.
            }
            (Some(b), Some(n), Some(live_e)) if b.prefab.is_some() || n.prefab.is_some() => {
                if nested_reference_changed(b, n) {
                    crate::prefab_watcher::despawn_subtree(world, live_e, own_source_paths);
                    resolved_by_raw_name.remove(raw_name);
                    // `n.prefab` may itself be `None` here -- the entity was a
                    // nested reference in `baseline` but the author converted
                    // it to a plain entity in `new`. `spawn_or_resolve_entity`
                    // handles both cases; this call site must not assume
                    // `n.prefab.is_some()` just because the *match guard*
                    // above required *either* side to have it.
                    let spawned_name = suffixed_name(raw_name, &mut suffix);
                    if let Some(fresh) = spawn_or_resolve_entity(world, &spawned_name, n) {
                        resolved_by_raw_name.insert(raw_name.to_string(), fresh);
                        spawned_needing_parent.push((fresh, n.parent.clone()));
                    }
                }
                // else: unchanged reference, left entirely alone.
            }
            (Some(b), Some(n), Some(live_e)) => {
                let live_desc = snapshot_entity_as_descriptor(world, &registry, live_e);
                let merged = merge_entity_descriptor(&live_desc, b, n);
                apply_merged_descriptor(world, live_e, &registry, &live_desc, &merged);
            }
            (None, Some(n), _) => {
                let spawned_name = suffixed_name(raw_name, &mut suffix);
                if let Some(fresh) = spawn_or_resolve_entity(world, &spawned_name, n) {
                    resolved_by_raw_name.insert(raw_name.to_string(), fresh);
                    spawned_needing_parent.push((fresh, n.parent.clone()));
                }
            }
            (None, None, _) => unreachable!("raw_name is drawn from baseline's or new's own keys"),
        }
    }

    for (entity, parent_raw_name) in spawned_needing_parent {
        let Some(parent_raw_name) = parent_raw_name else {
            continue; // no parent: field -> a genuine root within this instance (shouldn't happen since only the file's own root has no parent, and non-root entities always name one) -- leave unparented rather than guess
        };
        if let Some(&parent_entity) = resolved_by_raw_name.get(parent_raw_name.as_str()) {
            world
                .entity_mut(entity)
                .insert(bsengine_core::Parent(parent_entity));
        } else if parent_raw_name == new_root.name {
            world.entity_mut(entity).insert(bsengine_core::Parent(root));
        } else {
            tracing::warn!(
                "prefab live-sync: freshly-spawned entity names parent '{parent_raw_name}', \
                 which does not exist in the resynced prefab; leaving it unparented"
            );
        }
    }

    Ok(())
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
        assert_eq!(desc.transform.as_ref().unwrap().position, [1.0, 2.0, 3.0]);
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
    fn snapshot_captures_a_live_physics_body() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Crate".to_string()),
                bsengine_scene::PhysicsBodyDesc {
                    rigidbody: bsengine_scene::RigidBodyDesc::Dynamic,
                    collider: bsengine_scene::ColliderDesc {
                        shape: bsengine_scene::ColliderShapeDesc::Box {
                            hx: 0.5,
                            hy: 0.5,
                            hz: 0.5,
                        },
                        restitution: 0.1,
                        friction: 0.5,
                        sensor: false,
                    },
                    linear_damping: Some(0.2),
                    angular_damping: None,
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(desc.rigidbody, Some(bsengine_scene::RigidBodyDesc::Dynamic));
        assert_eq!(
            desc.collider,
            Some(bsengine_scene::ColliderDesc {
                shape: bsengine_scene::ColliderShapeDesc::Box {
                    hx: 0.5,
                    hy: 0.5,
                    hz: 0.5
                },
                restitution: 0.1,
                friction: 0.5,
                sensor: false,
            })
        );
        assert_eq!(desc.linear_damping, Some(0.2));
        assert_eq!(desc.angular_damping, None);
    }

    #[test]
    fn snapshot_omits_physics_fields_when_no_physics_body_present() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Ghost".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(desc.rigidbody, None);
        assert_eq!(desc.collider, None);
        assert_eq!(desc.linear_damping, None);
        assert_eq!(desc.angular_damping, None);
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
        assert_eq!(
            resolve_field(&"overridden", &"baseline", &"new"),
            "overridden"
        );
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
    fn merge_entity_descriptor_preserves_an_overridden_physics_field_while_adopting_others() {
        let baseline = bsengine_scene::EntityDescriptor {
            name: "Crate".to_string(),
            rigidbody: Some(bsengine_scene::RigidBodyDesc::Dynamic),
            collider: Some(bsengine_scene::ColliderDesc {
                shape: bsengine_scene::ColliderShapeDesc::Box {
                    hx: 0.5,
                    hy: 0.5,
                    hz: 0.5,
                },
                restitution: 0.1,
                friction: 0.5,
                sensor: false,
            }),
            linear_damping: Some(0.2),
            angular_damping: Some(0.1),
            ..Default::default()
        };
        let new = bsengine_scene::EntityDescriptor {
            name: "Crate".to_string(),
            rigidbody: Some(bsengine_scene::RigidBodyDesc::Dynamic),
            collider: Some(bsengine_scene::ColliderDesc {
                // Author widened the box -- unoverridden, must be adopted.
                shape: bsengine_scene::ColliderShapeDesc::Box {
                    hx: 1.0,
                    hy: 1.0,
                    hz: 1.0,
                },
                restitution: 0.1,
                friction: 0.5,
                sensor: false,
            }),
            linear_damping: Some(0.2),  // unchanged
            angular_damping: Some(0.9), // author changed this too, but it's overridden below
            ..Default::default()
        };
        let live = bsengine_scene::EntityDescriptor {
            name: "Crate".to_string(),
            rigidbody: Some(bsengine_scene::RigidBodyDesc::Dynamic), // matches baseline -> adopt new
            collider: baseline.collider.clone(), // matches baseline -> adopt new
            linear_damping: Some(0.2),           // matches baseline -> adopt new (no-op change)
            angular_damping: Some(0.75),         // user tuned this -> override, keep live
            ..Default::default()
        };

        let merged = merge_entity_descriptor(&live, &baseline, &new);

        assert_eq!(
            merged.collider, new.collider,
            "unoverridden collider shape must adopt the new file's value"
        );
        assert_eq!(
            merged.angular_damping,
            Some(0.75),
            "the user's angular_damping override must survive, ignoring the new file's value"
        );
    }

    #[test]
    fn merge_components_adopts_a_new_value_for_an_unoverridden_component() {
        let baseline = vec![("pkg::Shield".to_string(), "(hp: 10)".to_string())];
        let new = vec![("pkg::Shield".to_string(), "(hp: 20)".to_string())];
        let live = baseline.clone(); // untouched since sync

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(
            merged,
            vec![("pkg::Shield".to_string(), "(hp: 20)".to_string())]
        );
    }

    #[test]
    fn merge_components_keeps_a_user_modified_component_value() {
        let baseline = vec![("pkg::Shield".to_string(), "(hp: 10)".to_string())];
        let new = vec![("pkg::Shield".to_string(), "(hp: 20)".to_string())];
        let live = vec![("pkg::Shield".to_string(), "(hp: 999)".to_string())]; // user edited it

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(
            merged,
            vec![("pkg::Shield".to_string(), "(hp: 999)".to_string())]
        );
    }

    #[test]
    fn merge_components_always_keeps_a_user_attached_component_with_no_baseline_entry() {
        let baseline: Vec<(String, String)> = vec![];
        let new: Vec<(String, String)> = vec![];
        let live = vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())];

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(
            merged,
            vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())]
        );
    }

    #[test]
    fn merge_components_adopts_a_brand_new_component_the_prefab_author_added() {
        let baseline: Vec<(String, String)> = vec![];
        let new = vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())];
        let live: Vec<(String, String)> = vec![];

        let merged = merge_components(&live, &baseline, &new);
        assert_eq!(
            merged,
            vec![("pkg::Shield".to_string(), "(hp: 5)".to_string())]
        );
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

    #[test]
    fn apply_merged_descriptor_updates_an_adopted_field_and_leaves_an_overridden_one() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Barrel".to_string()),
                bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Cube),
                bsengine_core::Material {
                    base_color: glam::Vec3::new(0.0, 1.0, 0.0).into(),
                    ..Default::default()
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            primitive: Some(bsengine_scene::Primitive::Sphere), // adopted change
            color: live.color,                                  // kept as-is (override)
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        assert_eq!(
            app.world()
                .get::<bsengine_scene::PrimitiveMesh>(entity)
                .unwrap()
                .0,
            bsengine_scene::Primitive::Sphere
        );
        assert_eq!(
            app.world()
                .get::<bsengine_core::Material>(entity)
                .unwrap()
                .base_color
                .0
                .to_array(),
            [0.0, 1.0, 0.0]
        );
    }

    #[test]
    fn apply_merged_descriptor_removes_a_component_the_merge_dropped() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Barrel".to_string()),
                bsengine_scene::PrimitiveMesh(bsengine_scene::Primitive::Cube),
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            primitive: None, // the prefab author removed the primitive field, unoverridden
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        assert!(
            app.world()
                .get::<bsengine_scene::PrimitiveMesh>(entity)
                .is_none(),
            "PrimitiveMesh must be removed when the merged descriptor no longer has a primitive"
        );
    }

    #[test]
    fn apply_merged_descriptor_preserves_material_fields_outside_this_prs_scope() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Barrel".to_string()),
                bsengine_core::Material {
                    texture_id: Some(42),
                    metallic: 0.7,
                    roughness: 0.3,
                    base_color: glam::Vec3::new(1.0, 0.0, 0.0).into(),
                    ..Default::default()
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            color: Some([0.0, 0.0, 1.0]), // only color is adopted from `new`
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        let material = app.world().get::<bsengine_core::Material>(entity).unwrap();
        assert_eq!(
            material.base_color.0.to_array(),
            [0.0, 0.0, 1.0],
            "color must update"
        );
        assert_eq!(
            material.texture_id,
            Some(42),
            "texture_id is outside this PR's field set and must survive untouched"
        );
        assert_eq!(material.metallic, 0.7, "metallic must survive untouched");
        assert_eq!(material.roughness, 0.3, "roughness must survive untouched");
    }

    #[test]
    fn apply_merged_descriptor_attaches_and_detaches_reflected_components_per_the_merged_catalog() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        app.register_type::<bsengine_core::Shield>();
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Hero".to_string()))
            .id();
        // A scratch entity that already has `Shield` attached, so its snapshot
        // gives us the component's *real* reflected type path -- rather than
        // hardcoding a guess at "bsengine_core::shield::Shield" -- to build
        // `merged.components` from.
        let shield_entity = app.world_mut().spawn(bsengine_core::Shield::default()).id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let shield_snapshot = snapshot_entity_as_descriptor(app.world(), &reg, shield_entity);
        let shield_component = shield_snapshot
            .components
            .into_iter()
            .find(|(type_path, _)| type_path.ends_with("Shield"))
            .expect("scratch entity's snapshot must contain its Shield component");
        let merged = bsengine_scene::EntityDescriptor {
            components: vec![shield_component],
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        assert!(app.world().get::<bsengine_core::Shield>(entity).is_some());
    }

    #[test]
    fn apply_merged_descriptor_inserts_a_physics_body_when_merged_has_both_halves() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Crate".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            rigidbody: Some(bsengine_scene::RigidBodyDesc::Dynamic),
            collider: Some(bsengine_scene::ColliderDesc {
                shape: bsengine_scene::ColliderShapeDesc::Sphere { radius: 1.0 },
                restitution: 0.1,
                friction: 0.5,
                sensor: false,
            }),
            linear_damping: Some(0.3),
            angular_damping: None,
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        let body = app
            .world()
            .get::<bsengine_scene::PhysicsBodyDesc>(entity)
            .expect("PhysicsBodyDesc must be inserted when merged has both rigidbody and collider");
        assert_eq!(body.rigidbody, bsengine_scene::RigidBodyDesc::Dynamic);
        assert_eq!(
            body.collider.shape,
            bsengine_scene::ColliderShapeDesc::Sphere { radius: 1.0 }
        );
        assert_eq!(body.linear_damping, Some(0.3));
        assert_eq!(body.angular_damping, None);
    }

    #[test]
    fn apply_merged_descriptor_removes_a_physics_body_when_merged_is_missing_either_half() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Crate".to_string()),
                bsengine_scene::PhysicsBodyDesc {
                    rigidbody: bsengine_scene::RigidBodyDesc::Dynamic,
                    collider: bsengine_scene::ColliderDesc {
                        shape: bsengine_scene::ColliderShapeDesc::Sphere { radius: 1.0 },
                        restitution: 0.1,
                        friction: 0.5,
                        sensor: false,
                    },
                    linear_damping: None,
                    angular_damping: None,
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        // Source removed `collider:` (e.g. the author deleted the collider block),
        // unoverridden -- merged ends up with rigidbody but no collider.
        let merged = bsengine_scene::EntityDescriptor {
            rigidbody: live.rigidbody.clone(),
            collider: None,
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        assert!(
            app.world()
                .get::<bsengine_scene::PhysicsBodyDesc>(entity)
                .is_none(),
            "a merged result with only one half of the rigidbody/collider pair must remove the \
             component entirely, matching spawn_scene_entities's own \"both required\" rule"
        );
    }

    #[test]
    fn strip_instance_suffix_splits_at_the_last_hash_when_the_tail_is_numeric() {
        assert_eq!(strip_instance_suffix("Barrel#42"), Some(("Barrel", "42")));
        assert_eq!(
            strip_instance_suffix("My#Weird#Name#7"),
            Some(("My#Weird#Name", "7"))
        );
    }

    #[test]
    fn strip_instance_suffix_rejects_a_non_numeric_or_missing_tail() {
        assert_eq!(strip_instance_suffix("NoHashAtAll"), None);
        assert_eq!(strip_instance_suffix("Trailing#"), None);
        assert_eq!(strip_instance_suffix("NotDigits#abc"), None);
    }

    #[test]
    fn instance_suffix_is_recovered_from_any_one_matching_child() {
        let mut app = new_app();
        let a = app
            .world_mut()
            .spawn(bsengine_scene::Name("Barrel#7".to_string()))
            .id();
        let b = app
            .world_mut()
            .spawn(bsengine_scene::Name("Scope#7".to_string()))
            .id();

        assert_eq!(instance_suffix(app.world(), &[a, b]), Some("7".to_string()));
    }

    #[test]
    fn instance_suffix_is_none_with_no_children() {
        let app = new_app();
        assert_eq!(instance_suffix(app.world(), &[]), None);
    }

    fn parse(ron_str: &str) -> bsengine_scene::types::PrefabDescriptor {
        ron::from_str(ron_str).unwrap()
    }

    #[test]
    fn resync_instance_preserves_an_overridden_transform_while_updating_a_sibling_field() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        // User manually moves Barrel and never touches its primitive.
        let barrel = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Barrel#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        app.world_mut()
            .entity_mut(barrel)
            .insert(bsengine_core::Transform {
                position: glam::Vec3::new(5.0, 0.0, 0.0).into(),
                ..Default::default()
            });

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Sphere)),
            ])"#,
        );

        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let barrel_after = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Barrel#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        assert_eq!(
            barrel_after, barrel,
            "an entity with no structural change must keep its live Entity id, not respawn"
        );
        assert_eq!(
            app.world()
                .get::<bsengine_core::Transform>(barrel_after)
                .unwrap()
                .position
                .0
                .to_array(),
            [5.0, 0.0, 0.0],
            "the user's manual transform override must survive the resync"
        );
        assert_eq!(
            app.world()
                .get::<bsengine_scene::PrimitiveMesh>(barrel_after)
                .unwrap()
                .0,
            bsengine_scene::Primitive::Sphere,
            "the unoverridden primitive field must still update to the new file's value"
        );
    }

    #[test]
    fn resync_instance_preserves_a_manually_added_child_entity() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        let flag = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Flag".to_string()),
                bsengine_core::Parent(root),
            ))
            .id();

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Sphere)),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        assert!(
            app.world().get_entity(flag).is_some(),
            "a manually-added child entity must survive an unrelated field update on its siblings"
        );
    }

    #[test]
    fn resync_instance_cascade_deletes_a_removed_entity_even_with_overrides_under_it() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        let barrel = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Barrel#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        // Override Barrel's own field, and add a manual child under it -- both
        // must be swept away when the source removes Barrel entirely.
        app.world_mut()
            .entity_mut(barrel)
            .insert(bsengine_scene::PrimitiveMesh(
                bsengine_scene::Primitive::Sphere,
            ));
        let flag = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Flag".to_string()),
                bsengine_core::Parent(barrel),
            ))
            .id();

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        assert!(
            app.world().get_entity(barrel).is_none(),
            "Barrel must be despawned"
        );
        assert!(
            app.world().get_entity(flag).is_none(),
            "Flag must cascade-despawn with its parent, despite being a manual addition"
        );
    }

    #[test]
    fn resync_instance_does_not_resurrect_a_user_deleted_prefab_authored_entity() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        let barrel = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Barrel#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        app.world_mut().despawn(barrel); // user deletes it directly

        let new = baseline.clone(); // source unchanged, still has Barrel
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let barrel_count = {
            let mut q = app.world_mut().query::<&bsengine_scene::Name>();
            q.iter(app.world())
                .filter(|n| n.0.starts_with("Barrel#"))
                .count()
        };
        assert_eq!(
            barrel_count, 0,
            "a user-deleted prefab-authored entity must not come back"
        );
    }

    #[test]
    fn resync_instance_never_adopts_the_new_files_root_transform_even_when_unoverridden() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube), transform: Some((position: (0.0, 0.0, 0.0)))),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            Some(bsengine_scene::TransformDescriptor {
                position: [10.0, 0.0, 0.0], // the instance's own placement in the scene
                ..Default::default()
            }),
            None,
        )
        .unwrap();

        // The author moves the prefab's own authored root transform -- this must
        // never affect a placed instance's position, overridden or not; a
        // prefab instance's placement is never prefab-owned.
        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube), transform: Some((position: (99.0, 99.0, 99.0)))),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        assert_eq!(
            app.world()
                .get::<bsengine_core::Transform>(root)
                .unwrap()
                .position
                .0
                .to_array(),
            [10.0, 0.0, 0.0],
            "the root's transform must stay exactly what the instance had, ignoring the new file \
             entirely -- root transform is never diffed, unlike every other representative field"
        );
    }

    #[test]
    fn resync_instance_spawns_a_brand_new_entity_the_source_added() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Scope", parent: Some("Body"), primitive: Some(Sphere)),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let scope = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Scope#"))
                .map(|(e, _)| e)
        };
        assert!(
            scope.is_some(),
            "the newly-added Scope entity must be spawned"
        );
        assert_eq!(
            app.world()
                .get::<bsengine_core::Parent>(scope.unwrap())
                .map(|p| p.0),
            Some(root),
            "the new entity must be parented correctly"
        );
    }

    #[test]
    fn resync_instance_does_not_panic_when_a_removed_entitys_cascade_hits_a_still_declared_descendant(
    ) {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        // Three-level hierarchy: Body -> Barrel -> Scope. `new` removes
        // Barrel entirely and re-declares Scope directly under Body instead
        // -- an ordinary "delete the middle entity, reparent its children up
        // one level" edit, requiring no overrides. This reproduces the
        // final-review panic: `despawn_subtree(world, barrel, ..)` cascades
        // and despawns *both* Barrel and its live child Scope, but only
        // `"Barrel"`'s own key was ever pruned from `resolved_by_raw_name`.
        // When the structural-diff loop later reached `"Scope"`'s own
        // raw_name (alphabetically after `"Barrel"`), it read a stale,
        // already-despawned `Entity` for it and `apply_merged_descriptor`
        // panicked inside `World::entity_mut`.
        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Barrel", parent: Some("Body"), primitive: Some(Cube)),
                EntityDescriptor(name: "Scope", parent: Some("Barrel"), primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret.ron",
            Some("MyTurret"),
            None,
            None,
        )
        .unwrap();

        let barrel = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Barrel#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        let scope = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Scope#"))
                .map(|(e, _)| e)
                .unwrap()
        };

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Scope", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);

        // Must not panic -- reaching the assertions below is itself the
        // primary regression proof.
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        assert!(
            app.world().get_entity(barrel).is_none(),
            "Barrel must be despawned -- the source removed it"
        );
        assert!(
            app.world().get_entity(scope).is_none(),
            "the original live Scope entity must be gone too: it was a live descendant of \
             Barrel at the moment Barrel's structural removal cascaded, and the design spec's \
             removal rule is unconditional (\"that entity's entire live subtree is despawned... \
             regardless of... manually-added children under it\") -- Scope being independently \
             re-declared under Body in `new` does not exempt it from a cascade it was already \
             caught in. Consistent with rule 6 (\"a user-deleted, still-prefab-authored entity \
             is not resurrected\"): once the liveness check finds Scope's raw_name has no live \
             entity left, it is respected as already-gone rather than freshly respawned"
        );

        let scope_count = {
            let mut q = app.world_mut().query::<&bsengine_scene::Name>();
            q.iter(app.world())
                .filter(|n| n.0.starts_with("Scope#"))
                .count()
        };
        assert_eq!(
            scope_count, 0,
            "no fresh replacement Scope entity should have been spawned under Body either -- \
             the cascade-despawn is respected as a deletion, not treated as \"new entity the \
             source added\" just because Scope's raw_name still resolves in `new`"
        );
    }
}
