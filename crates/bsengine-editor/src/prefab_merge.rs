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
/// `angular_damping`, `point_light`/`spot_light`/`directional_light`,
/// `camera`/`camera_fov`, `gltf`/`script`/`texture`, and the reflected
/// `components` catalog) plus `name`.
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

    let point_light = world.get::<bsengine_core::PointLight>(entity).map(|pl| {
        bsengine_scene::PointLightDescriptor {
            color: pl.color.0.to_array(),
            intensity: pl.intensity,
            range: pl.range,
        }
    });

    let spot_light = world.get::<bsengine_core::SpotLight>(entity).map(|sl| {
        bsengine_scene::SpotLightDescriptor {
            color: sl.color.0.to_array(),
            intensity: sl.intensity,
            range: sl.range,
            inner_angle_degrees: sl.inner_angle_degrees.0,
            outer_angle_degrees: sl.outer_angle_degrees.0,
        }
    });

    // `direction` has no live counterpart -- it drives a one-time
    // Transform.rotation write at instantiation and is never stored on the
    // live entity afterward (see the design spec). This placeholder is never
    // read by anything: `resolve_directional_light` (a later task) compares
    // only `color`/`ambient`, and `apply_merged_descriptor` never reads
    // `direction` back off the merged result.
    let directional_light = world
        .get::<bsengine_core::DirectionalLight>(entity)
        .map(|dl| bsengine_scene::DirectionalLightDescriptor {
            direction: [0.0, 0.0, -1.0],
            color: dl.color.0.to_array(),
            ambient: dl.ambient.0.to_array(),
        });

    // `look_at` (like `direction` above) has no live counterpart -- it
    // drives a one-time Transform.rotation write at instantiation and is
    // never stored anywhere retrievable afterward. There is deliberately no
    // `look_at` snapshot here at all (unlike `direction`, there isn't even a
    // placeholder to write): `EntityDescriptor::default()`'s `None` is
    // already correct, and neither `merge_entity_descriptor` nor
    // `apply_merged_descriptor` (later tasks) ever touch this field.
    let camera = world.get::<bsengine_core::Camera>(entity);
    let camera_fov = camera.map(|c| c.fov_y_degrees.0);

    // Each of these is already the fully-resolved live path (see
    // `resolve_asset_ref_for_field`'s doc comment) -- wrapped as a bare
    // `AssetRef::Path` since a live component never carries a guid to
    // round-trip. `resync_instance`'s `patch_asset_ref_overrides` (a later
    // task) is what actually decides override-vs-adopt for these three
    // fields; this snapshot just reports what's live right now, same as
    // every other field this function captures.
    let gltf = world
        .get::<bsengine_gltf::GltfAsset>(entity)
        .map(|g| bsengine_scene::AssetRef::Path(g.path.clone()));
    let script = world
        .get::<bsengine_scene::ScriptPath>(entity)
        .map(|s| bsengine_scene::AssetRef::Path(s.0.clone()));
    let texture = world
        .get::<bsengine_core::TexturePath>(entity)
        .map(|t| bsengine_scene::AssetRef::Path(t.0.clone()));

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
        point_light,
        spot_light,
        directional_light,
        camera: camera.is_some(),
        camera_fov,
        gltf,
        script,
        texture,
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

/// Merges one light-like descriptor's individual attributes independently,
/// the same granularity the physics field group already gets by being split
/// into four separate top-level `EntityDescriptor` fields (`rigidbody`/
/// `collider`/`linear_damping`/`angular_damping`) instead of one atomic
/// struct: a user who only retuned `intensity` must not lose an unrelated
/// `range` change the prefab author made in the same file, and vice versa.
/// Whether the light exists at all (`Some`/`None`) is still resolved
/// atomically via `resolve_field`, exactly like every other `Option<T>`
/// field in this module -- there is no meaningful way to "partially" have a
/// light, so that dimension only decomposes once all three sides agree the
/// light exists.
fn merge_point_light(
    live: &Option<bsengine_scene::PointLightDescriptor>,
    baseline: &Option<bsengine_scene::PointLightDescriptor>,
    new: &Option<bsengine_scene::PointLightDescriptor>,
) -> Option<bsengine_scene::PointLightDescriptor> {
    match (live, baseline, new) {
        (Some(l), Some(b), Some(n)) => Some(bsengine_scene::PointLightDescriptor {
            color: resolve_field(&l.color, &b.color, &n.color),
            intensity: resolve_field(&l.intensity, &b.intensity, &n.intensity),
            range: resolve_field(&l.range, &b.range, &n.range),
        }),
        _ => resolve_field(live, baseline, new),
    }
}

/// Same rationale and shape as `merge_point_light`, for `SpotLightDescriptor`'s
/// five independently-tunable attributes.
fn merge_spot_light(
    live: &Option<bsengine_scene::SpotLightDescriptor>,
    baseline: &Option<bsengine_scene::SpotLightDescriptor>,
    new: &Option<bsengine_scene::SpotLightDescriptor>,
) -> Option<bsengine_scene::SpotLightDescriptor> {
    match (live, baseline, new) {
        (Some(l), Some(b), Some(n)) => Some(bsengine_scene::SpotLightDescriptor {
            color: resolve_field(&l.color, &b.color, &n.color),
            intensity: resolve_field(&l.intensity, &b.intensity, &n.intensity),
            range: resolve_field(&l.range, &b.range, &n.range),
            inner_angle_degrees: resolve_field(
                &l.inner_angle_degrees,
                &b.inner_angle_degrees,
                &n.inner_angle_degrees,
            ),
            outer_angle_degrees: resolve_field(
                &l.outer_angle_degrees,
                &b.outer_angle_degrees,
                &n.outer_angle_degrees,
            ),
        }),
        _ => resolve_field(live, baseline, new),
    }
}

/// Same per-attribute spirit as `merge_point_light`/`merge_spot_light`,
/// comparing and resolving only `color`/`ambient` independently -- never
/// `direction`. `direction` has no live counterpart (see
/// `snapshot_entity_as_descriptor`'s doc comment) and is never read back off
/// this function's result by `apply_merged_descriptor`, so comparing it
/// would only ever inject noise: every live snapshot's `direction` is the
/// same fixed placeholder, permanently "different" from whatever real value
/// a prefab file authors, which would make every directional light look
/// permanently overridden. The merged result's own `direction` is taken from
/// `new` when both sides have a light -- an arbitrary but harmless choice,
/// since nothing ever reads it back. Whether the light exists at all is
/// still resolved atomically, for the same reason `merge_point_light`
/// resolves presence atomically.
fn resolve_directional_light(
    live: &Option<bsengine_scene::DirectionalLightDescriptor>,
    baseline: &Option<bsengine_scene::DirectionalLightDescriptor>,
    new: &Option<bsengine_scene::DirectionalLightDescriptor>,
) -> Option<bsengine_scene::DirectionalLightDescriptor> {
    match (live, baseline, new) {
        (Some(l), Some(b), Some(n)) => Some(bsengine_scene::DirectionalLightDescriptor {
            direction: n.direction,
            color: resolve_field(&l.color, &b.color, &n.color),
            ambient: resolve_field(&l.ambient, &b.ambient, &n.ambient),
        }),
        (None, None, _) => new.clone(),
        _ => live.clone(),
    }
}

