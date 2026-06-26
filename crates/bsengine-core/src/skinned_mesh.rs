use bevy_ecs::prelude::{Component, Entity};

/// A mesh that deforms based on a set of `Bone` entities.
/// The render system reads all referenced bone entities, computes their skinning matrices,
/// and uploads them to the GPU for vertex shader deformation.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SkinnedMesh {
    /// Asset path to the skinned mesh (e.g. "characters/hero.glb").
    pub mesh_path: String,
    /// Asset path to the skeleton/rig definition.
    pub skeleton_path: String,
    /// Ordered list of bone entities. Index must match the bone indices in the mesh asset.
    pub bones: Vec<Entity>,
}

impl SkinnedMesh {
    pub fn new(
        mesh_path: impl Into<String>,
        skeleton_path: impl Into<String>,
        bones: Vec<Entity>,
    ) -> Self {
        Self {
            mesh_path: mesh_path.into(),
            skeleton_path: skeleton_path.into(),
            bones,
        }
    }

    pub fn bone_count(&self) -> usize {
        self.bones.len()
    }

    /// Returns the entity at the given bone index, or `None` if out of range.
    pub fn bone(&self, index: usize) -> Option<Entity> {
        self.bones.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_ecs::world::World;

    fn spawn_entities(world: &mut World, count: usize) -> Vec<Entity> {
        (0..count).map(|_| world.spawn_empty().id()).collect()
    }

    #[test]
    fn skinned_mesh_bone_count() {
        let mut world = World::new();
        let bones = spawn_entities(&mut world, 3);
        let sm = SkinnedMesh::new("hero.glb", "hero_skel.glb", bones);
        assert_eq!(sm.bone_count(), 3);
    }

    #[test]
    fn skinned_mesh_bone_lookup() {
        let mut world = World::new();
        let bones = spawn_entities(&mut world, 2);
        let sm = SkinnedMesh::new("hero.glb", "hero_skel.glb", bones.clone());
        assert_eq!(sm.bone(0), Some(bones[0]));
        assert_eq!(sm.bone(1), Some(bones[1]));
        assert_eq!(sm.bone(2), None);
    }

    #[test]
    fn skinned_mesh_empty_bones() {
        let sm = SkinnedMesh::new("mesh.glb", "skel.glb", vec![]);
        assert_eq!(sm.bone_count(), 0);
        assert_eq!(sm.bone(0), None);
    }
}
