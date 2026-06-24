use bsengine_ecs::Resource;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct EntityInfo {
    pub id: u64,
    pub name: Option<String>,
    pub position: Option<[f32; 3]>,
    pub mesh_id: Option<u64>,
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
            position: None,
        };
        assert!(e.position.is_none());
    }
}
