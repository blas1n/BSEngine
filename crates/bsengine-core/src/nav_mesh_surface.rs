use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

use crate::nav_mesh::NavBakeParams;

/// Asks for the scene's navigation mesh to be baked from its static colliders.
///
/// The scene-object form every reference engine uses for the same thing:
/// Unity's `NavMeshSurface` component, Godot's `NavigationRegion3D` node,
/// Unreal's `NavMeshBoundsVolume` actor. The bake parameters live on it, and
/// putting it on an entity -- the floor, or an empty "Navigation" entity --
/// is what makes a scene navigable, in place of a script hand-declaring a
/// grid with `Bsengine.navmesh.init`.
///
/// One per scene. The navigation system bakes from the first it finds, and
/// re-bakes whenever these parameters change or the set of static colliders
/// grows or shrinks (a streamed-in chunk, a destroyed wall) -- Unreal's
/// dynamic mode, chosen over Unity's and Godot's explicit-only rebake because
/// it is what makes "bake at load" need no ordering promise with physics:
/// the first frame the colliders exist is the first frame there is a mesh.
/// `Bsengine.navmesh.bake` re-bakes on demand as well.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct NavMeshSurface {
    /// See [`NavBakeParams::agent_radius`].
    pub agent_radius: f32,
    /// See [`NavBakeParams::agent_height`].
    pub agent_height: f32,
    /// See [`NavBakeParams::step_height`].
    pub step_height: f32,
    /// See [`NavBakeParams::floor_y`]. `None` finds the floor automatically.
    pub floor_y: Option<f32>,
    /// Whether the navigation system bakes from this surface on its own.
    /// `false` leaves baking to `Bsengine.navmesh.bake`.
    pub auto_bake: bool,
}

impl Default for NavMeshSurface {
    fn default() -> Self {
        let p = NavBakeParams::default();
        Self {
            agent_radius: p.agent_radius,
            agent_height: p.agent_height,
            step_height: p.step_height,
            floor_y: p.floor_y,
            auto_bake: true,
        }
    }
}

impl NavMeshSurface {
    /// The bake parameters this surface asks for.
    pub fn params(&self) -> NavBakeParams {
        NavBakeParams {
            agent_radius: self.agent_radius,
            agent_height: self.agent_height,
            step_height: self.step_height,
            floor_y: self.floor_y,
        }
    }
}