/// Resolves one `gltf`/`script`/`texture`-style `AssetRef` field. Unlike
/// `resolve_directional_light` (which skips a field with no live
/// counterpart), this is comparing `live`'s already-resolved path against
/// what re-resolving `baseline`'s raw reference would produce *right now*
/// -- raw equality between `live` and `baseline`'s `AssetRef`s would almost
/// always be false even when nothing changed, because
/// `resolve_asset_ref_for_field` can rewrite the stored path via guid-based
/// self-healing and, for `gltf` only, a `ProjectDir` prefix (see the design
/// spec's "why this cluster breaks every prior field group's core
/// assumption"). Both outcomes below carry an already-fully-resolved plain
/// path wrapped as `AssetRef::Path` -- never a re-derivable
/// `Identified { guid, .. }` -- so `apply_merged_descriptor` can write it
/// directly without resolving again; resolving twice would risk
/// double-prefixing `ProjectDir` on `gltf`.
fn resolve_asset_ref_override(
    world: &World,
    entity_name: &str,
    field: &str,
    live_path: Option<&str>,
    baseline: &Option<bsengine_scene::AssetRef>,
    new: &Option<bsengine_scene::AssetRef>,
) -> Option<bsengine_scene::AssetRef> {
    let resolved_baseline = baseline
        .as_ref()
        .map(|r| bsengine_scene::resolve_asset_ref_for_field(world, entity_name, field, r));
    let unchanged = live_path.map(|s| s.to_string()) == resolved_baseline;
    if unchanged {
        new.as_ref().map(|r| {
            bsengine_scene::AssetRef::Path(bsengine_scene::resolve_asset_ref_for_field(
                world,
                entity_name,
                field,
                r,
            ))
        })
    } else {
        live_path.map(|s| bsengine_scene::AssetRef::Path(s.to_string()))
    }
}

/// Applies `resolve_asset_ref_override` to `gltf`/`script`/`texture` at
/// once, mutating `merged` in place after the generic `merge_entity_descriptor`
/// call -- the same "patch one special field after the fact" shape
/// `resync_instance` already uses for the root transform override, just for
/// three fields instead of one, and (a later task) for every entity rather
/// than only the root.
fn patch_asset_ref_overrides(
    world: &World,
    live: &bsengine_scene::EntityDescriptor,
    baseline: &bsengine_scene::EntityDescriptor,
    new: &bsengine_scene::EntityDescriptor,
    merged: &mut bsengine_scene::EntityDescriptor,
) {
    merged.gltf = resolve_asset_ref_override(
        world,
        &live.name,
        "gltf",
        live.gltf.as_ref().map(|r| r.path()),
        &baseline.gltf,
        &new.gltf,
    );
    merged.script = resolve_asset_ref_override(
        world,
        &live.name,
        "script",
        live.script.as_ref().map(|r| r.path()),
        &baseline.script,
        &new.script,
    );
    merged.texture = resolve_asset_ref_override(
        world,
        &live.name,
        "texture",
        live.texture.as_ref().map(|r| r.path()),
        &baseline.texture,
        &new.texture,
    );
}

