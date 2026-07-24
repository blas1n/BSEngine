use bevy_ecs::prelude::Component;
use bsengine_ecs::Resource;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

/// Free-form string labels attached to an entity, set/cleared via
/// `EditorCommand::TagEntity`/`UntagEntity` and used for selection/filtering.
#[derive(Component, Clone, Default)]
pub struct Tags(pub Vec<String>);

/// Flattened, per-entity view of the ECS world used to build an
/// `EditorSnapshot`; every field is `Option`/absent when the entity has no
/// corresponding component, so the MCP layer can serialize it directly.
#[derive(Clone, Default, Debug)]
pub struct EntityInfo {
    /// Entity id, as assigned by the ECS.
    pub id: u64,
    /// Display name, if the entity has a `Name` component.
    pub name: Option<String>,
    /// World-space translation, if the entity has a `Transform`.
    pub position: Option<[f32; 3]>,
    /// Id of the attached mesh asset, if the entity has a `MeshRenderer`.
    pub mesh_id: Option<u64>,
    /// Procedural primitive shape attached to the entity, if any.
    pub primitive: Option<bsengine_scene::Primitive>,
    /// Path to the attached gameplay script, if the entity has one.
    pub script_path: Option<String>,
    /// Euler rotation in degrees, if the entity has a `Transform`.
    pub rotation: Option<[f32; 3]>,
    /// Per-axis scale, if the entity has a `Transform`.
    pub scale: Option<[f32; 3]>,
    /// Kind of light attached (e.g. "point", "directional", "spot"), if any.
    pub light_type: Option<String>,
    /// Light color, if the entity has a light component.
    pub light_color: Option<[f32; 3]>,
    /// Light intensity, if the entity has a point or spot light.
    pub light_intensity: Option<f32>,
    /// Light falloff range, if the entity has a point or spot light.
    pub light_range: Option<f32>,
    /// Ambient color contribution, if the entity has a directional light.
    pub light_ambient: Option<[f32; 3]>,
    /// Inner cone angle in degrees, if the entity has a spot light.
    pub spot_inner_angle: Option<f32>,
    /// Outer cone angle in degrees, if the entity has a spot light.
    pub spot_outer_angle: Option<f32>,
    /// Vertical field of view in degrees, if the entity has a `Camera`.
    pub camera_fov: Option<f32>,
    /// PBR base color, if the entity has a `Material`.
    pub material_base_color: Option<[f32; 3]>,
    /// PBR metallic factor, if the entity has a `Material`.
    pub material_metallic: Option<f32>,
    /// PBR roughness factor, if the entity has a `Material`.
    pub material_roughness: Option<f32>,
    /// Emissive color, if the entity has a `Material`.
    pub material_emissive: Option<[f32; 3]>,
    /// Id of the parent entity, if this entity is attached to one.
    pub parent_id: Option<u64>,
    /// Tags currently attached via `Tags`.
    pub tags: Vec<String>,
    /// Whether the entity is currently rendered (mirrors `Visible`).
    pub visible: bool,
    /// Whether the entity is in the editor's current selection set.
    pub selected: bool,
}

/// Full flattened capture of every tracked entity in the world, taken once
/// per frame and read by the MCP layer/UI without needing ECS access.
#[derive(Clone, Default)]
pub struct EditorSnapshot {
    /// Per-entity info captured this frame.
    pub entities: Vec<EntityInfo>,
}

