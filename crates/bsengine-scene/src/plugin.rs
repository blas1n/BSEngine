use crate::types::{
    AssetRef, EntityDescriptor, PhysicsBodyDesc, PrimitiveMesh, SceneDescriptor, ScriptPath,
    TransformDescriptor,
};
use bevy_app::{App, Plugin, Startup};
use bevy_ecs::prelude::{Component, Entity, IntoSystemConfigs, ReflectComponent, World};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;
use bsengine_asset::{AssetGuid, AssetIndex};
use bsengine_core::{
    Camera, DirectionalLight, GlobalTransform, Material, PointLight, ProjectDir, SkyboxPath,
    SpotLight, Transform,
};
use bsengine_gltf::GltfAsset;
use glam::{Quat, Vec3};

/// Human-readable name assigned to a spawned scene entity, taken from `EntityDescriptor::name`.
///
/// This is the engine's only `Name`. An identical `bsengine_core::Name(pub String)` existed
/// alongside it with zero uses and was removed — registering both would have put two
/// indistinguishable `Name` rows in the Inspector, which displays short type names. If you
/// find yourself wanting an entity label in another crate, use this one.
#[derive(Component, Debug, Clone, Default, Reflect)]
#[reflect(Component, Default)]
pub struct Name(pub String);

/// Bevy plugin that loads a scene file at startup and spawns its entities into the world.
pub struct ScenePlugin {
    path: String,
}

impl ScenePlugin {
    /// Creates a plugin that will load and spawn the scene at `path` when added to an `App`.
    pub fn from_file(path: &str) -> Self {
        Self {
            path: path.to_string(),
        }
    }
}

impl Plugin for ScenePlugin {
    fn build(&self, app: &mut App) {
        let path = self.path.clone();
        app.add_systems(
            Startup,
            (move |world: &mut World| {
                let content = std::fs::read_to_string(&path)
                    .unwrap_or_else(|e| panic!("Failed to read scene {path}: {e}"));
                let scene: SceneDescriptor = ron::from_str(&content)
                    .unwrap_or_else(|e| panic!("Failed to parse scene {path}: {e}"));
                spawn_scene_entities(world, &scene.entities);
                if let Some(skybox_rel) = &scene.skybox {
                    let scene_dir = std::path::Path::new(&path)
                        .parent()
                        .unwrap_or(std::path::Path::new("."));
                    let skybox_full = scene_dir.join(skybox_rel).to_string_lossy().into_owned();
                    world.insert_resource(SkyboxPath(Some(skybox_full)));
                }
            })
            // The edge that makes resolution by identity actually happen.
            // `AssetIdentityPlugin` publishes `AssetIndex` from `Startup` too,
            // and two systems in one schedule are not ordered by being in it.
            // Getting this wrong is silent by construction: a spawn that finds
            // no index falls back to the path the scene stores (see
            // `resolve_asset_ref`), so the scene loads, the game runs, nothing
            // warns, and identity simply never resolves anything.
            //
            // Adding the plugins in the right order is not a substitute, and
            // not because it is fragile — because it does not work. With this
            // edge deleted,
            // `a_scene_resolves_against_the_index_the_identity_plugin_publishes`
            // fails in *both* registration orders, this spawn included when
            // the identity plugin was added first: an unconstrained schedule
            // sorts its systems, it does not replay `add_plugins` calls.
            //
            // Free in an app that never adds that plugin — the constraint
            // names a system type with no instance in the schedule, so there
            // is nothing to order against. Most of this module's own tests,
            // and every other caller of `ScenePlugin` outside the three
            // hosts, are such apps.
            .after(bsengine_asset::identity::build_asset_index),
        );
    }
}

