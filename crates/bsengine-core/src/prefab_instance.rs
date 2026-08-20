use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::Reflect;

/// Marks an entity as the root of a live prefab instance, recording which
/// prefab file its subtree was instantiated from.
///
/// Attached only to the root entity a prefab instantiation produces — never
/// to its descendants, since a subtree's provenance is fully identified by
/// its root. Reflected and registered like every other gameplay component
/// (see `bsengine_scene::register_gameplay_reflect_types`), so it round-trips
/// through the existing scene-save/`extra_components` mechanism automatically:
/// a scene that is saved and reloaded still knows which of its entities are
/// prefab-instance roots and where they came from.
///
/// Deliberately has no `ReflectDefault` / `#[reflect(Default)]`: unlike most
/// reflected components, this one should never appear in the editor's "Add
/// Component" picker (which only lists `ReflectDefault`-constructible types) —
/// it is a provenance marker `instantiate_prefab` attaches itself, not
/// something meaningful to attach by hand. `Parent`/`Follow`/`LookAt`
/// already establish this same pattern for other components with no sensible
/// default.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct PrefabInstance {
    /// Project-relative path to the prefab file this entity's subtree was
    /// instantiated from, e.g. `"assets/prefabs/turret.ron"` — relative to
    /// `ProjectDir`, in the same spelling `EntityDescriptor::prefab` and
    /// `PrefabInstantiateCommand::path` already use.
    pub source_path: String,
}

/// Records the source prefab file's exact text as of the last successful
/// instantiation or push-sync of this instance, so a later push-sync can
/// tell "the user changed this field" (live value differs from what's
/// recorded here) apart from "this field still matches what the prefab
/// said last time" (safe to overwrite with the new file's value).
///
/// Attached only to the instance root, same lifecycle as [`PrefabInstance`]
/// (see that type's doc comment for why root-only is sufficient). A single
/// `String` field, so `Reflect` derivation is trivial and it round-trips
/// through the scene-save `extra_components` mechanism the same way
/// `PrefabInstance` already does -- no new persistence code, and it
/// survives an editor restart.
///
/// Deliberately has no `ReflectDefault` / `#[reflect(Default)]`, for the
/// same reason `PrefabInstance` doesn't: this is bookkeeping the resync
/// machinery attaches itself, not something meaningful to attach by hand
/// from the Inspector's "Add Component" picker.
#[derive(Component, Clone, Debug, Reflect)]
#[reflect(Component)]
pub struct PrefabInstanceBaseline {
    /// The source prefab file's RON text, exactly as read from disk, at the
    /// last successful sync. Re-parsed on demand by the resync algorithm --
    /// never compared as text, only as the parsed value it represents.
    pub synced_ron: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    #[test]
    fn prefab_instance_baseline_can_be_inserted_and_read_back() {
        let mut world = World::new();
        let entity = world
            .spawn(PrefabInstanceBaseline {
                synced_ron: "PrefabDescriptor(entities: [])".to_string(),
            })
            .id();
        assert_eq!(
            world
                .get::<PrefabInstanceBaseline>(entity)
                .unwrap()
                .synced_ron,
            "PrefabDescriptor(entities: [])"
        );
    }
}