/// Editor mutation commands, queued via `EditorCommandQueueResource` and
/// drained/applied to the world once per frame by `process_editor_commands`.
pub enum EditorCommand {
    /// Spawn a new named, empty entity at the origin.
    SpawnNamed(String),
    /// Set an entity's world-space translation.
    SetPosition {
        /// Entity to move.
        entity_id: u64,
        /// New X coordinate.
        x: f32,
        /// New Y coordinate.
        y: f32,
        /// New Z coordinate.
        z: f32,
    },
    /// Remove an entity from the world entirely.
    Despawn {
        /// Entity to despawn.
        entity_id: u64,
    },
    /// Replace the current world contents with the scene at this path.
    LoadScene(String),
    /// Spawn a named entity with a `GltfAsset` component so the existing
    /// `bsengine-gltf` async loader picks it up. See
    /// `InspectorCmd::SpawnMeshAsset`'s doc comment for the full rationale.
    SpawnMeshAsset {
        /// Name for the new entity.
        name: String,
        /// Path to the glTF asset to load.
        path: String,
    },
    /// Serialize the current scene to disk at this path.
    SaveScene {
        /// Destination file path.
        path: String,
    },
    /// Attach a `MeshRenderer` referencing an already-loaded mesh asset.
    AttachMeshRenderer {
        /// Entity to attach the renderer to.
        entity_id: u64,
        /// Id of the mesh asset to reference.
        mesh_id: u64,
    },
    /// Remove an entity's `MeshRenderer`, if any.
    DetachMeshRenderer {
        /// Entity to detach the renderer from.
        entity_id: u64,
    },
    /// Spawn a new point light entity.
    SpawnPointLight {
        /// Light color.
        color: [f32; 3],
        /// Light intensity.
        intensity: f32,
        /// Falloff range.
        range: f32,
        /// World-space position to spawn at.
        position: [f32; 3],
    },
    /// Spawn a new directional light entity.
    SpawnDirectionalLight {
        /// Light direction vector.
        direction: [f32; 3],
        /// Light color.
        color: [f32; 3],
        /// Ambient color contribution.
        ambient: [f32; 3],
    },
    /// Remove whichever light component is attached to an entity.
    RemoveLight {
        /// Entity to remove the light from.
        entity_id: u64,
    },
    /// Patch fields on an entity's existing `PointLight`; absent fields are
    /// left unchanged.
    UpdatePointLight {
        /// Entity whose point light to update.
        entity_id: u64,
        /// New color, if changed.
        color: Option<[f32; 3]>,
        /// New intensity, if changed.
        intensity: Option<f32>,
        /// New falloff range, if changed.
        range: Option<f32>,
    },
    /// Patch fields on an entity's existing `DirectionalLight`; absent
    /// fields are left unchanged.
    UpdateDirectionalLight {
        /// Entity whose directional light to update.
        entity_id: u64,
        /// New direction, if changed.
        direction: Option<[f32; 3]>,
        /// New color, if changed.
        color: Option<[f32; 3]>,
        /// New ambient contribution, if changed.
        ambient: Option<[f32; 3]>,
    },
    /// Set (or add) an entity's `Name` component.
    RenameEntity {
        /// Entity to rename.
        entity_id: u64,
        /// New display name.
        name: String,
    },
    /// Despawn every entity in the world.
    ClearScene,
    /// Toggle an entity's `Visible` component.
    SetVisible {
        /// Entity to update.
        entity_id: u64,
        /// New visibility state.
        visible: bool,
    },
    /// Add a tag to an entity's `Tags` component.
    TagEntity {
        /// Entity to tag.
        entity_id: u64,
        /// Tag to add.
        tag: String,
    },
    /// Remove a tag from an entity's `Tags` component.
    UntagEntity {
        /// Entity to untag.
        entity_id: u64,
        /// Tag to remove.
        tag: String,
    },
    /// Attach an entity as a child of another entity.
    SetParent {
        /// Child entity.
        entity_id: u64,
        /// Entity to parent to.
        parent_id: u64,
    },
    /// Detach an entity from its parent, if any.
    RemoveParent {
        /// Entity to unparent.
        entity_id: u64,
    },
    /// Set an entity's Euler rotation, in degrees.
    SetRotation {
        /// Entity to rotate.
        entity_id: u64,
        /// New rotation around X.
        rx: f32,
        /// New rotation around Y.
        ry: f32,
        /// New rotation around Z.
        rz: f32,
    },
    /// Set an entity's per-axis scale.
    SetScale {
        /// Entity to scale.
        entity_id: u64,
        /// New scale on X.
        sx: f32,
        /// New scale on Y.
        sy: f32,
        /// New scale on Z.
        sz: f32,
    },
    /// Patch an entity's `Transform`; absent fields are left unchanged.
    SetEntityTransform {
        /// Entity to update.
        entity_id: u64,
        /// New position, if changed.
        position: Option<[f32; 3]>,
        /// New rotation, if changed.
        rotation: Option<[f32; 3]>,
        /// New scale, if changed.
        scale: Option<[f32; 3]>,
    },
    /// Offset an entity's position by a relative delta.
    MoveEntity {
        /// Entity to move.
        entity_id: u64,
        /// Delta along X.
        dx: f32,
        /// Delta along Y.
        dy: f32,
        /// Delta along Z.
        dz: f32,
    },
    /// Clone an entity and all of its tracked components.
    DuplicateEntity {
        /// Entity to duplicate.
        entity_id: u64,
    },
    /// Spawn a new camera entity.
    SpawnCamera {
        /// Vertical field of view, in degrees.
        fov_y_degrees: f32,
        /// World-space position to spawn at.
        position: [f32; 3],
    },
    /// Patch fields on an entity's existing `Camera`; absent fields are
    /// left unchanged.
    UpdateCamera {
        /// Entity whose camera to update.
        entity_id: u64,
        /// New vertical field of view, if changed.
        fov_y_degrees: Option<f32>,
    },
    /// Spawn multiple named entities in one batch, each with an optional
    /// initial position.
    BatchSpawn {
        /// `(name, position)` pairs to spawn.
        entries: Vec<(String, Option<[f32; 3]>)>,
    },
    /// Spawn a new spot light entity.
    SpawnSpotLight {
        /// Light color.
        color: [f32; 3],
        /// Light intensity.
        intensity: f32,
        /// Falloff range.
        range: f32,
        /// Inner cone angle, in degrees.
        inner_angle: f32,
        /// Outer cone angle, in degrees.
        outer_angle: f32,
        /// World-space position to spawn at.
        position: [f32; 3],
    },
    /// Patch fields on an entity's existing spot light; absent fields are
    /// left unchanged.
    UpdateSpotLight {
        /// Entity whose spot light to update.
        entity_id: u64,
        /// New color, if changed.
        color: Option<[f32; 3]>,
        /// New intensity, if changed.
        intensity: Option<f32>,
        /// New falloff range, if changed.
        range: Option<f32>,
        /// New inner cone angle, if changed.
        inner_angle: Option<f32>,
        /// New outer cone angle, if changed.
        outer_angle: Option<f32>,
    },
    /// Attach a point light component directly to an existing entity.
    AttachPointLight {
        /// Entity to attach the light to.
        entity_id: u64,
        /// Light color.
        color: [f32; 3],
        /// Light intensity.
        intensity: f32,
        /// Falloff range.
        range: f32,
    },
    /// Attach a camera component directly to an existing entity.
    AttachCamera {
        /// Entity to attach the camera to.
        entity_id: u64,
        /// Vertical field of view, in degrees.
        fov_y_degrees: f32,
    },
    /// Attach a gameplay script to an entity.
    AttachScript {
        /// Entity to attach the script to.
        entity_id: u64,
        /// Path to the script file.
        path: String,
    },
    /// Remove an entity's attached gameplay script, if any.
    DetachScript {
        /// Entity to detach the script from.
        entity_id: u64,
    },
    /// Attach a procedural primitive mesh to an entity.
    AttachPrimitiveMesh {
        /// Entity to attach the primitive to.
        entity_id: u64,
        /// Primitive shape to attach.
        primitive: bsengine_scene::Primitive,
    },
    /// Remove an entity's attached primitive mesh, if any.
    DetachPrimitiveMesh {
        /// Entity to detach the primitive from.
        entity_id: u64,
    },
}

