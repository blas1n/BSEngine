use bevy_ecs::prelude::Component;

#[derive(Component, Debug, Clone)]
pub struct MeshRenderer {
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