thread_local! {
    /// Paths of prefab files currently being resolved, on this call stack.
    /// Threaded through nested `instantiate_prefab` → `spawn_scene_entities`
    /// → `instantiate_prefab` recursion without changing either function's
    /// public signature. Cleared entry-by-entry as each recursive call
    /// returns, so a sibling prefab reference at the same level is never
    /// mistaken for a cycle.
    static RESOLVING_PREFABS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

/// Removes one path from [`RESOLVING_PREFABS`] on drop, unconditionally --
/// including on an unwinding panic partway through resolving this prefab.
///
/// Nothing in the reachable call graph (`spawn_scene_entities`, reflected
/// component deserialization of scene-author-controlled data, ...) panics
/// today, so a plain insert-call-remove sequence has never actually leaked
/// in practice. But `RESOLVING_PREFABS` only ever clears entries one at a
/// time -- it has no other expiry -- so the day something downstream does
/// panic, a bare `remove(...)` statement placed after the call it guards
/// would simply never run, permanently marking that one prefab path as
/// "still resolving" for the rest of the process's life and turning every
/// future legitimate reference to it into a false-positive cycle warning.
/// An RAII guard makes the removal run on every exit path, not just the
/// non-panicking one.
struct ResolvingGuard(String);

impl Drop for ResolvingGuard {
    fn drop(&mut self) {
        RESOLVING_PREFABS.with(|r| {
            r.borrow_mut().remove(&self.0);
        });
    }
}

/// Reverses `resolve_project_path`: strips a `<ProjectDir>/` prefix back off
/// `prefab_path`, recovering the project-relative spelling
/// `PrefabInstance::source_path` is documented to hold. A no-op when no
/// `ProjectDir` is set or it's empty (every call site already passes the
/// project-relative form as-is in that case, since `resolve_project_path`
/// itself is a no-op then) or when `prefab_path` doesn't start with the
/// project dir's own prefix (defensive; shouldn't happen given every real
/// call site resolves through `resolve_project_path` first).
fn project_relative_prefab_path(world: &World, prefab_path: &str) -> String {
    let Some(project_dir) = world.get_resource::<bsengine_core::ProjectDir>() else {
        return prefab_path.to_string();
    };
    if project_dir.0.is_empty() {
        return prefab_path.to_string();
    }
    let prefix = format!("{}/", project_dir.0);
    prefab_path
        .strip_prefix(prefix.as_str())
        .unwrap_or(prefab_path)
        .to_string()
}

/// Instantiates the prefab at `prefab_path` into `world`, guarded against
/// cyclic prefab references via [`RESOLVING_PREFABS`]/[`ResolvingGuard`].
///
/// This is the one choke point every prefab-instantiation entry point --
/// the scene-file `prefab:` field (via [`resolve_prefab_reference`] below),
/// `Bsengine.instantiatePrefab` (`bsengine-scripting`), and the editor's
/// drag-and-drop (`bsengine-editor`) -- must route its *top-level* call
/// through, precisely so that call gets registered into
/// [`RESOLVING_PREFABS`] before anything inside the prefab can recurse.
///
/// That used to only be true for the scene-file path: the other two called
/// [`crate::prefab::instantiate_prefab`] directly with an already-parsed
/// descriptor, which never touches `RESOLVING_PREFABS` at all (nor should
/// it -- it has no path to key on). A prefab whose non-root child names its
/// own containing file was still caught -- `instantiate_prefab` delegates
/// to `spawn_scene_entities`, and a nested `prefab:` field there always
/// comes back through `resolve_prefab_reference` -> here -- but only on the
/// *second* encounter, since the first (top-level) call was never
/// registered. That let the prefab's real content silently spawn twice (one
/// extra recursion level) via those two paths, instead of once with a
/// warning like the identical file gets via a scene file. Routing all three
/// entry points through this function closes that gap by construction
/// rather than by each caller remembering to register itself.
///
/// Returns an error (never a warning of its own -- that's each caller's
/// job, matching how each already logs) on any failure: an unreadable file,
/// a parse error, or a cycle.
pub fn instantiate_prefab_from_path(
    world: &mut World,
    prefab_path: &str,
    root_name: Option<&str>,
    root_transform: Option<TransformDescriptor>,
    parent: Option<Entity>,
) -> Result<Entity, String> {
    let already_resolving = RESOLVING_PREFABS.with(|r| r.borrow().contains(prefab_path));
    if already_resolving {
        return Err(format!(
            "prefab '{prefab_path}' is already being resolved earlier in this same \
             instantiation chain -- skipping to avoid infinite recursion (cyclic prefab \
             reference)"
        ));
    }

    let content = std::fs::read_to_string(prefab_path)
        .map_err(|e| format!("prefab '{prefab_path}' could not be read: {e}"))?;
    let prefab: crate::types::PrefabDescriptor = ron::from_str(&content)
        .map_err(|e| format!("prefab '{prefab_path}' failed to parse: {e}"))?;

    RESOLVING_PREFABS.with(|r| r.borrow_mut().insert(prefab_path.to_string()));
    // Guarantees the entry above is removed on every exit from this point on
    // -- including an unwinding panic from `instantiate_prefab` or anything
    // it calls -- rather than only on the path that reaches a plain
    // `remove(...)` statement after it. See `ResolvingGuard`'s own doc for
    // why a missed removal would be a silent, permanent, process-lifetime
    // false positive rather than a one-off glitch.
    let _guard = ResolvingGuard(prefab_path.to_string());
    let source_path = project_relative_prefab_path(world, prefab_path);
    crate::prefab::instantiate_prefab(
        world,
        &prefab,
        &source_path,
        root_name,
        root_transform,
        parent,
    )
}

/// Resolves one `entity.prefab` reference: instantiates the file in the
/// entity's place via [`instantiate_prefab_from_path`]. Returns `None`
/// (after logging a warning) on any failure -- missing file, parse error,
/// bad root count, or a cycle -- so the caller can keep `spawned`'s length
/// in lockstep with `entities` even when this entity contributes no real
/// `Entity`.
fn resolve_prefab_reference(
    world: &mut World,
    entity_name: &str,
    prefab_path: &str,
    transform: Option<TransformDescriptor>,
) -> Option<Entity> {
    match instantiate_prefab_from_path(world, prefab_path, Some(entity_name), transform, None) {
        Ok(root) => Some(root),
        Err(e) => {
            tracing::warn!(
                "scene: entity '{entity_name}' references prefab '{prefab_path}', which \
                 failed to instantiate: {e}"
            );
            None
        }
    }
}

/// Turns one scene reference into the path the loader should actually open.
///
/// # The order, and why it is this one
///
/// **A known identity, then the stored path, then a path the project remembers
/// the asset leaving, then nothing.** The identity is
/// tried first because it is the only part of a reference that survives a
/// rename — that is the whole of roadmap item 30. Trying the path first and the
/// GUID only as a fallback would look equivalent and is not: a path that still
/// *exists*, because some other file moved into it, would resolve happily to
/// the wrong asset, and nothing would ever say so. The identity is the
/// narrower, more specific claim, so it goes first.
///
/// A GUID the index does not know falls back to the stored path rather than
/// failing: the identity being stale says nothing about the path, which may
/// well still name the right file. Losing the asset because the *label* went
/// missing would be a worse trade than the one before item 30.
///
/// The former-path lookup is genuinely last, after both. It answers a question
/// the other two cannot even be asked — "nothing has this identity and nothing
/// is at this path; did anything *used* to be?" — and answering it earlier
/// would let the memory of a move outrank an asset that is right there. See
/// [`recovered_from_a_former_path`] for the same order stated as code, and for
/// why the filesystem, not the index, has the final say.
///
/// This is deliberately the same order [`bsengine_asset::load_async`] uses for
/// the paths that live in JavaScript string literals, minus the identity step
/// no string literal can carry. Two surfaces resolving in two orders would mean
/// a scene and a script that name the same file could load different ones.
///
/// # When it is silent
///
/// A bare path — no identity recorded — resolves to itself with no diagnostic
/// at all. Sub-item B migrated every scene in `games/` to the `(guid, path)`
/// spelling, but a bare path stays valid on purpose: `AssetRef` accepts both so
/// that a hand-written scene, or a project with no sidecars at all, behaves
/// exactly as it did before item 30. Warning on each would make that supported
/// case look broken.
///
/// A bare path is still *recovered* if it names somewhere an asset has moved
/// away from, and that is not a leftover — it is the point. A path inside a
/// JavaScript string literal (`playSound("assets/sounds/hit.wav")`) can never
/// carry an identity, so recovery restricted to identified references would
/// miss the entire class of reference sub-item D exists for. What is silent is
/// a bare path that resolves; a bare path that only resolves because of a move
/// is not silent, because it is not resolving to what it says.
///
/// So is the absence of an index. All three hosts register
/// `AssetIdentityPlugin` now, but plenty of apps do not — most of this
/// module's own tests, `bsengine-app`'s and `bsengine-scripting`'s, anything
/// that adds `ScenePlugin` on its own — and with no index there is nothing to
/// resolve *against*, so an identified reference falls back to its stored path
/// in silence.
///
/// That silence is deliberate and it is also this function's sharpest edge:
/// **a missing index is indistinguishable, from the outside, from an index
/// that had not been published yet when the spawn ran.** Both load the game
/// perfectly. `ScenePlugin::build` is where that is prevented, with an
/// ordering edge rather than a hope; see the comment there.
///
/// # Cost
///
/// Two `BTreeMap` lookups at worst per identified reference, and one that
/// misses for a bare one — no scan of the index. A reference that resolves
/// normally never touches the disk: the `exists` check is behind the index
/// having said the stored path is one an asset left. The caller runs this once
/// per reference, before spawning, so a scene of tens of entities pays tens of
/// lookups once per load rather than per frame.
fn resolve_asset_ref(
    index: Option<&AssetIndex>,
    project_dir: Option<&ProjectDir>,
    entity_name: &str,
    field: &str,
    asset_ref: &AssetRef,
) -> String {
    let stored_path = asset_ref.path();
    // With no index there is nothing to resolve *against* at all — neither an
    // identity nor a former path — so both spellings are the path they store.
    let Some(index) = index else {
        return stored_path.to_string();
    };

    if let Some(guid_text) = asset_ref.guid() {
        // A hand-edited scene file is the expected source of this, and it is
        // reported separately from an identity nobody claims because the two
        // call for different fixes: this one is a spelling to correct, that one
        // is an asset to go find. Either way what is left to go on is the
        // stored path, so both fall through to the same last resort below.
        let Ok(guid) = guid_text.parse::<AssetGuid>() else {
            tracing::warn!(
                "scene: entity '{entity_name}' {field} '{stored_path}' has `{guid_text}` where an \
                 asset GUID should be; falling back to the stored path"
            );
            return recovered_from_a_former_path(
                index,
                project_dir,
                entity_name,
                field,
                stored_path,
            )
            .unwrap_or_else(|| stored_path.to_string());
        };

        match index.path_for_guid(guid) {
            // The rename item 30 exists to survive. Both paths are named
            // because a warning that reports only one leaves the developer
            // unable to tell which reference to fix or whether the file it
            // found is the right one.
            Some(current_path) if current_path != stored_path => {
                tracing::warn!(
                    "scene: entity '{entity_name}' {field} names '{stored_path}', but asset \
                     {guid} now lives at '{current_path}'; loading '{current_path}'. Re-save the \
                     scene to update the stored path"
                );
                return current_path.to_string();
            }
            Some(current_path) => return current_path.to_string(),
            None => {
                // The identity is stale. Before saying the stored path is all
                // that is left, ask whether the *path* is stale too — an asset
                // deleted and its replacement moved into place produces exactly
                // this pair, and reporting "nothing left to go on" while the
                // project remembers where the asset went would be a diagnostic
                // that stops one step short of the answer.
                if let Some(recovered) = recovered_from_a_former_path(
                    index,
                    project_dir,
                    entity_name,
                    field,
                    stored_path,
                ) {
                    return recovered;
                }
                tracing::warn!(
                    "scene: entity '{entity_name}' {field} '{stored_path}' carries identity \
                     {guid}, which no asset in this project has; the identity is stale, so the \
                     stored path is all that is left to go on"
                );
                return stored_path.to_string();
            }
        }
    }

    // A bare path: the pre-item-30 form, and what every scene in `games/` is
    // still written in. It has no identity to try, so the former-path lookup is
    // the only thing standing between a rename and a reference that loads
    // nothing.
    recovered_from_a_former_path(index, project_dir, entity_name, field, stored_path)
        .unwrap_or_else(|| stored_path.to_string())
}

/// Where a reference should load from when the path it stores is one an asset
/// has moved away from — and `None`, silently, in every other case.
///
/// # The order, which is [`bsengine_asset::load_async`]'s
///
/// 1. **Ask the index.** `AssetIndex::guid_for_former_path` already refuses a
///    path some asset currently occupies, so a reference that resolves normally
///    is not a former path and never reaches step 2.
/// 2. **Ask the filesystem.** A file at the stored path wins over the memory of
///    the one that left it. The index is a snapshot taken at `Startup`, so an
///    asset dropped into the vacated name after the scan is invisible to step 1;
///    redirecting away from a file that is right there, because of a move
///    recorded before it existed, is exactly the silent-wrong-asset failure
///    item 30 exists to end. The path is resolved through `ProjectDir` first,
///    because that is the spelling the loader will actually open.
/// 3. Otherwise: where the asset went, and a warning.
///
/// # It never recovers quietly
///
/// A reference that resolves somewhere other than what it spells has to say so.
/// The scene file still names the old path, so the next person to read it
/// learns nothing from the file itself — and a recovery nobody is told about
/// turns a broken reference into a permanent, invisible indirection layer,
/// which is the accumulated-forwarding pain Unreal documents rather than a
/// feature. This is a development-time affordance with an expiry; the warning
/// is what makes somebody spend it.
///
/// Unlike the load funnel's, this warning is *not* suppressed after the first
/// time, and the difference is the call frequency rather than a difference of
/// opinion: `load_async` is reachable from a script command that can fire every
/// frame, whereas this runs once per reference per scene load. Suppressing here
/// would mean a scene transition back to a scene loaded earlier reported
/// nothing.
fn recovered_from_a_former_path(
    index: &AssetIndex,
    project_dir: Option<&ProjectDir>,
    entity_name: &str,
    field: &str,
    stored_path: &str,
) -> Option<String> {
    let guid = index.guid_for_former_path(stored_path)?;
    let current_path = index.path_for_guid(guid)?;

    if std::path::Path::new(&bsengine_core::resolve_project_path(
        project_dir,
        stored_path,
    ))
    .exists()
    {
        return None;
    }

    tracing::warn!(
        "scene: entity '{entity_name}' {field} '{stored_path}' names a path no asset occupies — \
         asset {guid} used to live there and is now at '{current_path}', so that is what will \
         load. Re-save the scene to update the stored path: recovering through a former path is \
         a development-time convenience, not a permanent redirect"
    );
    Some(current_path.to_string())
}

/// Spawn entities from a list of descriptors into the given world.
/// Called at startup by ScenePlugin and at runtime for scene transitions.
pub fn spawn_scene_entities(world: &mut World, entities: &[EntityDescriptor]) {
    // Cloned once up front (cheap Arc clone) rather than re-fetched per
    // entity, since `world.spawn(..)` below holds an exclusive sub-borrow of
    // `world` for the rest of the loop body and a `Res`/`world.resource()`
    // call can't be interleaved with it.
    let app_registry = world
        .get_resource::<bevy_ecs::reflect::AppTypeRegistry>()
        .cloned();
    let project_dir = world.get_resource::<bsengine_core::ProjectDir>().cloned();

    // Every asset reference is resolved here, in one pass, before anything is
    // spawned. The borrow of `AssetIndex` has to end before the loop below
    // takes `world` mutably, and the alternative — cloning the whole index —
    // would copy a string per identified asset in the project to answer at most
    // two questions per entity. This borrows it, answers, and drops it.
    let resolved_refs: Vec<(Option<String>, Option<String>, Option<String>)> = {
        let index = world.get_resource::<AssetIndex>();
        entities
            .iter()
            .map(|entity| {
                let resolve = |field, asset_ref| {
                    resolve_asset_ref(index, project_dir.as_ref(), &entity.name, field, asset_ref)
                };
                (
                    entity.gltf.as_ref().map(|r| resolve("gltf", r)),
                    entity.script.as_ref().map(|r| resolve("script", r)),
                    entity.texture.as_ref().map(|r| resolve("texture", r)),
                )
            })
            .collect()
    };

    let mut spawned: Vec<Option<Entity>> = Vec::with_capacity(entities.len());

    for (entity, (gltf_path, script_path, texture_path)) in entities.iter().zip(&resolved_refs) {
        if let Some(prefab_ref) = &entity.prefab {
            // `resolve_asset_ref` only ever returns the path a scene *stores*
            // (identity-resolved or not) -- exactly like it does for
            // `gltf_path` below, which is why that branch separately joins
            // its result with `ProjectDir` before touching the filesystem.
            // Skipping this step here meant every project-relative
            // `prefab:` reference (the realistic case every game under
            // `games/*` actually uses) tried to open a path relative to
            // wherever the process happened to be running from and failed
            // with a plain "could not be read" I/O error.
            let prefab_path = resolve_asset_ref(
                world.get_resource::<AssetIndex>(),
                project_dir.as_ref(),
                &entity.name,
                "prefab",
                prefab_ref,
            );
            let prefab_path =
                bsengine_core::resolve_project_path(project_dir.as_ref(), &prefab_path);
            spawned.push(resolve_prefab_reference(
                world,
                &entity.name,
                &prefab_path,
                entity.transform.clone(),
            ));
            continue;
        }

        let mut builder = world.spawn(Name(entity.name.clone()));
        spawned.push(Some(builder.id()));

        if let Some(t) = &entity.transform {
            let mut rotation =
                Quat::from_xyzw(t.rotation[0], t.rotation[1], t.rotation[2], t.rotation[3]);
            if entity.camera {
                if let Some(target) = entity.look_at {
                    let pos = Vec3::from(t.position);
                    let dir = Vec3::from(target) - pos;
                    if dir.length_squared() > 1e-10 {
                        rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir.normalize());
                    }
                }
            }
            let transform = Transform {
                position: Vec3::from(t.position).into(),
                rotation: rotation.into(),
                scale: Vec3::from(t.scale).into(),
            };
            builder.insert((transform, GlobalTransform::default()));
        }

        if let Some(gltf_path) = gltf_path {
            builder.insert(GltfAsset::new(bsengine_core::resolve_project_path(
                project_dir.as_ref(),
                gltf_path,
            )));
        }

        if entity.camera {
            match entity.camera_fov {
                Some(fov) => {
                    builder.insert(Camera::perspective(fov, 16.0 / 9.0));
                }
                None => {
                    builder.insert(Camera::default());
                }
            }
        }

        if let Some(dl) = &entity.directional_light {
            builder.insert(DirectionalLight {
                color: Vec3::from(dl.color).into(),
                ambient: Vec3::from(dl.ambient).into(),
            });
            // Direction lives on Transform.rotation (rotation * -Z), same as
            // SpotLight; reuse any explicit translation/scale from the scene
            // file's own `transform:` block if one was given.
            let dir = Vec3::from(dl.direction).normalize_or(Vec3::NEG_Z);
            let rotation = Quat::from_rotation_arc(Vec3::NEG_Z, dir);
            let (translation, scale) = entity
                .transform
                .as_ref()
                .map(|t| (Vec3::from(t.position), Vec3::from(t.scale)))
                .unwrap_or((Vec3::ZERO, Vec3::ONE));
            builder.insert((
                Transform {
                    position: translation.into(),
                    rotation: rotation.into(),
                    scale: scale.into(),
                },
                GlobalTransform::default(),
            ));
        }

        if let Some(pl) = &entity.point_light {
            builder.insert(PointLight {
                color: Vec3::from(pl.color).into(),
                intensity: pl.intensity,
                range: pl.range,
            });
        }

        if let Some(sl) = &entity.spot_light {
            builder.insert(SpotLight {
                color: Vec3::from(sl.color).into(),
                intensity: sl.intensity,
                range: sl.range,
                inner_angle_degrees: sl.inner_angle_degrees.into(),
                outer_angle_degrees: sl.outer_angle_degrees.into(),
            });
        }

        if let Some(prim) = &entity.primitive {
            builder.insert(PrimitiveMesh(prim.clone()));
        }

        if let Some(script_path) = script_path {
            builder.insert(ScriptPath(script_path.clone()));
        }

        if let Some(texture_path) = texture_path {
            builder.insert(bsengine_core::TexturePath(texture_path.clone()));
        }

        // `texture` counts here too: an entity that names only a texture still
        // needs a Material, because the id the loader produces has nowhere else
        // to go and the texture would silently never appear.
        if entity.emissive.is_some() || entity.color.is_some() || entity.texture.is_some() {
            builder.insert(Material {
                emissive: entity.emissive.map(Vec3::from).unwrap_or(Vec3::ZERO).into(),
                base_color: entity.color.map(Vec3::from).unwrap_or(Vec3::ONE).into(),
                opacity: entity.opacity.unwrap_or(1.0),
                ..Default::default()
            });
        }

        if let (Some(rb), Some(col)) = (&entity.rigidbody, &entity.collider) {
            builder.insert(PhysicsBodyDesc {
                rigidbody: rb.clone(),
                collider: col.clone(),
                linear_damping: entity.linear_damping,
                angular_damping: entity.angular_damping,
            });
        }

        if !entity.components.is_empty() {
            if let Some(app_registry) = &app_registry {
                let registry = app_registry.read();
                for (type_path, value_ron) in &entity.components {
                    let Some(registration) = registry.get_with_type_path(type_path) else {
                        tracing::warn!(
                            "scene: entity '{}' references unknown reflected type path '{type_path}'",
                            entity.name
                        );
                        continue;
                    };
                    let Some(reflect_component) =
                        registration.data::<bevy_ecs::reflect::ReflectComponent>()
                    else {
                        tracing::warn!(
                            "scene: entity '{}' type path '{type_path}' is not a registered Component",
                            entity.name
                        );
                        continue;
                    };
                    let de =
                        bevy_reflect::serde::TypedReflectDeserializer::new(registration, &registry);
                    match ron::de::Deserializer::from_str(value_ron) {
                        Ok(mut deserializer) => {
                            match serde::de::DeserializeSeed::deserialize(de, &mut deserializer) {
                                Ok(value) => {
                                    reflect_component.apply_or_insert(
                                        &mut builder,
                                        value.as_ref(),
                                        &registry,
                                    );
                                }
                                Err(e) => tracing::warn!(
                                    "scene: entity '{}' component '{type_path}' RON value doesn't match its shape: {e}",
                                    entity.name
                                ),
                            }
                        }
                        Err(e) => tracing::warn!(
                            "scene: entity '{}' component '{type_path}' RON parse error: {e}",
                            entity.name
                        ),
                    }
                }
            }
        }
    }

    // Pass 3: resolve `parent: Some(name)` into a real `Parent(Entity)` now
    // that every entity in this call has spawned. A name map built only from
    // entities spawned *by this call* -- an entity left over from a previous
    // scene is never a valid parent target.
    let name_to_entity: std::collections::HashMap<&str, Entity> = entities
        .iter()
        .zip(spawned.iter().copied())
        .filter_map(|(e, s)| s.map(|entity| (e.name.as_str(), entity)))
        .collect();
    for (entity, &child_entity) in entities.iter().zip(&spawned) {
        let Some(child_entity) = child_entity else {
            continue; // a prefab reference that failed to instantiate -- nothing to parent
        };
        let Some(parent_name) = &entity.parent else {
            continue;
        };
        if parent_name == &entity.name {
            tracing::warn!(
                "scene: entity '{}' names itself as its own parent; leaving it a root",
                entity.name
            );
            continue;
        }
        match name_to_entity.get(parent_name.as_str()) {
            Some(&parent_entity) => {
                world
                    .entity_mut(child_entity)
                    .insert(bsengine_core::Parent(parent_entity));
            }
            None => {
                tracing::warn!(
                    "scene: entity '{}' names parent '{parent_name}', which does not exist \
                     in this scene; leaving it a root",
                    entity.name
                );
            }
        }
    }
}

