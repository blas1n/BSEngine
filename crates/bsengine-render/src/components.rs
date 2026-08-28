use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::Reflect;

/// Marks an entity as drawable and identifies which mesh asset to render it with.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct MeshRenderer {
    /// Id of the mesh asset to draw, as registered with the mesh/asset store.
    pub mesh_id: u64,
}

/// Marks a terrain chunk entity as ready to draw with the 4-layer splat
/// pipeline instead of the single-texture `Material` pipeline. Attached by
/// `bsengine-app`'s `generate_terrain_chunks` once all 5 of a chunk's
/// textures (4 shared diffuse layers + its own weight map) have uploaded.
///
/// Public and reflected (R1: every public `#[derive(Component)]` type must
/// be registered) because it is genuinely cross-crate API: `bsengine-app`
/// constructs it, `bsengine-render`'s `render_frame` queries it.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct TerrainSplat {
    /// GPU texture id of this chunk's own splat weight map (RGBA8, one
    /// channel per layer), sampled at the chunk-relative UV.
    pub weight_texture_id: u64,
    /// GPU texture ids of the 4 diffuse layers, shared by every chunk of the
    /// same `Terrain` entity, sampled at a world-space tiled UV. Index order
    /// matches `splat_weight_for`'s [grass, rock, dirt, snow] channel order.
    pub layer_texture_ids: [u64; 4],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mesh_renderer_stores_id() {
        let mr = MeshRenderer { mesh_id: 42 };
        assert_eq!(mr.mesh_id, 42);
    }

    #[test]
    fn terrain_splat_stores_its_texture_ids() {
        let ts = TerrainSplat {
            weight_texture_id: 7,
            layer_texture_ids: [1, 2, 3, 4],
        };
        assert_eq!(ts.weight_texture_id, 7);
        assert_eq!(ts.layer_texture_ids, [1, 2, 3, 4]);
    }
}