/// Merges this PR's representative field set for one matched entity (present
/// in `live`, `baseline`, and `new` alike). `name` always comes from `live`
/// unchanged -- matching is by name already, so there's nothing to resolve
/// there. Covers `transform`/`primitive`/`emissive`/`color`/`opacity`, the
/// physics field group (`rigidbody`/`collider`/`linear_damping`/
/// `angular_damping`), and the light field group: `point_light`/`spot_light`,
/// each resolved attribute-by-attribute via `merge_point_light`/
/// `merge_spot_light` (not the bare `resolve_field`, which would treat the
/// whole descriptor as one atomic value and lose an unrelated attribute's
/// override the moment any single attribute inside it changed), and
/// `directional_light`, resolved via the dedicated `resolve_directional_light`
/// for the same per-attribute reason, plus needing to skip `direction`
/// (`DirectionalLightDescriptor` deliberately doesn't derive `PartialEq` --
/// see that function's doc comment). Also covers the camera field group:
/// `camera`/`camera_fov` are already flat, independent top-level fields
/// (`bool` and `Option<f32>`, both already `Clone + PartialEq`), so the plain
/// `resolve_field` handles each directly -- no dedicated merge function is
/// needed for them, unlike the light cluster. `look_at` is deliberately never
/// resolved at all -- not even via `resolve_field` -- for the same reason
/// `direction` has no live counterpart (see `snapshot_entity_as_descriptor`'s
/// doc comment): it drives a one-time Transform.rotation write at
/// instantiation and is never stored on the live entity afterward, so it
/// stays at `EntityDescriptor::default()`'s `None` via the `..Default::default()`
/// below, same as every other field this function doesn't cover. Every field
/// this PR doesn't yet cover (see the plan's "Scope for this plan" note) is
/// left at `EntityDescriptor::default()`'s value on the returned descriptor;
/// callers must never treat that as "adopt an explicit clear" for those
/// fields -- `apply_merged_descriptor` (a later task) only ever touches the
/// fields this function actually resolves.
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
        point_light: merge_point_light(&live.point_light, &baseline.point_light, &new.point_light),
        spot_light: merge_spot_light(&live.spot_light, &baseline.spot_light, &new.spot_light),
        directional_light: resolve_directional_light(
            &live.directional_light,
            &baseline.directional_light,
            &new.directional_light,
        ),
        camera: resolve_field(&live.camera, &baseline.camera, &new.camera),
        camera_fov: resolve_field(&live.camera_fov, &baseline.camera_fov, &new.camera_fov),
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

    match &merged.point_light {
        Some(pl) => {
            world.entity_mut(entity).insert(bsengine_core::PointLight {
                color: glam::Vec3::from(pl.color).into(),
                intensity: pl.intensity,
                range: pl.range,
            });
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_core::PointLight>();
        }
    }

    match &merged.spot_light {
        Some(sl) => {
            world.entity_mut(entity).insert(bsengine_core::SpotLight {
                color: glam::Vec3::from(sl.color).into(),
                intensity: sl.intensity,
                range: sl.range,
                inner_angle_degrees: sl.inner_angle_degrees.into(),
                outer_angle_degrees: sl.outer_angle_degrees.into(),
            });
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_core::SpotLight>();
        }
    }

    // Deliberately never touches `Transform` -- see `resolve_directional_light`'s
    // and `snapshot_entity_as_descriptor`'s doc comments for why `direction`
    // is excluded entirely; the already-shipped `transform` field group
    // above is the sole owner of rotation, for light entities same as any
    // other.
    match &merged.directional_light {
        Some(dl) => {
            world
                .entity_mut(entity)
                .insert(bsengine_core::DirectionalLight {
                    color: glam::Vec3::from(dl.color).into(),
                    ambient: glam::Vec3::from(dl.ambient).into(),
                });
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_core::DirectionalLight>();
        }
    }

    // `Camera` also carries `aspect_ratio`/`near`/`far`, none of which the
    // wire format tracks -- `aspect_ratio` in particular is rewritten every
    // frame a viewport resize occurs (`bsengine-render/src/plugin.rs`), so a
    // full reconstruction here (mirroring `spawn_scene_entities`) would
    // silently reset it to a hardcoded 16:9 on every resync, visibly
    // distorting the camera until the next resize event. Mutate the
    // existing component's `fov_y_degrees` in place instead -- exactly what
    // `EditorCommand::UpdateCamera` already does for the same field. Only
    // when there's no existing `Camera` to preserve (a brand-new camera) do
    // we construct one fresh, identical to `spawn_scene_entities`'s own
    // construction. `look_at` is deliberately never read here, for the same
    // reason `directional_light`'s `direction` is never read by the block
    // above.
    if merged.camera {
        let fov = merged.camera_fov.unwrap_or(60.0);
        match world.get::<bsengine_core::Camera>(entity) {
            Some(existing) => {
                let mut cam = existing.clone();
                cam.fov_y_degrees = fov.into();
                world.entity_mut(entity).insert(cam);
            }
            None => {
                world
                    .entity_mut(entity)
                    .insert(bsengine_core::Camera::perspective(fov, 16.0 / 9.0));
            }
        }
    } else {
        world.entity_mut(entity).remove::<bsengine_core::Camera>();
    }

    // `merged.gltf`/`script`/`texture` are already fully-resolved plain
    // paths by construction (see `resolve_asset_ref_override`'s doc
    // comment) -- write them directly, never through the resolve pipeline
    // again here.
    match &merged.gltf {
        Some(r) => {
            world
                .entity_mut(entity)
                .insert(bsengine_gltf::GltfAsset::new(r.path().to_string()));
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_gltf::GltfAsset>();
        }
    }
    match &merged.script {
        Some(r) => {
            world
                .entity_mut(entity)
                .insert(bsengine_scene::ScriptPath(r.path().to_string()));
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_scene::ScriptPath>();
        }
    }
    match &merged.texture {
        Some(r) => {
            world
                .entity_mut(entity)
                .insert(bsengine_core::TexturePath(r.path().to_string()));
        }
        None => {
            world
                .entity_mut(entity)
                .remove::<bsengine_core::TexturePath>();
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
    patch_asset_ref_overrides(world, &root_live, baseline_root, new_root, &mut root_merged);
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
                let mut merged = merge_entity_descriptor(&live_desc, b, n);
                patch_asset_ref_overrides(world, &live_desc, b, n, &mut merged);
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

/// Builds the full entity list that should be written back to the source
/// file for `root`'s "Apply to Prefab" -- the write-direction mirror of
/// `resync_instance`'s read-direction resync. Iterates `new.entities` (the
/// file's own current list, in the file's own order) rather than `live`'s
/// entities, since pull sync only ever updates *values* on entities that
/// already exist in the file (see the design spec's "field values only for
/// v1" scope decision) -- it never adds or removes entities.
///
/// For each entity:
/// - The root (matched by name against `new_root`) is written back exactly
///   as the file already has it. Root `Name`/`Transform`/`Parent` are
///   instance-placement data, never prefab-authored data, in either sync
///   direction -- the same exception `resync_instance` already carves out
///   for push-sync, applied here to the write direction.
/// - A nested prefab reference (`entity.prefab.is_some()`) is written back
///   unchanged. `resync_instance`'s own matching loop never calls
///   `merge_entity_descriptor` on these either (see its
///   `b.prefab.is_some() || n.prefab.is_some()` match guard) -- there is no
///   per-field baseline/live comparison for a nested reference's own fields
///   to reuse here, so there is nothing to promote.
/// - An entity with no live counterpart (the user deleted it locally) is
///   written back unchanged -- there's nothing live to diff against or
///   promote.
/// - Otherwise: snapshot the live entity, merge it against baseline/new via
///   the exact same `merge_entity_descriptor`/`patch_asset_ref_overrides`
///   push-sync itself uses, then patch back every field that function
///   doesn't understand (`name`, `parent`, `prefab`, `look_at`) from the
///   file's own current value for this entity. Left unpatched, all four
///   would come out at `EntityDescriptor::default()`'s empty/`None` value
///   -- correct for push-sync (which never reads them off the merged
///   result) but silent, real data loss here, since this result is written
///   verbatim into the file.
///
/// Takes `world: &mut World`, *not* `&World` as originally sketched:
/// `collect_own_descendants` (reused verbatim below, exactly like
/// `resync_instance` already does) itself requires `&mut World`, purely
/// because `World::query` does in this bevy_ecs version (0.14) -- see that
/// function's own doc comment. This function still never writes through
/// `world`; the `&mut` is a borrow-checker artifact of reusing that helper
/// unchanged, not a semantic requirement of anything this function does.
pub(crate) fn build_applied_prefab_entities(
    world: &mut World,
    registry: &TypeRegistry,
    root: Entity,
    baseline_root: &bsengine_scene::EntityDescriptor,
    new_root: &bsengine_scene::EntityDescriptor,
    baseline: &bsengine_scene::types::PrefabDescriptor,
    new: &bsengine_scene::types::PrefabDescriptor,
) -> Vec<bsengine_scene::EntityDescriptor> {
    let own_descendants = collect_own_descendants(world, root);
    let suffix = instance_suffix(world, &own_descendants);

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

    new.entities
        .iter()
        .map(|new_entity| {
            if new_entity.name == new_root.name {
                return new_entity.clone();
            }
            if new_entity.prefab.is_some() {
                return new_entity.clone();
            }
            let Some(&live_entity) = resolved_by_raw_name.get(new_entity.name.as_str()) else {
                return new_entity.clone();
            };
            let Some(&baseline_entity) = baseline_by_name.get(new_entity.name.as_str()) else {
                return new_entity.clone();
            };

            let live_desc = snapshot_entity_as_descriptor(world, registry, live_entity);
            let mut merged = merge_entity_descriptor(&live_desc, baseline_entity, new_entity);
            patch_asset_ref_overrides(world, &live_desc, baseline_entity, new_entity, &mut merged);
            merged.name = new_entity.name.clone();
            merged.parent = new_entity.parent.clone();
            merged.prefab = new_entity.prefab.clone();
            merged.look_at = new_entity.look_at;
            merged
        })
        .collect()
}

/// The `&mut World` entry point for "Apply to Prefab": reads `root`'s
/// `PrefabInstance`/`PrefabInstanceBaseline`, re-parses the current source
/// file, builds the entity list via `build_applied_prefab_entities`, and
/// overwrites the file. Reuses the exact same missing/unreadable/unparseable/
/// structurally-invalid file guards `resync_prefab_instances`
/// (`prefab_watcher.rs`) already has -- see that function for why each one
/// exists.
///
/// One deliberate asymmetry from `resync_prefab_instances`: a missing or
/// corrupt baseline there falls back to "change nothing," since it's an
/// automatic background process where silently doing nothing is safe. This
/// is a deliberate, user-initiated write to a file every other instance
/// depends on -- with no baseline to compute overrides against, guessing
/// risks writing something wrong into shared state, so this refuses with an
/// error instead.
///
/// Deliberately does not update `root`'s own `PrefabInstanceBaseline`, does
/// not touch any live entity, and does not trigger a resync directly --
/// `PrefabWatcherPlugin`'s existing file-watch mechanism picks up the write
/// and handles all of that, for every instance of this file, automatically.
pub(crate) fn apply_instance_to_prefab(world: &mut World, root: Entity) -> Result<(), String> {
    let Some(instance) = world.get::<bsengine_core::PrefabInstance>(root) else {
        return Err(format!("entity {root:?} is not a prefab instance root"));
    };
    let source_path = instance.source_path.clone();

    let Some(baseline_ron) = world
        .get::<bsengine_core::PrefabInstanceBaseline>(root)
        .map(|b| b.synced_ron.clone())
    else {
        return Err(format!(
            "'{source_path}' has no recorded baseline for this instance; cannot determine \
             overrides -- resync it against the current file at least once before applying"
        ));
    };
    let baseline: bsengine_scene::types::PrefabDescriptor = ron::from_str(&baseline_ron)
        .map_err(|e| format!("'{source_path}' instance baseline failed to parse: {e}"))?;

    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();
    let resolved_path = bsengine_core::resolve_project_path(project_dir.as_ref(), &source_path);
    if !std::path::Path::new(&resolved_path).is_file() {
        return Err(format!("'{resolved_path}' no longer exists on disk"));
    }
    let content = std::fs::read_to_string(&resolved_path)
        .map_err(|e| format!("'{resolved_path}' could not be read: {e}"))?;
    let new: bsengine_scene::types::PrefabDescriptor =
        ron::from_str(&content).map_err(|e| format!("'{resolved_path}' failed to parse: {e}"))?;
    let new_root = bsengine_scene::validate_prefab_descriptor(&new)
        .map_err(|e| format!("'{resolved_path}' is not a valid instantiable prefab: {e}"))?
        .clone();
    let baseline_root = bsengine_scene::validate_prefab_descriptor(&baseline)
        .map_err(|e| format!("instance baseline is not a valid instantiable prefab: {e}"))?
        .clone();

    let registry = world
        .resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .clone();
    let registry = registry.read();
    let applied_entities = build_applied_prefab_entities(
        world,
        &registry,
        root,
        &baseline_root,
        &new_root,
        &baseline,
        &new,
    );
    drop(registry);

    let applied = bsengine_scene::types::PrefabDescriptor {
        entities: applied_entities,
    };
    let ron_str = ron::to_string(&applied).map_err(|e| format!("serialize failed: {e}"))?;
    std::fs::write(&resolved_path, ron_str).map_err(|e| format!("write failed: {e}"))?;
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
    fn snapshot_captures_a_live_point_light() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Lamp".to_string()),
                bsengine_core::PointLight {
                    color: glam::Vec3::new(1.0, 0.5, 0.25).into(),
                    intensity: 2.0,
                    range: 15.0,
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(
            desc.point_light,
            Some(bsengine_scene::PointLightDescriptor {
                color: [1.0, 0.5, 0.25],
                intensity: 2.0,
                range: 15.0,
            })
        );
        assert_eq!(desc.spot_light, None);
        // `DirectionalLightDescriptor` doesn't derive `PartialEq` (only
        // `PointLightDescriptor`/`SpotLightDescriptor` gained it, see commit
        // 04aa90e3), so `Option<DirectionalLightDescriptor>` can't be
        // compared with `assert_eq!` -- use `.is_none()` instead.
        assert!(desc.directional_light.is_none());
    }

    #[test]
    fn snapshot_captures_a_live_spot_light() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Spot".to_string()),
                bsengine_core::SpotLight {
                    color: glam::Vec3::ONE.into(),
                    intensity: 1.5,
                    range: 8.0,
                    inner_angle_degrees: 20.0.into(),
                    outer_angle_degrees: 35.0.into(),
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(
            desc.spot_light,
            Some(bsengine_scene::SpotLightDescriptor {
                color: [1.0, 1.0, 1.0],
                intensity: 1.5,
                range: 8.0,
                inner_angle_degrees: 20.0,
                outer_angle_degrees: 35.0,
            })
        );
    }

    #[test]
    fn snapshot_captures_a_live_directional_lights_color_and_ambient_but_not_direction() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Sun".to_string()),
                bsengine_core::DirectionalLight {
                    color: glam::Vec3::new(1.0, 0.9, 0.8).into(),
                    ambient: glam::Vec3::splat(0.1).into(),
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        let dl = desc
            .directional_light
            .expect("directional_light must be captured");
        assert_eq!(dl.color, [1.0, 0.9, 0.8]);
        assert_eq!(dl.ambient, [0.1, 0.1, 0.1]);
        // Deliberately no assertion on `dl.direction` -- it's a fixed, unused
        // placeholder (see the design spec's "direction is not tracked"
        // decision), not a value with a meaningful "correct" answer here.
    }

    #[test]
    fn snapshot_omits_light_fields_when_no_lights_present() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Dark".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(desc.point_light, None);
        assert_eq!(desc.spot_light, None);
        // See the comment in `snapshot_captures_a_live_point_light`:
        // `DirectionalLightDescriptor` has no `PartialEq`.
        assert!(desc.directional_light.is_none());
    }

    #[test]
    fn snapshot_captures_a_live_camera() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("MainCam".to_string()),
                bsengine_core::Camera::perspective(75.0, 16.0 / 9.0),
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert!(desc.camera);
        assert_eq!(desc.camera_fov, Some(75.0));
    }

    #[test]
    fn snapshot_omits_camera_fields_when_no_camera_present() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("NotACamera".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert!(!desc.camera);
        assert_eq!(desc.camera_fov, None);
    }

    #[test]
    fn snapshot_captures_a_live_gltf_script_and_texture_reference() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Prop".to_string()),
                bsengine_gltf::GltfAsset::new("assets/models/prop.glb"),
                bsengine_scene::ScriptPath("assets/scripts/prop.js".to_string()),
                bsengine_core::TexturePath("assets/textures/prop.png".to_string()),
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(
            desc.gltf,
            Some(bsengine_scene::AssetRef::Path(
                "assets/models/prop.glb".to_string()
            ))
        );
        assert_eq!(
            desc.script,
            Some(bsengine_scene::AssetRef::Path(
                "assets/scripts/prop.js".to_string()
            ))
        );
        assert_eq!(
            desc.texture,
            Some(bsengine_scene::AssetRef::Path(
                "assets/textures/prop.png".to_string()
            ))
        );
    }

    #[test]
    fn snapshot_omits_asset_ref_fields_when_absent() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Bare".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let desc = snapshot_entity_as_descriptor(app.world(), &reg, entity);

        assert_eq!(desc.gltf, None);
        assert_eq!(desc.script, None);
        assert_eq!(desc.texture, None);
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
    fn merge_entity_descriptor_resolves_camera_and_camera_fov_independently() {
        let baseline = bsengine_scene::EntityDescriptor {
            name: "Cam".to_string(),
            camera: true,
            camera_fov: Some(60.0),
            ..Default::default()
        };
        let new = bsengine_scene::EntityDescriptor {
            name: "Cam".to_string(),
            camera: false, // author removed the camera, unoverridden
            camera_fov: Some(60.0),
            ..Default::default()
        };
        let live = bsengine_scene::EntityDescriptor {
            name: "Cam".to_string(),
            camera: true,           // matches baseline -> adopt new (false)
            camera_fov: Some(90.0), // user widened fov -> override
            ..Default::default()
        };

        let merged = merge_entity_descriptor(&live, &baseline, &new);

        assert!(
            !merged.camera,
            "unoverridden camera presence must adopt the new file's value"
        );
        assert_eq!(
            merged.camera_fov,
            Some(90.0),
            "the user's fov override must survive independently of the presence change"
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
    fn merge_entity_descriptor_preserves_an_overridden_point_light_field() {
        let baseline = bsengine_scene::EntityDescriptor {
            name: "Lamp".to_string(),
            point_light: Some(bsengine_scene::PointLightDescriptor {
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                range: 10.0,
            }),
            ..Default::default()
        };
        let new = bsengine_scene::EntityDescriptor {
            name: "Lamp".to_string(),
            point_light: Some(bsengine_scene::PointLightDescriptor {
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                range: 25.0, // author widened the range, unoverridden
            }),
            ..Default::default()
        };
        let live = bsengine_scene::EntityDescriptor {
            name: "Lamp".to_string(),
            point_light: Some(bsengine_scene::PointLightDescriptor {
                color: [1.0, 1.0, 1.0],
                intensity: 3.0, // user tuned intensity -> override
                range: 10.0,    // matches baseline -> adopt new
            }),
            ..Default::default()
        };

        let merged = merge_entity_descriptor(&live, &baseline, &new);

        let pl = merged.point_light.unwrap();
        assert_eq!(
            pl.intensity, 3.0,
            "the user's intensity override must survive"
        );
        assert_eq!(
            pl.range, 25.0,
            "the unoverridden range must adopt the new file's value"
        );
    }

    #[test]
    fn resolve_directional_light_compares_only_color_and_ambient() {
        let baseline = Some(bsengine_scene::DirectionalLightDescriptor {
            direction: [0.0, 0.0, -1.0],
            color: [1.0, 1.0, 1.0],
            ambient: [0.1, 0.1, 0.1],
        });
        let new = Some(bsengine_scene::DirectionalLightDescriptor {
            direction: [1.0, 0.0, 0.0], // changed, but must have zero effect on the decision
            color: [1.0, 1.0, 1.0],     // unchanged -> adopt
            ambient: [0.2, 0.2, 0.2],   // unchanged... wait see below
        });
        // live's color diverges from baseline -> override; ambient matches baseline -> adopt.
        let live = Some(bsengine_scene::DirectionalLightDescriptor {
            direction: [0.0, 0.0, -1.0],
            color: [0.5, 0.0, 0.0],   // user recolored the sun -> override
            ambient: [0.1, 0.1, 0.1], // matches baseline -> adopt new's ambient
        });

        let resolved = resolve_directional_light(&live, &baseline, &new).unwrap();

        assert_eq!(
            resolved.color,
            [0.5, 0.0, 0.0],
            "overridden color must survive"
        );
        assert_eq!(
            resolved.ambient,
            [0.2, 0.2, 0.2],
            "unoverridden ambient must adopt the new value"
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
    fn apply_merged_descriptor_inserts_and_removes_a_point_light() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Lamp".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            point_light: Some(bsengine_scene::PointLightDescriptor {
                color: [1.0, 0.0, 0.0],
                intensity: 2.0,
                range: 12.0,
            }),
            ..live.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);
        let pl = app
            .world()
            .get::<bsengine_core::PointLight>(entity)
            .unwrap();
        assert_eq!(pl.color.0.to_array(), [1.0, 0.0, 0.0]);
        assert_eq!(pl.intensity, 2.0);
        assert_eq!(pl.range, 12.0);

        // Now resolve away and confirm removal.
        let live2 = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged2 = bsengine_scene::EntityDescriptor {
            point_light: None,
            ..live2.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live2, &merged2);
        assert!(app
            .world()
            .get::<bsengine_core::PointLight>(entity)
            .is_none());
    }

    #[test]
    fn apply_merged_descriptor_inserts_and_removes_a_spot_light() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Spot".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            spot_light: Some(bsengine_scene::SpotLightDescriptor {
                color: [1.0, 1.0, 1.0],
                intensity: 1.0,
                range: 10.0,
                inner_angle_degrees: 15.0,
                outer_angle_degrees: 25.0,
            }),
            ..live.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);
        let sl = app.world().get::<bsengine_core::SpotLight>(entity).unwrap();
        assert_eq!(sl.inner_angle_degrees.0, 15.0);
        assert_eq!(sl.outer_angle_degrees.0, 25.0);
    }

    #[test]
    fn apply_merged_descriptor_writes_directional_light_color_and_ambient_only() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Sun".to_string()),
                bsengine_core::Transform {
                    rotation: glam::Quat::from_rotation_x(0.3).into(),
                    ..Default::default()
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            directional_light: Some(bsengine_scene::DirectionalLightDescriptor {
                direction: [1.0, 0.0, 0.0], // must have no effect on Transform
                color: [1.0, 0.8, 0.6],
                ambient: [0.2, 0.2, 0.2],
            }),
            transform: live.transform.clone(), // keep whatever transform snapshot already had
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        let dl = app
            .world()
            .get::<bsengine_core::DirectionalLight>(entity)
            .unwrap();
        assert_eq!(dl.color.0.to_array(), [1.0, 0.8, 0.6]);
        assert_eq!(dl.ambient.0.to_array(), [0.2, 0.2, 0.2]);
        let rotation_after = app
            .world()
            .get::<bsengine_core::Transform>(entity)
            .unwrap()
            .rotation
            .0;
        let rotation_before = glam::Quat::from_rotation_x(0.3);
        assert!(
            rotation_after.abs_diff_eq(rotation_before, 1e-5),
            "directional_light must never write Transform.rotation -- got {rotation_after:?}, \
             expected it unchanged at {rotation_before:?}"
        );
    }

    #[test]
    fn apply_merged_descriptor_preserves_aspect_ratio_and_clip_planes_when_updating_fov() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn((
                bsengine_scene::Name("Cam".to_string()),
                bsengine_core::Camera {
                    fov_y_degrees: 60.0.into(),
                    aspect_ratio: 2.35, // an unusual value only a real resize event would produce
                    near: 0.05,
                    far: 500.0,
                },
            ))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            camera: true,
            camera_fov: Some(90.0),
            ..live.clone()
        };

        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        let cam = app.world().get::<bsengine_core::Camera>(entity).unwrap();
        assert_eq!(cam.fov_y_degrees.0, 90.0, "fov must update");
        assert_eq!(
            cam.aspect_ratio, 2.35,
            "aspect_ratio must survive untouched -- it's driven by the live viewport-resize system, \
             never by prefab override tracking"
        );
        assert_eq!(cam.near, 0.05, "near must survive untouched");
        assert_eq!(cam.far, 500.0, "far must survive untouched");
    }

    #[test]
    fn apply_merged_descriptor_inserts_and_removes_a_camera() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("NotYetACamera".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            camera: true,
            camera_fov: None, // absent -> defaults to 60, matching spawn_scene_entities
            ..live.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);
        let cam = app.world().get::<bsengine_core::Camera>(entity).unwrap();
        assert_eq!(cam.fov_y_degrees.0, 60.0);

        // Now resolve away and confirm removal.
        let live2 = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged2 = bsengine_scene::EntityDescriptor {
            camera: false,
            ..live2.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live2, &merged2);
        assert!(app.world().get::<bsengine_core::Camera>(entity).is_none());
    }

    #[test]
    fn apply_merged_descriptor_inserts_and_removes_a_gltf_script_and_texture_reference() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let entity = app
            .world_mut()
            .spawn(bsengine_scene::Name("Prop".to_string()))
            .id();

        let reg = registry(app.world());
        let reg = reg.read();
        let live = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged = bsengine_scene::EntityDescriptor {
            gltf: Some(bsengine_scene::AssetRef::Path(
                "assets/models/prop.glb".to_string(),
            )),
            script: Some(bsengine_scene::AssetRef::Path(
                "assets/scripts/prop.js".to_string(),
            )),
            texture: Some(bsengine_scene::AssetRef::Path(
                "assets/textures/prop.png".to_string(),
            )),
            ..live.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live, &merged);

        assert_eq!(
            app.world()
                .get::<bsengine_gltf::GltfAsset>(entity)
                .unwrap()
                .path,
            "assets/models/prop.glb"
        );
        assert_eq!(
            app.world()
                .get::<bsengine_scene::ScriptPath>(entity)
                .unwrap()
                .0,
            "assets/scripts/prop.js"
        );
        assert_eq!(
            app.world()
                .get::<bsengine_core::TexturePath>(entity)
                .unwrap()
                .0,
            "assets/textures/prop.png"
        );

        // Now resolve away and confirm removal.
        let live2 = snapshot_entity_as_descriptor(app.world(), &reg, entity);
        let merged2 = bsengine_scene::EntityDescriptor {
            gltf: None,
            script: None,
            texture: None,
            ..live2.clone()
        };
        apply_merged_descriptor(app.world_mut(), entity, &reg, &live2, &merged2);
        assert!(app
            .world()
            .get::<bsengine_gltf::GltfAsset>(entity)
            .is_none());
        assert!(app
            .world()
            .get::<bsengine_scene::ScriptPath>(entity)
            .is_none());
        assert!(app
            .world()
            .get::<bsengine_core::TexturePath>(entity)
            .is_none());
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
    fn resync_instance_preserves_an_overridden_physics_field_while_updating_a_sibling_field() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Crate",
                    parent: Some("Body"),
                    primitive: Some(Cube),
                    rigidbody: Some(Dynamic),
                    collider: Some((shape: Sphere(radius: 1.0), restitution: 0.1, friction: 0.5, sensor: false)),
                    linear_damping: Some(0.2),
                    angular_damping: Some(0.1),
                ),
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

        // User manually tunes angular_damping and never touches the collider shape.
        let crate_entity = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Crate#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        {
            let mut body = app
                .world_mut()
                .get_mut::<bsengine_scene::PhysicsBodyDesc>(crate_entity)
                .unwrap();
            body.angular_damping = Some(0.9);
        }

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Crate",
                    parent: Some("Body"),
                    primitive: Some(Cube),
                    rigidbody: Some(Dynamic),
                    collider: Some((shape: Sphere(radius: 2.0), restitution: 0.1, friction: 0.5, sensor: false)),
                    linear_damping: Some(0.2),
                    angular_damping: Some(0.1),
                ),
            ])"#,
        );

        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let body = app
            .world()
            .get::<bsengine_scene::PhysicsBodyDesc>(crate_entity)
            .unwrap();
        assert_eq!(
            body.angular_damping,
            Some(0.9),
            "the user's angular_damping override must survive the resync"
        );
        assert_eq!(
            body.collider.shape,
            bsengine_scene::ColliderShapeDesc::Sphere { radius: 2.0 },
            "the unoverridden collider shape must still update to the new file's value"
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

    #[test]
    fn resync_instance_preserves_an_overridden_point_light_field_while_updating_a_sibling_field() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Lamp",
                    parent: Some("Body"),
                    point_light: Some((color: (1.0, 1.0, 1.0), intensity: 1.0, range: 10.0)),
                ),
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

        let lamp = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
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
            pl.intensity = 5.0; // user override
        }

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Lamp",
                    parent: Some("Body"),
                    point_light: Some((color: (1.0, 1.0, 1.0), intensity: 1.0, range: 30.0)),
                ),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let pl = app.world().get::<bsengine_core::PointLight>(lamp).unwrap();
        assert_eq!(pl.intensity, 5.0, "overridden intensity must survive");
        assert_eq!(
            pl.range, 30.0,
            "unoverridden range must adopt the new file's value"
        );
    }

    #[test]
    fn resync_instance_preserves_an_overridden_spot_light_field_while_updating_a_sibling_field() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Spot",
                    parent: Some("Body"),
                    spot_light: Some((
                        color: (1.0, 1.0, 1.0),
                        intensity: 1.0,
                        range: 10.0,
                        inner_angle_degrees: 20.0,
                        outer_angle_degrees: 35.0,
                    )),
                ),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/searchlight.ron",
            Some("MySearchlight"),
            None,
            None,
        )
        .unwrap();

        let spot = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Spot#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        {
            let mut sl = app
                .world_mut()
                .get_mut::<bsengine_core::SpotLight>(spot)
                .unwrap();
            sl.outer_angle_degrees = 60.0.into(); // user override
        }

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Spot",
                    parent: Some("Body"),
                    spot_light: Some((
                        color: (1.0, 1.0, 1.0),
                        intensity: 1.0,
                        range: 40.0,
                        inner_angle_degrees: 20.0,
                        outer_angle_degrees: 35.0,
                    )),
                ),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/searchlight.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let sl = app.world().get::<bsengine_core::SpotLight>(spot).unwrap();
        assert_eq!(
            sl.outer_angle_degrees.0, 60.0,
            "overridden outer_angle_degrees must survive"
        );
        assert_eq!(
            sl.range, 40.0,
            "unoverridden range must adopt the new file's value"
        );
    }

    #[test]
    fn resync_instance_never_lets_a_directional_lights_direction_change_touch_transform() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(
                    name: "Sun",
                    directional_light: Some((direction: (0.0, -1.0, 0.0), color: (1.0, 1.0, 1.0), ambient: (0.1, 0.1, 0.1))),
                ),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/sun.ron",
            Some("MySun"),
            None,
            None,
        )
        .unwrap();

        // User manually rotates the root -- an overridden transform (root transform
        // is always preserved regardless of override status, per PR #1789, but
        // this test is specifically about proving directional_light's `direction`
        // never gets a chance to fight that guarantee).
        let manual_rotation = glam::Quat::from_rotation_y(1.2);
        app.world_mut()
            .entity_mut(root)
            .insert(bsengine_core::Transform {
                rotation: manual_rotation.into(),
                ..Default::default()
            });

        // Source changes `direction:` -- if the old spawn-time side effect were
        // reproduced here, this would silently re-point the light and undo the
        // user's manual rotation.
        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(
                    name: "Sun",
                    directional_light: Some((direction: (1.0, 0.0, 0.0), color: (1.0, 1.0, 1.0), ambient: (0.1, 0.1, 0.1))),
                ),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/sun.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let rotation_after = app
            .world()
            .get::<bsengine_core::Transform>(root)
            .unwrap()
            .rotation
            .0;
        assert!(
            rotation_after.abs_diff_eq(manual_rotation, 1e-5),
            "the user's manual rotation must survive a resync that changes direction:, proving \
             the exact regression this design exists to avoid does not happen -- got \
             {rotation_after:?}, expected {manual_rotation:?}"
        );
    }

    #[test]
    fn resync_instance_preserves_an_overridden_camera_fov_while_updating_a_sibling_field() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(
                    name: "Cam",
                    camera: true,
                    camera_fov: Some(60.0),
                ),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/turret_cam.ron",
            Some("MyTurretCam"),
            None,
            None,
        )
        .unwrap();

        {
            let mut cam = app
                .world_mut()
                .get_mut::<bsengine_core::Camera>(root)
                .unwrap();
            cam.fov_y_degrees = 90.0.into(); // user override
        }

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(
                    name: "Cam",
                    camera: true,
                    camera_fov: Some(60.0),
                ),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/turret_cam.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let cam = app.world().get::<bsengine_core::Camera>(root).unwrap();
        assert_eq!(cam.fov_y_degrees.0, 90.0, "overridden fov must survive");
    }

    #[test]
    fn resync_instance_never_lets_a_look_at_change_touch_transform() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(
                    name: "Cam",
                    camera: true,
                    transform: Some((position: (0.0, 0.0, 5.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0))),
                    look_at: Some((0.0, 0.0, 0.0)),
                ),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/aimed_cam.ron",
            Some("MyAimedCam"),
            None,
            None,
        )
        .unwrap();

        // User manually rotates the root -- an overridden transform (root
        // transform is always preserved regardless of override status, per PR
        // #1789, but this test is specifically about proving `look_at` never
        // gets a chance to fight that guarantee).
        let manual_rotation = glam::Quat::from_rotation_y(0.9);
        app.world_mut()
            .entity_mut(root)
            .insert(bsengine_core::Transform {
                position: glam::Vec3::new(0.0, 0.0, 5.0).into(),
                rotation: manual_rotation.into(),
                scale: glam::Vec3::ONE.into(),
            });

        // Source changes `look_at:` -- if the old spawn-time side effect were
        // reproduced here, this would silently re-aim the camera and undo the
        // user's manual rotation.
        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(
                    name: "Cam",
                    camera: true,
                    transform: Some((position: (0.0, 0.0, 5.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0))),
                    look_at: Some((10.0, 0.0, 0.0)),
                ),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/aimed_cam.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let rotation_after = app
            .world()
            .get::<bsengine_core::Transform>(root)
            .unwrap()
            .rotation
            .0;
        assert!(
            rotation_after.abs_diff_eq(manual_rotation, 1e-5),
            "the user's manual rotation must survive a resync that changes look_at:, proving the exact \
             regression this design exists to avoid does not happen -- got {rotation_after:?}, \
             expected {manual_rotation:?}"
        );
    }

    #[test]
    fn resolve_asset_ref_override_preserves_an_override_and_adopts_an_unrelated_change() {
        let app = new_app();
        let world = app.world();
        let baseline = Some(bsengine_scene::AssetRef::Path(
            "assets/models/a.glb".to_string(),
        ));
        let new = Some(bsengine_scene::AssetRef::Path(
            "assets/models/b.glb".to_string(),
        ));

        // live diverges from (resolved) baseline -> override, keep live's path.
        let overridden = resolve_asset_ref_override(
            world,
            "Thing",
            "gltf",
            Some("assets/models/custom.glb"),
            &baseline,
            &new,
        );
        assert_eq!(
            overridden,
            Some(bsengine_scene::AssetRef::Path(
                "assets/models/custom.glb".to_string()
            )),
            "an overridden gltf path must survive"
        );

        // live matches (resolved) baseline -> adopt new.
        let adopted = resolve_asset_ref_override(
            world,
            "Thing",
            "gltf",
            Some("assets/models/a.glb"),
            &baseline,
            &new,
        );
        assert_eq!(
            adopted,
            Some(bsengine_scene::AssetRef::Path(
                "assets/models/b.glb".to_string()
            )),
            "an unoverridden gltf must adopt the new file's value"
        );
    }

    #[test]
    fn resync_instance_preserves_an_overridden_texture_reference_while_updating_a_sibling_field() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Floor",
                    parent: Some("Body"),
                    texture: Some("assets/textures/checker.png"),
                    emissive: Some((0.0, 0.0, 0.0)),
                ),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/floor_prop.ron",
            Some("MyFloorProp"),
            None,
            None,
        )
        .unwrap();

        let floor = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Floor#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        {
            let mut tp = app
                .world_mut()
                .get_mut::<bsengine_core::TexturePath>(floor)
                .unwrap();
            tp.0 = "assets/textures/custom.png".to_string(); // user override
        }

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Floor",
                    parent: Some("Body"),
                    texture: Some("assets/textures/checker.png"),
                    emissive: Some((0.2, 0.2, 0.2)),
                ),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/floor_prop.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        let tp = app
            .world()
            .get::<bsengine_core::TexturePath>(floor)
            .unwrap();
        assert_eq!(
            tp.0, "assets/textures/custom.png",
            "overridden texture reference must survive"
        );
        let material = app.world().get::<bsengine_core::Material>(floor).unwrap();
        assert_eq!(
            material.emissive.0.to_array(),
            [0.2, 0.2, 0.2],
            "unoverridden sibling field must still adopt the new file's value"
        );
    }

    #[test]
    fn resync_instance_lets_an_unoverridden_gltf_reference_adopt_a_source_change_under_a_real_project_dir(
    ) {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        app.world_mut()
            .insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Prop", gltf: Some("assets/models/a.glb")),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/prop.ron",
            Some("MyProp"),
            None,
            None,
        )
        .unwrap();

        // Confirm the live path really is ProjectDir-prefixed after
        // instantiation -- if this assertion fails, the rest of the test
        // proves nothing about the bug this design exists to prevent.
        assert_eq!(
            app.world()
                .get::<bsengine_gltf::GltfAsset>(root)
                .unwrap()
                .path,
            "games/demo/assets/models/a.glb"
        );

        // Source changes the gltf reference, unoverridden. If the naive
        // raw-equality bug this design exists to prevent were still present,
        // `live` ("games/demo/assets/models/a.glb") would never match
        // `baseline`'s raw, unresolved value ("assets/models/a.glb"), so this
        // reference would look permanently overridden and never adopt the
        // change below.
        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Prop", gltf: Some("assets/models/b.glb")),
            ])"#,
        );
        let own_source_paths =
            std::collections::HashSet::from(["assets/prefabs/prop.ron".to_string()]);
        resync_instance(app.world_mut(), root, &baseline, &new, &own_source_paths).unwrap();

        assert_eq!(
            app.world().get::<bsengine_gltf::GltfAsset>(root).unwrap().path,
            "games/demo/assets/models/b.glb",
            "an unoverridden gltf reference must adopt the source file's new value, correctly \
             re-resolved with the ProjectDir prefix -- proving the resolve-then-compare fix actually \
             works, not just that overrides survive"
        );
    }

    #[test]
    fn build_applied_prefab_entities_promotes_an_override_and_leaves_an_unoverridden_field_unchanged(
    ) {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Lamp",
                    parent: Some("Root"),
                    point_light: Some((color: (1.0, 1.0, 1.0), intensity: 1.0, range: 10.0)),
                ),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/lamp_holder.ron",
            Some("MyLampHolder"),
            None,
            None,
        )
        .unwrap();

        let lamp = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
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
            pl.intensity = 5.0; // user override
        }

        // The file itself has since moved on for an unrelated field, unsynced by this instance yet.
        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Lamp",
                    parent: Some("Root"),
                    point_light: Some((color: (1.0, 1.0, 1.0), intensity: 1.0, range: 30.0)),
                ),
            ])"#,
        );

        let registry = registry(app.world());
        let registry = registry.read();
        let baseline_root = bsengine_scene::validate_prefab_descriptor(&baseline).unwrap();
        let new_root = bsengine_scene::validate_prefab_descriptor(&new).unwrap();
        let applied = build_applied_prefab_entities(
            app.world_mut(),
            &registry,
            root,
            baseline_root,
            new_root,
            &baseline,
            &new,
        );

        let lamp_out = applied.iter().find(|e| e.name == "Lamp").unwrap();
        let pl = lamp_out.point_light.as_ref().unwrap();
        assert_eq!(
            pl.intensity, 5.0,
            "the user's override must be promoted into the written entity"
        );
        assert_eq!(
            pl.range, 30.0,
            "the unoverridden range must keep the file's current value, not be overwritten by \
             anything from the (out of date) baseline"
        );
    }

    #[test]
    fn build_applied_prefab_entities_never_wipes_parent_or_name_even_when_unrelated_fields_change()
    {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
                EntityDescriptor(name: "Child", parent: Some("Root"), emissive: Some((0.0, 0.0, 0.0))),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/parented.ron",
            Some("MyParented"),
            None,
            None,
        )
        .unwrap();

        let new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
                EntityDescriptor(name: "Child", parent: Some("Root"), emissive: Some((0.5, 0.5, 0.5))),
            ])"#,
        );

        let registry = registry(app.world());
        let registry = registry.read();
        let baseline_root = bsengine_scene::validate_prefab_descriptor(&baseline).unwrap();
        let new_root = bsengine_scene::validate_prefab_descriptor(&new).unwrap();
        let applied = build_applied_prefab_entities(
            app.world_mut(),
            &registry,
            root,
            baseline_root,
            new_root,
            &baseline,
            &new,
        );

        let child_out = applied.iter().find(|e| e.name == "Child").unwrap();
        assert_eq!(
            child_out.name, "Child",
            "the file's raw unsuffixed name must survive, not the live instance's '#N'-suffixed name"
        );
        assert_eq!(
            child_out.parent.as_deref(),
            Some("Root"),
            "parent must survive -- merge_entity_descriptor doesn't understand this field at all"
        );
    }

    #[test]
    fn build_applied_prefab_entities_never_writes_a_change_to_the_root_or_a_nested_reference() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/simple.ron",
            Some("MySimple"),
            None,
            None,
        )
        .unwrap();

        // User manually rotates the root live -- must never leak into the file.
        app.world_mut()
            .entity_mut(root)
            .insert(bsengine_core::Transform {
                rotation: glam::Quat::from_rotation_y(1.0).into(),
                ..Default::default()
            });

        let new = baseline.clone();

        let registry = registry(app.world());
        let registry = registry.read();
        let baseline_root = bsengine_scene::validate_prefab_descriptor(&baseline).unwrap();
        let new_root = bsengine_scene::validate_prefab_descriptor(&new).unwrap();
        let applied = build_applied_prefab_entities(
            app.world_mut(),
            &registry,
            root,
            baseline_root,
            new_root,
            &baseline,
            &new,
        );

        let root_out = applied.iter().find(|e| e.name == "Root").unwrap();
        assert_eq!(
            root_out.transform, new_root.transform,
            "the root's transform must never be written, regardless of live override status"
        );
    }

    #[test]
    fn build_applied_prefab_entities_leaves_a_locally_deleted_entity_untouched_in_the_file() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let baseline = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
                EntityDescriptor(name: "Doomed", parent: Some("Root"), emissive: Some((0.0, 0.0, 0.0))),
            ])"#,
        );
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            "assets/prefabs/deletable.ron",
            Some("MyDeletable"),
            None,
            None,
        )
        .unwrap();

        let doomed = {
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
            q.iter(app.world())
                .find(|(_, n)| n.0.starts_with("Doomed#"))
                .map(|(e, _)| e)
                .unwrap()
        };
        app.world_mut().despawn(doomed);

        let new = baseline.clone();

        let registry = registry(app.world());
        let registry = registry.read();
        let baseline_root = bsengine_scene::validate_prefab_descriptor(&baseline).unwrap();
        let new_root = bsengine_scene::validate_prefab_descriptor(&new).unwrap();
        let applied = build_applied_prefab_entities(
            app.world_mut(),
            &registry,
            root,
            baseline_root,
            new_root,
            &baseline,
            &new,
        );

        let doomed_out = applied.iter().find(|e| e.name == "Doomed").unwrap();
        let expected = new.entities.iter().find(|e| e.name == "Doomed").unwrap();
        // `EntityDescriptor` doesn't derive `PartialEq` (its `directional_light:
        // Option<DirectionalLightDescriptor>` field can't -- see
        // `snapshot_captures_a_live_point_light`'s comment above), so compare
        // the fields this prefab actually sets instead of the whole struct.
        assert_eq!(
            doomed_out.name, expected.name,
            "an entity the user deleted locally must be written back byte-for-byte unchanged"
        );
        assert_eq!(
            doomed_out.parent, expected.parent,
            "an entity the user deleted locally must be written back byte-for-byte unchanged"
        );
        assert_eq!(
            doomed_out.emissive, expected.emissive,
            "an entity the user deleted locally must be written back byte-for-byte unchanged"
        );
    }

    #[test]
    fn apply_instance_to_prefab_writes_a_promoted_override_to_the_real_file() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assets/prefabs/lamp.ron");
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

        let baseline = parse(source);
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            path.to_str().unwrap(),
            Some("MyLamp"),
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
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
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
            pl.intensity = 5.0; // user override
        }

        apply_instance_to_prefab(app.world_mut(), root).unwrap();

        let written = std::fs::read_to_string(&path).unwrap();
        let parsed: bsengine_scene::types::PrefabDescriptor = ron::from_str(&written).unwrap();
        let lamp_out = parsed.entities.iter().find(|e| e.name == "Lamp").unwrap();
        assert_eq!(
            lamp_out.point_light.as_ref().unwrap().intensity,
            5.0,
            "the override must be written into the real file on disk"
        );
    }

    #[test]
    fn applying_and_then_resyncing_makes_the_promoted_field_stop_being_an_override() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assets/prefabs/roundtrip.ron");
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

        let baseline = parse(source);
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            path.to_str().unwrap(),
            Some("MyRoundtrip"),
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
            let mut q = app.world_mut().query::<(Entity, &bsengine_scene::Name)>();
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
            pl.intensity = 5.0; // user override
        }

        // Apply: writes the override into the file.
        apply_instance_to_prefab(app.world_mut(), root).unwrap();
        let applied_content = std::fs::read_to_string(&path).unwrap();
        let applied_prefab: bsengine_scene::types::PrefabDescriptor =
            ron::from_str(&applied_content).unwrap();

        // Simulate what PrefabWatcherPlugin's file-watch would do next: resync
        // this instance against its (pre-apply) baseline and the newly-applied
        // file content, exactly as `resync_prefab_instances` does.
        let own_source_paths =
            std::collections::HashSet::from([path.to_str().unwrap().to_string()]);
        resync_instance(
            app.world_mut(),
            root,
            &baseline,
            &applied_prefab,
            &own_source_paths,
        )
        .unwrap();
        app.world_mut()
            .entity_mut(root)
            .insert(bsengine_core::PrefabInstanceBaseline {
                synced_ron: applied_content,
            });

        // Prove the baseline genuinely advanced: a fresh live edit to intensity,
        // followed by a resync against a file that changed range again, should
        // now diff the intensity override against 5.0 (the new baseline), not
        // 1.0 (the original one) -- and an unrelated further file change should
        // still be adopted for range.
        {
            let mut pl = app
                .world_mut()
                .get_mut::<bsengine_core::PointLight>(lamp)
                .unwrap();
            assert_eq!(
                pl.intensity, 5.0,
                "live intensity must still read as 5.0 post-resync"
            );
            pl.range = 99.0; // a fresh, different override this time
        }
        let further_new = parse(
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Root", primitive: Some(Cube)),
                EntityDescriptor(
                    name: "Lamp",
                    parent: Some("Root"),
                    point_light: Some((color: (1.0, 1.0, 1.0), intensity: 5.0, range: 10.0)),
                ),
            ])"#,
        );
        resync_instance(
            app.world_mut(),
            root,
            &applied_prefab,
            &further_new,
            &own_source_paths,
        )
        .unwrap();
        let pl = app.world().get::<bsengine_core::PointLight>(lamp).unwrap();
        assert_eq!(
            pl.intensity, 5.0,
            "intensity must still read 5.0 -- it matches the new baseline, so it's not an override \
             anymore and simply keeps its already-correct value"
        );
        assert_eq!(
            pl.range, 99.0,
            "the fresh override on range (set after the apply) must survive this second resync, \
             proving the applied value truly became the new baseline rather than some stale state \
             silently protecting the old override forever"
        );
    }

    #[test]
    fn apply_instance_to_prefab_refuses_when_the_instance_has_no_baseline() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("assets/prefabs/nobaseline.ron");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let source = r#"PrefabDescriptor(entities: [EntityDescriptor(name: "Root", primitive: Some(Cube))])"#;
        std::fs::write(&path, source).unwrap();

        let baseline = parse(source);
        let root = bsengine_scene::instantiate_prefab(
            app.world_mut(),
            &baseline,
            path.to_str().unwrap(),
            Some("MyNoBaseline"),
            None,
            None,
        )
        .unwrap();
        // Simulate a scene saved before override tracking existed: strip the baseline this
        // instance would normally have.
        app.world_mut()
            .entity_mut(root)
            .remove::<bsengine_core::PrefabInstanceBaseline>();

        let result = apply_instance_to_prefab(app.world_mut(), root);
        assert!(
            result.is_err(),
            "with no baseline to diff against, apply must refuse rather than guess"
        );
        let unchanged = std::fs::read_to_string(&path).unwrap();
        assert_eq!(
            unchanged, source,
            "the file must not be touched when the operation is refused"
        );
    }

    #[test]
    fn apply_instance_to_prefab_refuses_for_an_entity_with_no_prefab_instance() {
        let mut app = new_app();
        bsengine_scene::register_gameplay_reflect_types(&mut app);
        let plain = app
            .world_mut()
            .spawn(bsengine_scene::Name("NotAPrefabInstance".to_string()))
            .id();

        let result = apply_instance_to_prefab(app.world_mut(), plain);
        assert!(
            result.is_err(),
            "an entity with no PrefabInstance must be refused"
        );
    }
}
