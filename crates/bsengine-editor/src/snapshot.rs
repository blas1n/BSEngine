use bsengine_ecs::Resource;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct EntityInfo {
    pub id: u64,
    pub name: Option<String>,
}

#[derive(Clone, Default)]
pub struct EditorSnapshot {
    pub entities: Vec<EntityInfo>,
}

pub type SharedSnapshot = Arc<Mutex<EditorSnapshot>>;
pub type SharedSpawnQueue = Arc<Mutex<Vec<String>>>;

#[derive(Resource)]
pub struct EditorSnapshotResource(pub SharedSnapshot);

#[derive(Resource)]
pub struct EditorSpawnQueueResource(pub SharedSpawnQueue);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editor_snapshot_default_is_empty() {
        let s = EditorSnapshot::default();
        assert!(s.entities.is_empty());
    }

    #[test]
    fn entity_info_stores_name() {
        let e = EntityInfo {
            id: 42,
            name: Some("Player".to_string()),
        };
        assert_eq!(e.id, 42);
        assert_eq!(e.name.as_deref(), Some("Player"));
    }
}