/// Registers every `bsengine_core` component type an `EntityDescriptor`'s
/// `components:` field (arbitrary reflected components, e.g. `Shield`,
/// `SaveData`, `AnimationStateMachine`, `NavMeshAgent`, `Bloom`, `ToneMap`)
/// may reference, so [`spawn_scene_entities`]'s reflection-based
/// deserialization above can actually find and attach them.
///
/// Previously these registrations lived only inline in `EditorPlugin::build`
/// (`bsengine-editor`), which the headless test runtime
/// (`bsengine-runtime --test`'s `build_test_app`) never adds -- it can't,
/// since `EditorPlugin` requires the render/window stack a headless app
/// doesn't have. That meant every reflected `components:` entry was silently
/// dropped in headless test mode (logged only as a `tracing::warn!`
/// "unknown reflected type path", easy to miss), so any gameplay gated on
/// one of these components -- e.g. a `Shield`-based death check -- was
/// unexercisable via `bsengine-runtime --test`, even though the identical
/// scene loaded and played correctly in the windowed editor runtime. Call
/// this from both `EditorPlugin::build` and the headless test app's setup so
/// the two stay in parity.
pub fn register_gameplay_reflect_types(app: &mut bevy_app::App) {
    app.register_type::<bsengine_core::Camera>();
    app.register_type::<bsengine_core::PointLight>();
    app.register_type::<bsengine_core::DirectionalLight>();
    app.register_type::<bsengine_core::SpotLight>();
    app.register_type::<bsengine_core::Material>();
    app.register_type::<bsengine_core::TexturePath>();
    app.register_type::<bsengine_core::ParticleEmitter>();
    app.register_type::<bsengine_core::AmbientOcclusion>();
    app.register_type::<bsengine_core::AnimationPlayer>();
    app.register_type::<bsengine_core::Bloom>();
    app.register_type::<bsengine_core::CustomShader>();
    app.register_type::<bsengine_core::Lifetime>();
    app.register_type::<bsengine_core::NetworkId>();
    app.register_type::<bsengine_core::PrefabInstance>();
    app.register_type::<bsengine_core::SaveData>();
    app.register_type::<bsengine_core::Shield>();
    app.register_type::<bsengine_core::Skybox>();
    app.register_type::<bsengine_core::Timer>();
    app.register_type::<bsengine_core::ToneMap>();
    app.register_type::<bsengine_core::Visible>();
    app.register_type::<bsengine_core::Follow>();
    app.register_type::<bsengine_core::LookAt>();
    app.register_type::<bsengine_core::NavMeshAgent>();
    app.register_type::<bsengine_core::Transform>();
    app.register_type::<bsengine_core::GlobalTransform>();
    app.register_type::<bsengine_core::Parent>();
    app.register_type::<bsengine_core::AnimationStateMachine>();
    // `AsmState` holds an `Option<BlendTree1D>`, and a field type the registry
    // does not know does not make deserialization *fail* — the containing map
    // comes back empty and nothing warns. A state machine whose `states` is
    // silently `{}` still loads, still runs, and simply never animates, which
    // is why `a_state_machine_written_before_blend_trees_still_parses` asserts
    // on the states rather than on the component being present.
    app.register_type::<bsengine_core::BlendTree1D>();
    app.register_type::<bsengine_core::BlendClip>();
    app.register_type::<Option<bsengine_core::BlendTree1D>>();
    // AnimationStateMachine::triggers is a HashSet<String>; unlike Map/List/Struct
    // fields, HashSet isn't structurally recursed by TypedReflectDeserializer and
    // needs its value-kind ReflectDeserialize registered explicitly, or JSON/RON
    // authoring fails with "doesn't have ReflectDeserialize" even for an empty
    // `[]`. Same story for ReflectSerialize on the save side. See
    // `bsengine-editor`'s prior inline copy of this registration for the fuller
    // history (design-research probe that found this).
    app.register_type_data::<std::collections::HashSet<String>, bevy_reflect::ReflectDeserialize>();
    app.register_type_data::<std::collections::HashSet<String>, bevy_reflect::ReflectSerialize>();
    app.register_type::<bsengine_core::Tween>();

    // Two types that are not `bsengine_core`'s, registered here anyway, each
    // for its own reason.
    //
    // `PhysicsBodyDesc` is this crate's own -- `spawn_scene_entities` inserts
    // it from an entity's `rigidbody:`/`collider:` fields -- so this is simply
    // where it lives. Its three field types go with it: a `Reflect` field must
    // itself be reflectable, and naming them keeps that true independently of
    // `register_type`'s dependency walk.
    //
    // `GltfAsset` belongs to `bsengine-gltf`, and the obvious home would be
    // `GltfPlugin::build`. That home is wrong: `GltfPlugin` is absent from the
    // headless `bsengine-runtime --test` app (it needs the GPU registries
    // `WgpuRHIPlugin` publishes), so a registration made there would be
    // missing from exactly the host the E2E replays run in -- the same silent
    // divergence between the editor and the test runtime this whole function
    // was written to end. Both hosts call this, and the scene -> gltf edge
    // already exists.
    //
    // `SkinnedMesh` and `AnimationClipLibrary` are `bsengine-gltf`'s for the
    // same reason and land here for the same one. Both are attached by
    // `GltfPlugin` at import time, and both reflect only their identifying
    // data -- their bulk per-vertex and per-keyframe fields are
    // `#[reflect(ignore)]`; see each type's own note for what is hidden and
    // why.
    app.register_type::<PhysicsBodyDesc>();
    app.register_type::<crate::types::RigidBodyDesc>();
    app.register_type::<crate::types::ColliderDesc>();
    app.register_type::<crate::types::ColliderShapeDesc>();
    app.register_type::<bsengine_gltf::GltfAsset>();
    app.register_type::<bsengine_gltf::SkinnedMesh>();
    app.register_type::<bsengine_gltf::AnimationClipLibrary>();

    // `Name` is this crate's own, attached by `spawn_scene_entities` to every
    // entity a scene declares. It was unregistered until now because a second
    // `Name` in `bsengine-core` -- structurally identical, zero uses -- made it
    // ambiguous which one the Inspector's short-name list would be showing.
    // That one is gone; this is the only `Name`.
    app.register_type::<Name>();
}

