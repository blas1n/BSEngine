use bevy_ecs::prelude::Component;
use bsengine_ecs::Resource;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Component, Clone, Default)]
pub struct Tags(pub Vec<String>);

#[derive(Component, Clone)]
pub struct Visible(pub bool);

impl Default for Visible {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Clone, Default, Debug)]
pub struct EntityInfo {
    pub id: u64,
    pub name: Option<String>,
    pub position: Option<[f32; 3]>,
    pub mesh_id: Option<u64>,
    pub primitive: Option<bsengine_scene::Primitive>,
    pub script_path: Option<String>,
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
    pub light_type: Option<String>,
    pub light_color: Option<[f32; 3]>,
    pub light_intensity: Option<f32>,
    pub light_range: Option<f32>,
    pub light_ambient: Option<[f32; 3]>,
    pub spot_inner_angle: Option<f32>,
    pub spot_outer_angle: Option<f32>,
    pub camera_fov: Option<f32>,
    pub material_base_color: Option<[f32; 3]>,
    pub material_metallic: Option<f32>,
    pub material_roughness: Option<f32>,
    pub material_emissive: Option<[f32; 3]>,
    pub parent_id: Option<u64>,
    pub tags: Vec<String>,
    pub visible: bool,
    pub selected: bool,
}

#[derive(Clone, Default)]
pub struct EditorSnapshot {
    pub entities: Vec<EntityInfo>,
}

pub enum EditorCommand {
    SpawnNamed(String),
    SetPosition {
        entity_id: u64,
        x: f32,
        y: f32,
        z: f32,
    },
    Despawn {
        entity_id: u64,
    },
    LoadScene(String),
    SaveScene {
        path: String,
    },
    AttachMeshRenderer {
        entity_id: u64,
        mesh_id: u64,
    },
    DetachMeshRenderer {
        entity_id: u64,
    },
    SpawnPointLight {
        color: [f32; 3],
        intensity: f32,
        range: f32,
        position: [f32; 3],
    },
    SpawnDirectionalLight {
        direction: [f32; 3],
        color: [f32; 3],
        ambient: [f32; 3],
    },
    RemoveLight {
        entity_id: u64,
    },
    UpdatePointLight {
        entity_id: u64,
        color: Option<[f32; 3]>,
        intensity: Option<f32>,
        range: Option<f32>,
    },
    UpdateDirectionalLight {
        entity_id: u64,
        direction: Option<[f32; 3]>,
        color: Option<[f32; 3]>,
        ambient: Option<[f32; 3]>,
    },
    RenameEntity {
        entity_id: u64,
        name: String,
    },
    ClearScene,
    SetVisible {
        entity_id: u64,
        visible: bool,
    },
    TagEntity {
        entity_id: u64,
        tag: String,
    },
    UntagEntity {
        entity_id: u64,
        tag: String,
    },
    SetParent {
        entity_id: u64,
        parent_id: u64,
    },
    RemoveParent {
        entity_id: u64,
    },
    SetRotation {
        entity_id: u64,
        rx: f32,
        ry: f32,
        rz: f32,
    },
    SetScale {
        entity_id: u64,
        sx: f32,
        sy: f32,
        sz: f32,
    },
    SetEntityTransform {
        entity_id: u64,
        position: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
    },
    MoveEntity {
        entity_id: u64,
        dx: f32,
        dy: f32,
        dz: f32,
    },
    DuplicateEntity {
        entity_id: u64,
    },
    SpawnCamera {
        fov_y_degrees: f32,
        position: [f32; 3],
    },
    UpdateCamera {
        entity_id: u64,
        fov_y_degrees: Option<f32>,
    },
    BatchSpawn {
        entries: Vec<(String, Option<[f32; 3]>)>,
    },
    SpawnSpotLight {
        color: [f32; 3],
        intensity: f32,
        range: f32,
        inner_angle: f32,
        outer_angle: f32,
        position: [f32; 3],
    },
    UpdateSpotLight {
        entity_id: u64,
        color: Option<[f32; 3]>,
        intensity: Option<f32>,
        range: Option<f32>,
        inner_angle: Option<f32>,
        outer_angle: Option<f32>,
    },
    AttachPointLight {
        entity_id: u64,
        color: [f32; 3],
        intensity: f32,
        range: f32,
    },
    AttachCamera {
        entity_id: u64,
        fov_y_degrees: f32,
    },
    UpdateMaterial {
        entity_id: u64,
        base_color: Option<[f32; 3]>,
        metallic: Option<f32>,
        roughness: Option<f32>,
        emissive: Option<[f32; 3]>,
    },
}

/// Undo/redo checkpoint stacks. Each entry is a full `EditorSnapshot` taken
/// just before an `EditorCommand` batch was applied, so undo/redo restores
/// state by diffing+reconciling against a target snapshot rather than
/// replaying inverse commands.
#[derive(Default)]
pub struct EditorHistory {
    pub undo_stack: Vec<EditorSnapshot>,
    pub redo_stack: Vec<EditorSnapshot>,
}

pub type SharedSnapshot = Arc<Mutex<EditorSnapshot>>;
pub type SharedCommandQueue = Arc<Mutex<Vec<EditorCommand>>>;
pub type SharedSelection = Arc<Mutex<HashSet<u64>>>;
pub type SharedHistory = Arc<Mutex<EditorHistory>>;

#[derive(Resource)]
pub struct EditorSnapshotResource(pub SharedSnapshot);

#[derive(Resource)]
pub struct EditorCommandQueueResource(pub SharedCommandQueue);

#[derive(Resource)]
pub struct EditorSelectionResource(pub SharedSelection);

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
