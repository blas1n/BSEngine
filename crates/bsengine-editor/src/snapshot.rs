use bsengine_ecs::Resource;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct EntityInfo {
    pub id: u64,
    pub name: Option<String>,
    pub position: Option<[f32; 3]>,
    pub mesh_id: Option<u64>,
    pub rotation: Option<[f32; 3]>,
    pub scale: Option<[f32; 3]>,
    pub light_type: Option<String>,
    pub light_color: Option<[f32; 3]>,
    pub light_intensity: Option<f32>,
    pub light_range: Option<f32>,
    pub camera_fov: Option<f32>,
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
}

pub type SharedSnapshot = Arc<Mutex<EditorSnapshot>>;
pub type SharedCommandQueue = Arc<Mutex<Vec<EditorCommand>>>;

#[derive(Resource)]
pub struct EditorSnapshotResource(pub SharedSnapshot);

#[derive(Resource)]
pub struct EditorCommandQueueResource(pub SharedCommandQueue);

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
            mesh_id: None,
            light_type: None,
            rotation: None,
            scale: None,
            light_color: None,
            light_intensity: None,
            light_range: None,
            camera_fov: None,
        };
        assert_eq!(e.id, 42);
        assert_eq!(e.name.as_deref(), Some("Player"));
        assert_eq!(e.position, Some([1.0, 2.0, 3.0]));
    }

    #[test]
    fn entity_info_without_transform_has_none_position() {
        let e = EntityInfo {
            id: 1,
            name: None,
            mesh_id: None,
            light_type: None,
            rotation: None,
            scale: None,
            light_color: None,
            light_intensity: None,
            light_range: None,
            camera_fov: None,
            position: None,
        };
        assert!(e.position.is_none());
    }
}