/// Undo/redo checkpoint stacks. Each entry is a full `EditorSnapshot` taken
/// just before an `EditorCommand` batch was applied, so undo/redo restores
/// state by diffing+reconciling against a target snapshot rather than
/// replaying inverse commands.
#[derive(Default)]
pub struct EditorHistory {
    /// Snapshots to restore on undo, most recent last.
    pub undo_stack: Vec<EditorSnapshot>,
    /// Snapshots to restore on redo, most recent last.
    pub redo_stack: Vec<EditorSnapshot>,
}

/// Attach/remove a component on an entity by its reflected type path (e.g.
/// "bsengine_core::camera::Camera"), looked up via `AppTypeRegistry`.
/// Processed by a dedicated exclusive system (`process_reflect_commands`)
/// because `ReflectComponent::insert`/`remove` need `&mut EntityWorldMut`,
/// which `process_editor_commands`'s typed system params can't provide.
pub enum ReflectCommand {
    /// Attach a default-constructed instance of the named type as a
    /// component, via `ReflectComponent::insert`.
    AttachComponentByType {
        /// Entity to attach the component to.
        entity_id: u64,
        /// Fully-qualified reflected type path to attach.
        type_path: String,
    },
    /// Remove the named component type from an entity, via
    /// `ReflectComponent::remove`.
    RemoveComponentByType {
        /// Entity to remove the component from.
        entity_id: u64,
        /// Fully-qualified reflected type path to remove.
        type_path: String,
    },
    /// Apply an edited reflected value onto an already-attached component.
    /// Handled in `process_reflect_commands` via
    /// `ReflectComponent::apply_or_insert` (mutates in place if the
    /// component is already attached, which is always the expected case
    /// here — this command only ever originates from editing an
    /// already-cloned, already-attached component's fields).
    ApplyComponentValue {
        /// Entity whose component to update.
        entity_id: u64,
        /// Fully-qualified reflected type path of the component.
        type_path: String,
        /// Edited reflected value to apply onto the component.
        value: Box<dyn bevy_reflect::Reflect>,
    },
}

