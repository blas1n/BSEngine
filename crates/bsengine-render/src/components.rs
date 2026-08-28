use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::Reflect;
use bsengine_core::ReflectVec3;

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

/// Extra, lower-detail mesh alternatives for an entity, switched by camera
/// distance. LOD 0 (highest detail) is never duplicated here -- it's
/// whatever `MeshRenderer.mesh_id` already is; this component only adds
/// the alternatives and the switching state. An entity with no
/// `LodLevels` renders exactly as it always has.
///
/// Public and reflected (R1: every public `#[derive(Component)]` type
/// must be registered) because it is genuinely cross-crate: `bsengine-gltf`
/// and `bsengine-app` both construct it, `bsengine-render`'s `render_frame`
/// queries and mutates it directly every frame.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct LodLevels {
    /// GPU mesh ids for LOD 1, LOD 2, ... in nearest-to-farthest order.
    pub mesh_ids: Vec<u64>,
    /// Camera distance at which each transition occurs:
    /// `switch_distances[0]` is LOD0->LOD1, `switch_distances[1]` is
    /// LOD1->LOD2, etc. Same length as `mesh_ids`.
    pub switch_distances: Vec<f32>,
    /// Width of the dead zone around each `switch_distances` entry -- see
    /// `select_lod_level`'s doc comment for why this prevents popping.
    pub hysteresis_band: f32,
    /// Currently selected level: `None` = LOD 0 (`MeshRenderer.mesh_id`),
    /// `Some(i)` = `mesh_ids[i]`. Updated once per frame by `render_frame`.
    pub current_index: Option<usize>,
}

/// Marks an entity as able to hide other entities behind it, by giving a
/// box that occlusion culling may rasterize as an opaque blocker.
///
/// **The box must fit INSIDE the entity's real geometry.** It is a
/// conservative under-estimate, never a bounding box. This is the one
/// input that can make occlusion culling hide something genuinely
/// visible: rasterizing a blocker larger than the real geometry claims
/// coverage the geometry does not have. Everything else in
/// `crate::occlusion` is built to under-cull; this component is where that
/// guarantee is either kept or broken.
///
/// An entity without an `Occluder` hides nothing. It can still *be*
/// hidden -- being an occluder and being a culling candidate are
/// independent.
///
/// Public and reflected (R1: every public `#[derive(Component)]` type must
/// be registered) because it is genuinely cross-crate: `bsengine-app`
/// constructs it for terrain chunks, scenes author it by reflection, and
/// `bsengine-render`'s `render_frame` queries it every frame.
#[derive(Component, Debug, Clone, Copy, Reflect)]
#[reflect(Component)]
pub struct Occluder {
    /// Center of the box in the entity's LOCAL space. Not always the local
    /// origin: a terrain chunk's mesh spans local `(0,0)..(chunk_size,
    /// chunk_size)`, so its box has to be offset to sit under the geometry
    /// it stands in for.
    pub center: ReflectVec3,
    /// Half-extents of the box in the entity's LOCAL space.
    pub half_extents: ReflectVec3,
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

    #[test]
    fn lod_levels_stores_its_fields() {
        let lod = LodLevels {
            mesh_ids: vec![7, 8],
            switch_distances: vec![10.0, 30.0],
            hysteresis_band: 2.0,
            current_index: Some(1),
        };
        assert_eq!(lod.mesh_ids, vec![7, 8]);
        assert_eq!(lod.current_index, Some(1));
    }

    #[test]
    fn occluder_stores_its_box() {
        let o = Occluder {
            center: glam::Vec3::new(1.0, 2.0, 3.0).into(),
            half_extents: glam::Vec3::new(4.0, 5.0, 6.0).into(),
        };
        assert_eq!(*o.center, glam::Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(*o.half_extents, glam::Vec3::new(4.0, 5.0, 6.0));
    }
}
