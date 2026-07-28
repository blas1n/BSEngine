use bevy_ecs::prelude::Component;

/// Marks an entity as drawable and identifies which mesh asset to render it with.
#[derive(Component, Debug, Clone)]
pub struct MeshRenderer {
    /// Id of the mesh asset to draw, as registered with the mesh/asset store.
    pub mesh_id: u64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_renderer_stores_id() {
        let mr = MeshRenderer { mesh_id: 42 };
        assert_eq!(mr.mesh_id, 42);
    }
}