#[cfg(test)]
mod tests {
    use super::{Name, ScenePlugin};
    use bsengine_app::new_app;
    use bsengine_asset::test_support::{unique, ProbeDir};
    use bsengine_core::{Camera, DirectionalLight, GlobalTransform, Transform};
    use glam::Vec3;

    fn write_temp_scene(filename: &str, content: &str) -> String {
        let path = std::env::temp_dir().join(filename);
        std::fs::write(&path, content).unwrap();
        path.to_str().unwrap().to_string()
    }

    #[test]
    fn every_reflect_wrapper_can_be_authored_in_a_scene() {
        // `impl_reflect_value!` makes these opaque leaf types, and an opaque
        // type can only be deserialized through `ReflectDeserialize`. Without
        // it the whole *containing component* comes back absent -- a warning in
        // the log, no error, and an entity quietly missing what the scene said
        // it had. `ReflectColor` had exactly that until item 28 tripped over
        // it; this covers the siblings so the next one is caught here rather
        // than in a game.
        //
        // Driven through real components, because the wrappers are only
        // reachable that way: Transform holds two ReflectVec3s and a
        // ReflectQuat, SpotLight a ReflectDegrees.
        //
        // ReflectVec2, ReflectVec4 and ReflectMat4 are fixed the same way but
        // cannot be covered here: no component has a field of those types, so
        // there is nothing to author. They are done for uniformity -- six
        // near-identical wrappers where three behave differently is its own
        // trap.
        // `Follow` is deliberately not in this list even though it holds a
        // ReflectVec3: its other field is a `bevy_ecs::entity::Entity`, which
        // has no ReflectDeserialize either -- and should not. An entity id is a
        // runtime handle, so a scene naming one would be pointing at whichever
        // entity happened to land in that slot. Not authorable is the right
        // answer there, and it is a different fact from the one this test is
        // about.
        let cases: [(&str, &str); 2] = [
            (
                "bsengine_core::transform::Transform",
                "(position: (1.0, 2.0, 3.0), rotation: (0.0, 0.0, 0.0, 1.0), scale: (1.0, 1.0, 1.0))",
            ),
            (
                "bsengine_core::light::SpotLight",
                "(color: (1.0, 1.0, 1.0), intensity: 1.0, range: 10.0, inner_angle_degrees: 15.0, outer_angle_degrees: 30.0)",
            ),
        ];

        for (path, value) in cases {
            // Built rather than parsed: nesting a RON value inside a RON
            // string inside a Rust raw string is three levels of quoting for
            // no gain.
            let mut scene: crate::types::SceneDescriptor =
                ron::from_str(r#"SceneDescriptor(entities: [EntityDescriptor(name: "E")])"#)
                    .expect("the base scene should parse");
            scene.entities[0].components = vec![(path.to_string(), value.to_string())];

            let mut app = new_app();
            super::register_gameplay_reflect_types(&mut app);
            super::spawn_scene_entities(app.world_mut(), &scene.entities);

            let entity = app
                .world()
                .iter_entities()
                .next()
                .expect("the entity should exist")
                .id();
            let registry = app
                .world()
                .resource::<bevy_ecs::reflect::AppTypeRegistry>()
                .clone();
            let registry = registry.read();
            let registration = registry
                .get_with_type_path(path)
                .unwrap_or_else(|| panic!("{path} is not registered for reflection"));
            let reflect_component = registration
                .data::<bevy_ecs::reflect::ReflectComponent>()
                .unwrap_or_else(|| panic!("{path} has no ReflectComponent"));
            assert!(
                reflect_component
                    .reflect(app.world().entity(entity))
                    .is_some(),
                "{path} did not survive deserialization -- one of its fields is an                  opaque wrapper with no ReflectDeserialize"
            );
        }
    }

    #[test]
    fn mini_arenas_hit_spark_emitter_survives_deserialization() {
        // A reflected component can fail to deserialize *silently*: the scene
        // parses, the entity spawns, a warning goes to the log and the
        // component is simply not there. This one did exactly that until
        // `ReflectColor` was given `ReflectDeserialize` -- an opaque type
        // without it takes the whole containing component down with it.
        //
        // Missing *fields* are tolerated, which is worth knowing and is not
        // what item 29 was about: the value is applied over a Default, so a
        // field left out of the RON keeps its default rather than voiding the
        // component. Item 29's silent emptying was a collection whose element
        // type had gained a field, which is a different failure.
        //
        // Reads the real scene rather than a fixture, because the point is to
        // fail when someone edits that file.
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(2)
            .expect("workspace root is two levels above crates/bsengine-scene");
        let text = std::fs::read_to_string(root.join("games/mini-arena/assets/scenes/main.ron"))
            .expect("mini-arena's scene should be readable");
        let scene: crate::types::SceneDescriptor =
            ron::from_str(&text).expect("mini-arena's scene should parse");

        // The reflect registry is what reflected components deserialize
        // against; without it every one of them comes back absent, which is
        // the very failure this test exists to detect.
        let mut app = new_app();
        super::register_gameplay_reflect_types(&mut app);
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut query = app.world_mut().query::<&bsengine_core::ParticleEmitter>();
        assert_eq!(
            query.iter(app.world()).count(),
            1,
            "the emitter did not survive deserialization. Run with RUST_LOG=warn:              the scene loader logs why, and the usual cause is a field whose              type is opaque to reflection and has no ReflectDeserialize"
        );
    }

    #[test]
    fn a_scene_texture_reference_becomes_a_texture_path_component() {
        // Parsing the field proves nothing on its own: the resolved path has to
        // reach the entity, because that component is the only thing the loader
        // downstream can see.
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Floor", texture: Some("assets/textures/checker.png")),
        ])"#;
        let scene: crate::types::SceneDescriptor = ron::from_str(ron).expect("scene should parse");

