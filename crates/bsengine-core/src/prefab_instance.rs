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
/// something meaningful to attach by hand. `Parent`/`Follow`/`LookAt`/`Tween`
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