/// Shared handle to the current frame's `EditorSnapshot`, read by the MCP
/// layer and written once per frame by the editor's snapshot system.
pub type SharedSnapshot = Arc<Mutex<EditorSnapshot>>;
/// Shared handle to the pending `EditorCommand` queue, pushed to by the MCP
/// layer/UI and drained each frame by `process_editor_commands`.
pub type SharedCommandQueue = Arc<Mutex<Vec<EditorCommand>>>;
/// Shared handle to the pending `ReflectCommand` queue, drained each frame
/// by `process_reflect_commands`.
pub type SharedReflectCommandQueue = Arc<Mutex<Vec<ReflectCommand>>>;
/// Shared handle to the set of currently-selected entity ids.
pub type SharedSelection = Arc<Mutex<HashSet<u64>>>;
/// Shared handle to the undo/redo checkpoint stacks.
pub type SharedHistory = Arc<Mutex<EditorHistory>>;

/// Bevy resource exposing the shared snapshot to systems inside the `App`.
#[derive(Resource)]
pub struct EditorSnapshotResource(pub SharedSnapshot);

/// Bevy resource exposing the shared `EditorCommand` queue to systems
/// inside the `App`.
#[derive(Resource)]
pub struct EditorCommandQueueResource(pub SharedCommandQueue);

/// Bevy resource exposing the shared `ReflectCommand` queue to systems
/// inside the `App`.
#[derive(Resource)]
pub struct ReflectCommandQueueResource(pub SharedReflectCommandQueue);

/// Bevy resource exposing the shared selection set to systems inside the
/// `App`.
#[derive(Resource)]
pub struct EditorSelectionResource(pub SharedSelection);

/// Bevy resource exposing the shared undo/redo history to systems inside
/// the `App`.
#[derive(Resource)]
pub struct EditorHistoryResource(pub SharedHistory);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_snapshot_default_is_empty() {
        let s = EditorSnapshot::default();
        assert!(s.entities.is_empty());
    }

    #[test]
    fn entity_info_stores_name_and_position() {
        let e = EntityInfo {
            id: 42,
            name: Some("Player".to_string()),
            position: Some([1.0, 2.0, 3.0]),
            visible: true,
            ..Default::default()
        };
        assert_eq!(e.id, 42);
        assert_eq!(e.name.as_deref(), Some("Player"));
        assert_eq!(e.position, Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn entity_info_without_transform_has_none_position() {
        let e = EntityInfo {
            id: 1,
            visible: true,
            ..Default::default()
        };
        assert!(e.position.is_none());
    }
}