        let mut app = new_app();
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut query = app.world_mut().query::<&bsengine_core::TexturePath>();
        let paths: Vec<String> = query.iter(app.world()).map(|p| p.0.clone()).collect();
        assert_eq!(paths, vec!["assets/textures/checker.png".to_string()]);
    }

    #[test]
    fn an_entity_with_only_a_texture_still_gets_a_material() {
        // The id the loader produces is written onto Material. An entity naming
        // a texture but no colour used to get no Material at all, which would
        // have left the texture nowhere to land.
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Floor", texture: Some("t.png")),
        ])"#;
        let scene: crate::types::SceneDescriptor = ron::from_str(ron).expect("scene should parse");

        let mut app = new_app();
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut query = app.world_mut().query::<&bsengine_core::Material>();
        assert_eq!(query.iter(app.world()).count(), 1);
    }

    #[test]
    fn parent_field_resolves_to_a_real_parent_component() {
        let scene: crate::types::SceneDescriptor = ron::from_str(
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Body"),
                EntityDescriptor(name: "Wheel", parent: Some("Body")),
            ])"#,
        )
        .expect("a scene entity naming its parent should parse");

        let mut app = new_app();
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut query = app.world_mut().query::<(
            bevy_ecs::prelude::Entity,
            &Name,
            Option<&bsengine_core::Parent>,
        )>();
        let rows: Vec<(
            bevy_ecs::prelude::Entity,
            String,
            Option<bevy_ecs::prelude::Entity>,
        )> = query
            .iter(app.world())
            .map(|(e, n, p)| (e, n.0.clone(), p.map(|p| p.0)))
            .collect();

        let body_entity = rows
            .iter()
            .find(|(_, name, _)| name == "Body")
            .map(|(e, _, _)| *e)
            .expect("Body should have spawned");
        let wheel_parent = rows
            .iter()
            .find(|(_, name, _)| name == "Wheel")
            .and_then(|(_, _, p)| *p)
            .expect("Wheel should have a Parent component");
        assert_eq!(
            wheel_parent, body_entity,
            "Wheel's parent should be the Body entity"
        );
    }

    #[test]
    fn scene_file_prefab_reference_instantiates_the_prefab() {
        // `ProbeDir`/`unique` (not `tempfile`, which this crate does not
        // depend on) are the exact real-filesystem-fixture pattern the
        // `identity` submodule below already uses for a real `AssetIndex`
        // scan; borrowed here for the same reason -- a prefab reference has
        // to name a real file on disk, and there is no way to fake that.
        let dir = ProbeDir(std::env::temp_dir().join(unique("scene-prefab-basic")));
        let root = &dir.0;
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        std::fs::write(
            root.join("assets/prefabs/enemy.ron"),
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Gun", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        )
        .unwrap();

        let scene: crate::types::SceneDescriptor = ron::from_str(&format!(
            r#"SceneDescriptor(entities: [
                EntityDescriptor(
                    name: "Spawn1",
                    prefab: Some("{}"),
                    transform: Some((position: (5.0, 0.0, 0.0))),
                ),
            ])"#,
            root.join("assets/prefabs/enemy.ron")
                .to_str()
                .unwrap()
                .replace('\\', "/")
        ))
        .unwrap();

        // No `ProjectDir` here on purpose -- the prefab reference above is
        // already the temp directory's absolute path, and `ProjectDir`
        // resolution (see the dedicated test right below this one) would
        // join it onto that absolute path rather than leave it alone.
        let mut app = new_app();
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        // The prefab's root always takes over the instantiation point's own
        // name verbatim (no suffix) -- that's what "Spawn1: prefab: Some(...)"
        // means: "instantiate this prefab and call the result Spawn1", not
        // "spawn a separate Spawn1 next to an auto-named prefab root". Only
        // *non-root* entities inside the prefab (here, "Gun") get the shared
        // instance suffix, since only those need in-file uniqueness, not a
        // caller-meaningful name.
        assert_eq!(
            names.iter().filter(|n| n.as_str() == "Spawn1").count(),
            1,
            "the prefab's root should be named exactly 'Spawn1' -- the \
             instantiation point's own name, taken over verbatim -- and only \
             once (not left behind as a separate stray entity), names: {names:?}"
        );
        assert!(
            names.iter().any(|n| n.starts_with("Gun#")),
            "the prefab's non-root child should have been instantiated with a \
             shared-suffix name, names: {names:?}"
        );
        assert_eq!(
            names.len(),
            2,
            "exactly two entities should exist: the renamed root and the child, names: {names:?}"
        );
    }

    #[test]
    fn scene_file_prefab_reference_resolves_against_project_dir() {
        // The realistic case every game under `games/*` actually uses:
        // `prefab:` written as a path relative to the project root, exactly
        // like every other asset reference (`gltf:`, `script:`, `texture:`)
        // already resolves against `ProjectDir` before touching the
        // filesystem (see the `gltf_path` branch right below the prefab
        // branch in `spawn_scene_entities`). The test above this one
        // deliberately does not cover this: it writes the temp directory's
        // own *absolute* path straight into `prefab: Some(...)`, so a
        // regression that resolved `prefab:` against the process's working
        // directory instead of `ProjectDir` would have passed that test
        // while failing every real project's scene file.
        let dir = ProbeDir(std::env::temp_dir().join(unique("scene-prefab-project-relative")));
        let root = &dir.0;
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        std::fs::write(
            root.join("assets/prefabs/enemy.ron"),
            r#"PrefabDescriptor(entities: [
                EntityDescriptor(name: "Body", primitive: Some(Cube)),
                EntityDescriptor(name: "Gun", parent: Some("Body"), primitive: Some(Cube)),
            ])"#,
        )
        .unwrap();

        // Project-relative, not the temp dir's absolute path -- the whole
        // point of this test.
        let scene: crate::types::SceneDescriptor = ron::from_str(
            r#"SceneDescriptor(entities: [
                EntityDescriptor(
                    name: "Spawn1",
                    prefab: Some("assets/prefabs/enemy.ron"),
                ),
            ])"#,
        )
        .unwrap();

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir(
            root.to_str().unwrap().to_string(),
        ));
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert_eq!(
            names.iter().filter(|n| n.as_str() == "Spawn1").count(),
            1,
            "a project-relative prefab reference must resolve against ProjectDir and \
             actually instantiate -- it must not silently fail to read the file just \
             because it isn't already an absolute path, names: {names:?}"
        );
        assert!(
            names.iter().any(|n| n.starts_with("Gun#")),
            "the prefab's non-root child should have instantiated too, names: {names:?}"
        );
        assert_eq!(
            names.len(),
            2,
            "exactly two entities should exist: the renamed root and the child, names: {names:?}"
        );
    }

    #[test]
    fn nested_prefab_reference_instantiates_recursively() {
        let dir = ProbeDir(std::env::temp_dir().join(unique("scene-prefab-nested")));
        let root = &dir.0;
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        std::fs::write(
            root.join("assets/prefabs/wheel.ron"),
            r#"PrefabDescriptor(entities: [EntityDescriptor(name: "WheelRoot", primitive: Some(Cube))])"#,
        )
        .unwrap();
        std::fs::write(
            root.join("assets/prefabs/car.ron"),
            format!(
                r#"PrefabDescriptor(entities: [
                    EntityDescriptor(name: "Body", primitive: Some(Cube)),
                    EntityDescriptor(name: "WheelSlot", parent: Some("Body"), prefab: Some("{}")),
                ])"#,
                root.join("assets/prefabs/wheel.ron")
                    .to_str()
                    .unwrap()
                    .replace('\\', "/")
            ),
        )
        .unwrap();

        let scene: crate::types::SceneDescriptor = ron::from_str(&format!(
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Spawn1", prefab: Some("{}")),
            ])"#,
            root.join("assets/prefabs/car.ron")
                .to_str()
                .unwrap()
                .replace('\\', "/")
        ))
        .unwrap();

        // No `ProjectDir` here either, for the same reason as the top-level
        // test above: both prefab references (`car.ron` and, inside it,
        // `wheel.ron`) are already absolute temp-directory paths.
        let mut app = new_app();
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        // Car's root ("Body") takes over the scene's instantiation point
        // name "Spawn1" verbatim. Car's non-root "WheelSlot" (itself a
        // nested prefab reference) gets the shared suffix like any other
        // non-root entity -- "WheelSlot#N". Wheel's own root ("WheelRoot")
        // then takes over *that* name in turn, the same rule applied one
        // level deeper: nesting is just this same rule recursing, so the
        // wheel's actual spawned name is "WheelSlot#N", not "WheelRoot#N".
        assert_eq!(
            names.iter().filter(|n| n.as_str() == "Spawn1").count(),
            1,
            "car's root should be renamed to the scene's instantiation point name, \
             names: {names:?}"
        );
        assert!(
            names.iter().any(|n| n.starts_with("WheelSlot#")),
            "the nested prefab's root should take over its own referencing entity's \
             (already-suffixed) name, not keep its own original name, names: {names:?}"
        );
        assert!(
            !names.iter().any(|n| n.starts_with("WheelRoot")),
            "the wheel prefab's original entity name should not survive instantiation \
             at all, names: {names:?}"
        );
    }

    #[test]
    fn cyclic_prefab_reference_fails_loudly_instead_of_recursing_forever() {
        let dir = ProbeDir(std::env::temp_dir().join(unique("scene-prefab-cycle")));
        let root = &dir.0;
        std::fs::create_dir_all(root.join("assets/prefabs")).unwrap();
        let a_path = root.join("assets/prefabs/a.ron");
        let b_path = root.join("assets/prefabs/b.ron");
        std::fs::write(
            &a_path,
            format!(
                r#"PrefabDescriptor(entities: [EntityDescriptor(name: "A", prefab: Some("{}"))])"#,
                b_path.to_str().unwrap().replace('\\', "/")
            ),
        )
        .unwrap();
        std::fs::write(
            &b_path,
            format!(
                r#"PrefabDescriptor(entities: [EntityDescriptor(name: "B", prefab: Some("{}"))])"#,
                a_path.to_str().unwrap().replace('\\', "/")
            ),
        )
        .unwrap();

        let scene: crate::types::SceneDescriptor = ron::from_str(&format!(
            r#"SceneDescriptor(entities: [EntityDescriptor(name: "Spawn1", prefab: Some("{}"))])"#,
            a_path.to_str().unwrap().replace('\\', "/")
        ))
        .unwrap();

        // No `ProjectDir` here either -- same reason as the other two tests
        // above that write an absolute prefab path straight into the RON.
        let mut app = new_app();
        // Must not hang/stack-overflow. spawn_scene_entities itself doesn't
        // return a Result, so the observable contract is: it returns at
        // all (this test having a timeout via the test harness itself is
        // the real safety net) and logs a warning rather than crashing.
        super::spawn_scene_entities(app.world_mut(), &scene.entities);
    }

    #[test]
    fn unresolvable_or_self_named_parent_leaves_the_entity_a_root() {
        let scene: crate::types::SceneDescriptor = ron::from_str(
            r#"SceneDescriptor(entities: [
                EntityDescriptor(name: "Typo", parent: Some("Doesnt Exist")),
                EntityDescriptor(name: "SelfRef", parent: Some("SelfRef")),
            ])"#,
        )
        .expect("a scene should parse even with an unresolvable parent name");

        let mut app = new_app();
        super::spawn_scene_entities(app.world_mut(), &scene.entities);

        let mut query = app
            .world_mut()
            .query::<(&Name, Option<&bsengine_core::Parent>)>();
        for (name, parent) in query.iter(app.world()) {
            assert!(
                parent.is_none(),
                "{} should have no Parent component (unresolvable or self-referential), got {:?}",
                name.0,
                parent.map(|p| p.0)
            );
        }
    }

    #[test]
    fn scene_plugin_spawns_entities() {
        let ron = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Player", components: []), EntityDescriptor(name: "Camera", components: [])])"#;
        let path = write_temp_scene("test_spawn.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert!(
            names.contains(&"Player".to_string()),
            "Player missing: {:?}",
            names
        );
        assert!(
            names.contains(&"Camera".to_string()),
            "Camera missing: {:?}",
            names
        );
    }

    #[test]
    fn scene_plugin_empty_scene() {
        let ron = r#"SceneDescriptor(entities: [])"#;
        let path = write_temp_scene("test_empty.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        assert_eq!(q.iter(app.world()).count(), 0);
    }

    #[test]
    fn scene_plugin_spawns_transform() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Cube",
                transform: Some((position: (1.0, 2.0, 3.0))),
            )
        ])"#;
        let path = write_temp_scene("test_transform.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &Transform, &GlobalTransform)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        let (name, t, _) = &results[0];
        assert_eq!(name.0, "Cube");
        assert!((t.position.x - 1.0).abs() < 1e-5);
        assert!((t.position.y - 2.0).abs() < 1e-5);
        assert!((t.position.z - 3.0).abs() < 1e-5);
    }

    #[test]
    fn scene_plugin_spawns_camera() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "MainCam", camera: true)
        ])"#;
        let path = write_temp_scene("test_camera.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<(&Name, &Camera)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "MainCam");
    }

    #[test]
    fn scene_plugin_spawns_directional_light() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Sun",
                directional_light: Some((direction: (0.0, -1.0, 0.0))),
            )
        ])"#;
        let path = write_temp_scene("test_light.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &DirectionalLight, &Transform)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        let (name, _light, transform) = &results[0];
        assert_eq!(name.0, "Sun");
        let derived_dir = transform.rotation.0 * Vec3::NEG_Z;
        assert!((derived_dir.y - (-1.0)).abs() < 1e-5);
    }

    #[test]
    fn scene_plugin_applies_reflected_component_from_ron_value() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Enemy",
                components: [
                    ("bsengine_core::nav_mesh_agent::NavMeshAgent", "(destination: None, speed: 3.5, angular_speed: 2.0, acceleration: 8.0, stopping_distance: 0.1, radius: 0.3, height: 1.8, state: Idle, enabled: true)"),
                ],
            )
        ])"#;
        let path = write_temp_scene("test_reflected_component.ron", ron);

        let mut app = new_app();
        app.register_type::<bsengine_core::NavMeshAgent>();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::NavMeshAgent)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "Enemy");
        assert!((results[0].1.speed - 3.5).abs() < 1e-5);
        assert!(results[0].1.enabled);
    }

    #[test]
    fn scene_plugin_applies_prefab_instance_from_ron_value() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Turret",
                components: [
                    ("bsengine_core::prefab_instance::PrefabInstance", "(source_path: \"assets/prefabs/turret.ron\")"),
                ],
            )
        ])"#;
        let path = write_temp_scene("test_prefab_instance_component.ron", ron);

        let mut app = new_app();
        app.register_type::<bsengine_core::PrefabInstance>();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_core::PrefabInstance)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "Turret");
        assert_eq!(results[0].1.source_path, "assets/prefabs/turret.ron");
    }

    #[test]
    fn a_state_machines_states_survive_deserialization() {
        // Guards a failure mode with no diagnostic at all.
        //
        // Reflected deserialization requires every field of a struct to be
        // present in the RON. When one is missing the *containing collection*
        // comes back empty rather than erroring, so a state machine whose
        // `states` silently became `{}` still loads, still runs, and simply
        // never animates. Adding `AsmState::blend` did exactly that to
        // mini-arena until its scene was updated, and no warning was printed.
        //
        // So this asserts on the states, not on the component being present:
        // the component is always present, which is the whole problem.
        let ron = r##"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Player",
                components: [
                    ("bsengine_core::animation_state_machine::AnimationStateMachine", r#"(
                        states: {
                            "idle": (clip: "Survey", blend: None, looping: true, speed: 1.0, duration: 1.0),
                            "walk": (clip: "Walk", blend: None, looping: true, speed: 1.0, duration: 1.0),
                        },
                        transitions: [
                            (from: "idle", to: "walk", condition: FloatGreater(param: "speed", threshold: 0.1), blend_duration: 0.15),
                        ],
                        current_state: "idle",
                        params_float: {"speed": 0.0},
                        params_bool: {},
                        triggers: [],
                        blend_from: None,
                        blend_weight: 1.0,
                        blend_duration: 0.0,
                        blend_elapsed: 0.0,
                    )"#),
                ],
            )
        ])"##;
        let path = write_temp_scene("test_asm_without_blend_field.ron", ron);

        let mut app = new_app();
        // The whole function, not a bare `register_type`: this component's
        // `triggers` is a `HashSet<String>`, which `TypedReflectDeserializer`
        // does not structurally recurse, so it needs the extra registration
        // that lives in there. Registering only the component itself fails with
        // "doesn't have ReflectDeserialize" — which reads exactly like the
        // field this test is about having broken something, and does not.
        super::register_gameplay_reflect_types(&mut app);
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<&bsengine_core::AnimationStateMachine>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1, "the state machine must survive the load");
        assert_eq!(results[0].current_state, "idle");
        assert_eq!(results[0].states.len(), 2);
        assert!(
            results[0].states["idle"].blend.is_none(),
            "a state that names no blend tree has none"
        );
    }

    #[test]
    fn a_blend_tree_authored_in_a_scene_arrives_intact() {
        // mini-arena's spelling. Worth its own test for the same reason as the
        // one above: a blend tree that failed to deserialize would leave the
        // state's `blend` as None, the character would animate from the single
        // `clip` instead, and nothing anywhere would say so — it would just
        // stop blending.
        let ron = r##"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Player",
                components: [
                    ("bsengine_core::animation_state_machine::AnimationStateMachine", r#"(
                        states: {
                            "locomotion": (clip: "Survey", blend: Some((
                                param: "speed",
                                clips: [
                                    (clip: "Survey", threshold: 0.0),
                                    (clip: "Walk", threshold: 4.5),
                                    (clip: "Run", threshold: 8.1),
                                ],
                            )), looping: true, speed: 1.0, duration: 1.0),
                        },
                        transitions: [],
                        current_state: "locomotion",
                        params_float: {"speed": 0.0},
                        params_bool: {},
                        triggers: [],
                        blend_from: None,
                        blend_weight: 1.0,
                        blend_duration: 0.0,
                        blend_elapsed: 0.0,
                    )"#),
                ],
            )
        ])"##;
        let path = write_temp_scene("test_blend_tree_scene.ron", ron);

        let mut app = new_app();
        super::register_gameplay_reflect_types(&mut app);
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<&bsengine_core::AnimationStateMachine>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        let tree = results[0].states["locomotion"]
            .blend
            .as_ref()
            .expect("the blend tree must survive the load");
        assert_eq!(tree.param, "speed");
        assert_eq!(tree.clips.len(), 3);
        assert_eq!(tree.clips[2].clip, "Run");
        assert!((tree.clips[2].threshold - 8.1).abs() < 1e-5);
    }

    #[test]
    fn scene_plugin_unknown_reflected_type_path_is_skipped_not_fatal() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(
                name: "Ghost",
                components: [("not::a::real::Type", "()")],
            )
        ])"#;
        let path = write_temp_scene("test_unknown_type.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<&Name>();
        let names: Vec<String> = q.iter(app.world()).map(|n| n.0.clone()).collect();
        assert!(names.contains(&"Ghost".to_string()));
    }

    #[test]
    fn scene_plugin_gltf_path_resolves_against_project_dir() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Player", gltf: Some("models/hero.glb")),
        ])"#;
        let path = write_temp_scene("test_gltf_project_dir.ron", ron);

        let mut app = new_app();
        app.insert_resource(bsengine_core::ProjectDir("games/demo".to_string()));
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0 .0, "Player");
        assert_eq!(results[0].1.path, "games/demo/models/hero.glb");
    }

    #[test]
    fn scene_plugin_gltf_path_unchanged_without_project_dir() {
        let ron = r#"SceneDescriptor(entities: [
            EntityDescriptor(name: "Player", gltf: Some("models/hero.glb")),
        ])"#;
        let path = write_temp_scene("test_gltf_no_project_dir.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app
            .world_mut()
            .query::<(&Name, &bsengine_gltf::GltfAsset)>();
        let results: Vec<_> = q.iter(app.world()).collect();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].1.path, "models/hero.glb");
    }

    // ---- resolution by identity (roadmap item 30, sub-item C) -------------
    //
    // Every test below drives the real spawn path — a scene file on disk, a
    // real `ScenePlugin`, a real `AssetIndex` built by `bsengine-asset`'s real
    // scan of a real directory — because the thing being measured is what an
    // entity ends up pointing at, and an index assembled by hand would prove
    // only that this module agrees with itself about map lookups.
    //
    // The warnings are asserted as text rather than as "some warning
    // happened". A scene that silently resolved to a different file than it
    // names is precisely the failure item 30 exists to end, and a diagnostic
    // that does not say *which* file is a diagnostic that leaves the developer
    // exactly where they started.
    mod identity {
        use super::super::{Name, ScenePlugin};
        use super::write_temp_scene;
        use crate::types::ScriptPath;
        use bsengine_app::new_app;
        use bsengine_asset::identity::scan;
        use bsengine_asset::test_support::{capture_warnings, unique, ProbeDir};
        use bsengine_asset::{AssetGuid, AssetIdentityPlugin, AssetIndex};

        /// Where the probe asset really is, and the stale path a scene written
        /// before it moved would still name.
        ///
        /// The two share no substring, so an assertion that the warning names
        /// one cannot be satisfied by the other.
        const CURRENT_PATH: &str = "assets/models/fox.glb";
        const STALE_PATH: &str = "assets/models/vulpes.glb";

        /// A throwaway project holding one real asset, and the index a real
        /// scan built from it.
        ///
        /// The GUID is read back out of the index rather than minted here: the
        /// spelling resolution has to read is the spelling a scan writes into a
        /// `.meta`, and a hand-made one would not prove those agree.
        struct Probe {
            /// Kept alive so the directory (and the sidecar the scan wrote into
            /// it) outlives the test that is using it.
            _dir: ProbeDir,
            index: AssetIndex,
            guid: String,
        }

        fn probe(tag: &str) -> Probe {
            let dir = ProbeDir(std::env::temp_dir().join(unique(tag)));
            let asset = dir.0.join(CURRENT_PATH);
            std::fs::create_dir_all(asset.parent().expect("probe asset has a parent"))
                .expect("create probe directories");
            std::fs::write(&asset, b"fake glb").expect("write probe asset");

            let index = scan(&dir.0).expect("scan the probe project");
            let guid = index
                .guid_for_path(CURRENT_PATH)
                .expect("the scan must give the probe asset an identity")
                .to_string();
            Probe {
                _dir: dir,
                index,
                guid,
            }
        }

        /// Spawns a one-entity scene whose `gltf:` is spelled `gltf_ron`, with
        /// `index` published or not, and reports where it ended up plus every
        /// warning that reached the developer.
        fn spawn(file: &str, gltf_ron: &str, index: Option<AssetIndex>) -> (Vec<String>, String) {
            let scene = format!(
                r#"SceneDescriptor(entities: [EntityDescriptor(name: "Fox", gltf: Some({gltf_ron}))])"#
            );
            let path = write_temp_scene(file, &scene);

            let mut app = new_app();
            if let Some(index) = index {
                app.insert_resource(index);
            }
            app.add_plugins(ScenePlugin::from_file(&path));
            let (_, logs) = capture_warnings(|| app.update());

            let mut q = app.world_mut().query::<&bsengine_gltf::GltfAsset>();
            let resolved = q.iter(app.world()).map(|g| g.path.clone()).collect();
            (resolved, logs)
        }

        // Case 1. The whole point of item 30: the asset was renamed, the scene
        // still names the old path, and the identity is what bridges them.
        #[test]
        fn a_known_identity_wins_over_the_path_stored_beside_it() {
            let probe = probe("scene-identity-moved");
            let (resolved, logs) = spawn(
                "test_identity_stale_path.ron",
                &format!(r#"(guid: "{}", path: "{STALE_PATH}")"#, probe.guid),
                Some(probe.index.clone()),
            );

            assert_eq!(
                resolved,
                vec![CURRENT_PATH.to_string()],
                "the index knows where this identity lives now; the stored path \
                 is what renaming broke"
            );
            assert!(
                logs.contains(STALE_PATH),
                "a scene that resolved somewhere other than what it says must \
                 name what it said, or the developer cannot find the reference \
                 to fix. Got: {logs}"
            );
            assert!(
                logs.contains(CURRENT_PATH),
                "and must name what it loaded instead, or the warning reports a \
                 problem with no way to check it. Got: {logs}"
            );
        }

        // The case that makes the *order* load-bearing rather than a
        // preference. Above, the stale path simply is not in the index, so
        // consulting it first and the identity second would reach the same
        // answer. Here another asset has moved into the path the scene names —
        // one rename and one file added, which is an afternoon's work in any
        // project — and the two orders disagree: the identity finds the asset
        // the scene meant, the path finds a different file that resolves
        // perfectly and silently.
        #[test]
        fn an_identity_beats_a_stored_path_that_another_asset_now_occupies() {
            let dir = ProbeDir(std::env::temp_dir().join(unique("scene-identity-occupied")));
            for name in [CURRENT_PATH, STALE_PATH] {
                let file = dir.0.join(name);
                std::fs::create_dir_all(file.parent().expect("parent")).expect("create dirs");
                // Distinct contents, or the scan's orphan recovery would have
                // two indistinguishable files to tell apart.
                std::fs::write(&file, name.as_bytes()).expect("write probe asset");
            }
            let index = scan(&dir.0).expect("scan the probe project");
            let wanted = index
                .guid_for_path(CURRENT_PATH)
                .expect("the scan must identify the asset the scene means");
            assert!(
                index.guid_for_path(STALE_PATH).is_some_and(|g| g != wanted),
                "the point of this test is that the stored path resolves — to \
                 somebody else"
            );

            let (resolved, logs) = spawn(
                "test_identity_occupied_path.ron",
                &format!(r#"(guid: "{wanted}", path: "{STALE_PATH}")"#),
                Some(index),
            );

            assert_eq!(
                resolved,
                vec![CURRENT_PATH.to_string()],
                "resolving the stored path first would load a real, existing, \
                 wrong asset and never say a word about it"
            );
            assert!(
                logs.contains(STALE_PATH) && logs.contains(CURRENT_PATH),
                "and the developer has to be told the scene no longer means \
                 what it says. Got: {logs}"
            );
        }

        // Case 2. The identity is real but nothing in the project carries it —
        // the asset was deleted, or lives somewhere the scan does not look. The
        // stored path is all that is left, and it may well still work.
        #[test]
        fn an_unknown_identity_falls_back_to_the_stored_path_and_says_so() {
            let probe = probe("scene-identity-unknown");
            let orphaned = AssetGuid::new();
            let (resolved, logs) = spawn(
                "test_identity_unknown.ron",
                &format!(r#"(guid: "{orphaned}", path: "{CURRENT_PATH}")"#),
                Some(probe.index.clone()),
            );

            assert_eq!(
                resolved,
                vec![CURRENT_PATH.to_string()],
                "an identity nobody claims must not cost the scene the path it \
                 still has"
            );
            assert!(
                logs.contains(&orphaned.to_string()),
                "the stale identity is the thing to go looking for. Got: {logs}"
            );
            assert!(
                logs.contains(CURRENT_PATH),
                "and the reference it is attached to. Got: {logs}"
            );
        }

        // Case 3. The pre-item-30 form. All ten scenes in `games/` are still
        // spelled this way, so a warning here would fire on every reference of
        // every scene the engine loads and teach everyone to ignore the log.
        #[test]
        fn a_bare_path_resolves_to_itself_without_a_word() {
            let probe = probe("scene-identity-bare");
            let (resolved, logs) = spawn(
                "test_identity_bare_path.ron",
                &format!(r#""{CURRENT_PATH}""#),
                Some(probe.index.clone()),
            );

            assert_eq!(resolved, vec![CURRENT_PATH.to_string()]);
            assert!(
                !logs.contains("Fox"),
                "a bare path is not a problem — it is what every scene in \
                 games/ still contains. Got: {logs}"
            );
        }

        // Case 4. No index at all. The three hosts publish one now, but any
        // app that adds `ScenePlugin` without `AssetIdentityPlugin` does not —
        // most of this module's own tests, `bsengine-app`'s, every other
        // caller — and a scene has to load identically either way. It is also,
        // exactly, the shape the *bug* takes when a host does register the
        // plugin but the two run unordered — hence
        // `a_scene_resolves_against_the_index_the_identity_plugin_publishes`
        // at the end of this module.
        #[test]
        fn without_an_index_both_forms_behave_exactly_like_a_bare_path() {
            let probe = probe("scene-identity-absent");
            for (form, spelling, expected) in [
                ("bare path", format!(r#""{CURRENT_PATH}""#), CURRENT_PATH),
                (
                    "guid pair",
                    // The GUID this index *would* have resolved, so this test
                    // fails rather than passes by accident if the index is
                    // consulted when it should not be.
                    format!(r#"(guid: "{}", path: "{STALE_PATH}")"#, probe.guid),
                    STALE_PATH,
                ),
            ] {
                let (resolved, logs) = spawn(
                    &format!("test_identity_no_index_{}.ron", form.replace(' ', "_")),
                    &spelling,
                    None,
                );

                assert_eq!(
                    resolved,
                    vec![expected.to_string()],
                    "{form}: with no index there is nothing to resolve against, \
                     so the stored path is the answer"
                );
                assert!(
                    !logs.contains("Fox"),
                    "{form}: an index nobody published is the normal state \
                     today, not a fault to report. Got: {logs}"
                );
            }
        }

        // A hand-edited scene file. Distinguished from case 2 because the fixes
        // differ: this one is a typo to correct, that one is an asset to find.
        #[test]
        fn an_identity_that_is_not_a_guid_is_reported_rather_than_swallowed() {
            let probe = probe("scene-identity-malformed");
            let (resolved, logs) = spawn(
                "test_identity_malformed.ron",
                &format!(r#"(guid: "not-a-guid", path: "{CURRENT_PATH}")"#),
                Some(probe.index.clone()),
            );

            assert_eq!(
                resolved,
                vec![CURRENT_PATH.to_string()],
                "a typo in an identity must not cost the scene its path"
            );
            assert!(
                logs.contains("not-a-guid"),
                "the rejected spelling is the thing to correct. Got: {logs}"
            );
        }

        // `script:` is an `AssetRef` too, and a JS file gets a sidecar for the
        // same reason a mesh does — `goal_level1.js` names the next scene by
        // path. A resolution that only reached `gltf:` would leave half of item
        // 30 undone in a way no glTF test could see.
        #[test]
        fn a_script_reference_resolves_by_identity_as_well() {
            let dir = ProbeDir(std::env::temp_dir().join(unique("scene-identity-script")));
            let script = dir.0.join("assets/scripts/player.js");
            std::fs::create_dir_all(script.parent().expect("parent")).expect("create dirs");
            std::fs::write(&script, b"// player").expect("write script");
            let index = scan(&dir.0).expect("scan");
            let guid = index
                .guid_for_path("assets/scripts/player.js")
                .expect("the scan must identify a script")
                .to_string();

            let scene = format!(
                r#"SceneDescriptor(entities: [EntityDescriptor(name: "Player", script: Some((guid: "{guid}", path: "assets/scripts/hero.js")))])"#
            );
            let path = write_temp_scene("test_identity_script.ron", &scene);

            let mut app = new_app();
            app.insert_resource(index);
            app.add_plugins(ScenePlugin::from_file(&path));
            let (_, logs) = capture_warnings(|| app.update());

            let mut q = app.world_mut().query::<(&Name, &ScriptPath)>();
            let found: Vec<_> = q
                .iter(app.world())
                .map(|(n, s)| (n.0.clone(), s.0.clone()))
                .collect();
            assert_eq!(
                found,
                vec![("Player".to_string(), "assets/scripts/player.js".to_string())],
                "a renamed script must follow its identity like a mesh does"
            );
            assert!(
                logs.contains("assets/scripts/hero.js")
                    && logs.contains("assets/scripts/player.js"),
                "and the rename must be reported with both spellings. Got: {logs}"
            );
        }

        // Resolution reads the index once per reference, not once per frame:
        // scene spawn is a `Startup` system, and a warning that repeated every
        // frame would bury the log it exists to appear in.
        #[test]
        fn a_stale_reference_warns_once_rather_than_every_frame() {
            let probe = probe("scene-identity-once");
            let scene = format!(
                r#"SceneDescriptor(entities: [EntityDescriptor(name: "Fox", gltf: Some((guid: "{}", path: "{STALE_PATH}")))])"#,
                probe.guid
            );
            let path = write_temp_scene("test_identity_once.ron", &scene);

            let mut app = new_app();
            app.insert_resource(probe.index.clone());
            app.add_plugins(ScenePlugin::from_file(&path));
            let (_, logs) = capture_warnings(|| {
                for _ in 0..5 {
                    app.update();
                }
            });

            assert_eq!(
                logs.matches(STALE_PATH).count(),
                1,
                "five frames produced more than one warning. Got: {logs}"
            );
        }

        // ---- recovery through a former path (sub-item D) -----------------
        //
        // The last resort, after the identity and after the stored path: a
        // reference naming somewhere an asset used to be, when nothing is
        // there now. Sub-item B can rewrite an identified reference; nothing
        // can rewrite the ten scenes in `games/` that store bare paths, or the
        // paths spelled inside JavaScript string literals, so remembering the
        // move is what reaches them.
        //
        // The move is always made the way a person makes one — rename the file
        // and leave the `.meta` behind, which is what `git mv` and Explorer do
        // — so the scan's own orphan recovery is what writes `former_paths`.
        // A hand-written sidecar would prove only that this module agrees with
        // itself about a field name.

        /// The phrase only a former-path recovery emits, so an assertion that
        /// one did *not* happen cannot be satisfied by some other warning that
        /// happens to name the same file.
        const RECOVERY_PHRASE: &str = "used to live there";

        /// A project whose one asset has been renamed with its sidecar left
        /// behind, then rescanned. `Probe::guid` is the surviving asset's, and
        /// [`STALE_PATH`] is where it used to be.
        fn moved_asset_probe(tag: &str) -> Probe {
            let dir = ProbeDir(std::env::temp_dir().join(unique(tag)));
            let asset = dir.0.join(STALE_PATH);
            std::fs::create_dir_all(asset.parent().expect("probe asset has a parent"))
                .expect("create probe directories");
            std::fs::write(&asset, b"fake glb").expect("write probe asset");
            // Mints `vulpes.glb.meta` where the asset started.
            scan(&dir.0).expect("scan the probe project");
            std::fs::rename(&asset, dir.0.join(CURRENT_PATH)).expect("rename the probe asset");

            let index = scan(&dir.0).expect("rescan the probe project");
            let guid = index
                .guid_for_path(CURRENT_PATH)
                .expect("orphan recovery must have carried the identity across")
                .to_string();
            Probe {
                _dir: dir,
                index,
                guid,
            }
        }

        // The case sub-item C stops one step short of: the identity is stale
        // *and* so is the path. Before this, the warning said "the stored path
        // is all that is left to go on" while the project knew exactly where
        // the asset had gone.
        #[test]
        fn an_unknown_identity_whose_stored_path_was_left_behind_recovers_and_says_so() {
            let probe = moved_asset_probe("scene-former-unknown-guid");
            assert!(
                probe.index.guid_for_former_path(STALE_PATH).is_some(),
                "precondition: the rescan's orphan recovery must have recorded \
                 the move, or this test measures nothing"
            );

            let orphaned = AssetGuid::new();
            let (resolved, logs) = spawn(
                "test_former_unknown_guid.ron",
                &format!(r#"(guid: "{orphaned}", path: "{STALE_PATH}")"#),
                Some(probe.index.clone()),
            );

            assert_eq!(
                resolved,
                vec![CURRENT_PATH.to_string()],
                "neither the identity nor the path answers any more, but the \
                 project remembers the asset leaving that path — falling back to \
                 a path that loads nothing throws that away"
            );
            assert!(
                logs.contains(RECOVERY_PHRASE),
                "recovering through a former path must never be silent: the scene \
                 file still says the old path, so nothing else will ever tell the \
                 developer. Got: {logs}"
            );
            assert!(
                logs.contains(STALE_PATH),
                "the warning must name the reference to fix. Got: {logs}"
            );
            assert!(
                logs.contains(CURRENT_PATH),
                "and where it actually went, or there is no way to check it. \
                 Got: {logs}"
            );
        }

        // The form that matters most in this repository: all ten scenes in
        // `games/` store bare paths, so a recovery reserved for identified
        // references would reach almost nothing that is actually written down.
        #[test]
        fn a_bare_path_the_project_remembers_being_left_recovers_as_well() {
            let probe = moved_asset_probe("scene-former-bare");
            let (resolved, logs) = spawn(
                "test_former_bare.ron",
                &format!(r#""{STALE_PATH}""#),
                Some(probe.index.clone()),
            );

            assert_eq!(
                resolved,
                vec![CURRENT_PATH.to_string()],
                "a bare path has no identity to fall back on, so the former path \
                 is the only thing between a rename and a reference that loads \
                 nothing"
            );
            assert!(
                logs.contains(STALE_PATH) && logs.contains(CURRENT_PATH),
                "and it is still a reference that no longer means what it says. \
                 Got: {logs}"
            );
        }

        // The collision, and the one direction that must never be got wrong: an
        // asset is renamed away, and then something new is created at the name
        // it left. The file that is *there* wins over the memory of the one
        // that left, silently, because nothing is stale about a path that
        // resolves.
        //
        // The newcomer is created after the scan on purpose. `AssetIndex`
        // already refuses a former path an *indexed* asset occupies, so a
        // newcomer the scan saw would be caught a layer up and prove nothing
        // about the layer that has to catch the rest: the index is a `Startup`
        // snapshot, and files appear after it.
        #[test]
        fn a_file_at_the_stored_path_beats_the_memory_of_the_asset_that_left_it() {
            let probe = moved_asset_probe("scene-former-collision");
            let project_dir = probe._dir.0.display().to_string();
            std::fs::write(probe._dir.0.join(STALE_PATH), b"a different glb")
                .expect("write the newcomer");

            let scene = format!(
                r#"SceneDescriptor(entities: [EntityDescriptor(name: "Fox", gltf: Some("{STALE_PATH}"))])"#
            );
            let path = write_temp_scene("test_former_collision.ron", &scene);

            let mut app = new_app();
            app.insert_resource(bsengine_core::ProjectDir(project_dir.clone()));
            app.insert_resource(probe.index.clone());
            app.add_plugins(ScenePlugin::from_file(&path));
            let (_, logs) = capture_warnings(|| app.update());

            let mut q = app.world_mut().query::<&bsengine_gltf::GltfAsset>();
            let resolved: Vec<String> = q.iter(app.world()).map(|g| g.path.clone()).collect();
            assert_eq!(
                resolved,
                vec![format!("{project_dir}/{STALE_PATH}")],
                "a file that exists at the stored path is the asset the scene \
                 asked for; redirecting away from it because of a move recorded \
                 before it existed would load a real, wrong file and say nothing \
                 useful about it"
            );
            assert!(
                !logs.contains(RECOVERY_PHRASE),
                "and nothing is stale here, so there is nothing to report. \
                 Got: {logs}"
            );
        }

        // ---- the ordering everything above rests on ----------------------
        //
        // Every test above hands the index to the app ready-made. No host
        // does that: `AssetIdentityPlugin` publishes it, from the same
        // `Startup` schedule `ScenePlugin` spawns from. "Both in `Startup`"
        // is not an order, and getting it wrong has no symptom — a spawn that
        // finds no index falls back to the stored path and loads perfectly,
        // so every game still runs, no warning fires, and the feature simply
        // never happens. This is the one test that would notice.
        //
        // Both registration orders are exercised because a host is free to
        // add its plugins in either, and three hosts do. The guarantee has to
        // come from the schedule rather than from the order somebody happened
        // to type into a `main.rs`.
        #[test]
        fn a_scene_resolves_against_the_index_the_identity_plugin_publishes() {
            for identity_first in [true, false] {
                let order = if identity_first {
                    "identity plugin added first"
                } else {
                    "scene plugin added first"
                };

                let dir = ProbeDir(std::env::temp_dir().join(unique("scene-identity-order")));
                let asset = dir.0.join(CURRENT_PATH);
                std::fs::create_dir_all(asset.parent().expect("parent")).expect("create dirs");
                std::fs::write(&asset, b"fake glb").expect("write probe asset");
                // Mint the sidecar first, the way a project that has been
                // scanned once already carries it, and read the identity back
                // out of it: the app below has to reach the same one from the
                // same `.meta` on disk.
                let guid = scan(&dir.0)
                    .expect("scan the probe project")
                    .guid_for_path(CURRENT_PATH)
                    .expect("the scan must identify the probe asset");

                let scene_path = dir.0.join("assets/scenes/main.ron");
                std::fs::create_dir_all(scene_path.parent().expect("parent")).expect("create dirs");
                std::fs::write(
                    &scene_path,
                    format!(
                        r#"SceneDescriptor(entities: [EntityDescriptor(name: "Fox", gltf: Some((guid: "{guid}", path: "{STALE_PATH}")))])"#
                    ),
                )
                .expect("write probe scene");

                let project_dir = dir.0.display().to_string();
                let mut app = new_app();
                app.insert_resource(bsengine_core::ProjectDir(project_dir.clone()));
                let scene = ScenePlugin::from_file(scene_path.to_str().expect("utf-8 probe path"));
                if identity_first {
                    app.add_plugins(AssetIdentityPlugin);
                    app.add_plugins(scene);
                } else {
                    app.add_plugins(scene);
                    app.add_plugins(AssetIdentityPlugin);
                }
                app.update();

                assert!(
                    app.world().get_resource::<AssetIndex>().is_some(),
                    "{order}: no index was published at all, so this test is \
                     measuring nothing"
                );

                let mut q = app.world_mut().query::<&bsengine_gltf::GltfAsset>();
                let resolved: Vec<String> = q.iter(app.world()).map(|g| g.path.clone()).collect();
                assert_eq!(
                    resolved,
                    vec![format!("{project_dir}/{CURRENT_PATH}")],
                    "{order}: the scene spawned before the index it resolves \
                     against existed. That falls back to the stored path in \
                     silence, which is exactly what this whole feature being \
                     inert looks like from the outside"
                );
            }
        }
    }

    /// What `register_gameplay_reflect_types` has to leave behind for the two
    /// glTF components, checked against the registry rather than against the
    /// source text.
    ///
    /// The catalog's R1 gate scans for the *string* `register_type::<SkinnedMesh>`,
    /// so it cannot tell a registration that works from one that compiles: a
    /// type registered without `ReflectComponent` data is in the registry and
    /// still unreachable by `spawn_scene_entities` above, by the Inspector, and
    /// by MCP's `set_reflected_component` — every consumer R1 exists for. This
    /// asserts the thing the gate is a proxy for.
    #[test]
    fn the_gltf_components_register_with_the_data_their_consumers_look_up() {
        let mut app = new_app();
        super::register_gameplay_reflect_types(&mut app);

        let registry = app
            .world()
            .resource::<bevy_ecs::reflect::AppTypeRegistry>()
            .read();

        for name in ["SkinnedMesh", "AnimationClipLibrary"] {
            let type_path = format!("bsengine_gltf::skinned_mesh::{name}");
            let registration = registry.get_with_type_path(&type_path).unwrap_or_else(|| {
                panic!(
                    "{name} is not registered under '{type_path}'. \
                     `spawn_scene_entities` looks types up by exactly this path, \
                     so a component missing here is one a scene's `components:` \
                     list can only report as an unknown type path"
                )
            });
            assert!(
                registration
                    .data::<bevy_ecs::reflect::ReflectComponent>()
                    .is_some(),
                "{name} is registered but carries no `ReflectComponent`, so \
                 nothing can attach or read it reflectively — which is the \
                 whole of what R1 asks for"
            );
        }
    }

    #[test]
    fn scene_plugin_no_transform_when_not_specified() {
        let ron = r#"SceneDescriptor(entities: [EntityDescriptor(name: "Ghost")])"#;
        let path = write_temp_scene("test_no_transform.ron", ron);

        let mut app = new_app();
        app.add_plugins(ScenePlugin::from_file(&path));
        app.update();

        let mut q = app.world_mut().query::<(&Name, &Transform)>();
        assert_eq!(
            q.iter(app.world()).count(),
            0,
            "entity without transform field should have no Transform component"
        );
    }
}
