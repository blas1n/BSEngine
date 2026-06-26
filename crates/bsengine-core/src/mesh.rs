use bevy_ecs::prelude::Component;

/// References a 3D mesh asset for rendering.
#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub struct Mesh {
    /// Asset path to the mesh file (e.g. "models/cube.glb").
    pub path: String,
    /// Index of the submesh within the asset (0 = first/only mesh).
    pub submesh_index: usize,
    /// Whether this mesh casts shadows.
    pub cast_shadow: bool,
    /// Whether this mesh receives shadows cast by others.
    pub receive_shadow: bool,
}

impl Mesh {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            submesh_index: 0,
            cast_shadow: true,
            receive_shadow: true,
        }
    }

    pub fn with_submesh(mut self, index: usize) -> Self {
        self.submesh_index = index;
        self
    }

    pub fn with_cast_shadow(mut self, cast: bool) -> Self {
        self.cast_shadow = cast;
        self
    }

    pub fn with_receive_shadow(mut self, receive: bool) -> Self {
        self.receive_shadow = receive;
        self
    }

    /// Convenience: disable both shadow casting and receiving.
    pub fn no_shadow(mut self) -> Self {
        self.cast_shadow = false;
        self.receive_shadow = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_default_submesh_and_shadows() {
        let m = Mesh::new("models/cube.glb");
        assert_eq!(m.path, "models/cube.glb");
        assert_eq!(m.submesh_index, 0);
        assert!(m.cast_shadow);
        assert!(m.receive_shadow);
    }

    #[test]
    fn mesh_builder_sets_submesh() {
        let m = Mesh::new("models/multi.glb").with_submesh(2);
        assert_eq!(m.submesh_index, 2);
    }

    #[test]
    fn mesh_no_shadow_disables_both() {
        let m = Mesh::new("models/glass.glb").no_shadow();
        assert!(!m.cast_shadow);
        assert!(!m.receive_shadow);
    }

    #[test]
    fn mesh_builder_shadow_overrides() {
        let m = Mesh::new("models/emitter.glb").with_cast_shadow(false);
        assert!(!m.cast_shadow);
        assert!(m.receive_shadow);
    }
}
