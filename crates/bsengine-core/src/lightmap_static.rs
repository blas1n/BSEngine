//! Which renderers a lightmap bake takes in.
//!
//! An explicit opt-in, as all three reference engines make it: Unity's
//! "Contribute Global Illumination" static flag, Godot's `GI Mode: Static`,
//! Unreal's `Mobility: Static`. Inferring it -- "no rigid body, so static" --
//! would bake a crate that a script slides across the room, and its shadow
//! would stay painted where it started.

use crate::reflect_validate::{ReflectValidate, Validate};
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// This entity's mesh is static: it receives a lightmap and occludes and
/// bounces light for every other static mesh in the bake.
///
/// Its mesh must have lightmap UVs (`Vertex::uv2`): the built-in primitives
/// and static glTF models do; a skinned or morphed mesh does not, and the
/// bake refuses it rather than baking garbage.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default, Validate)]
pub struct LightmapStatic {
    /// Multiplies the bake's texel density for this renderer: 2.0 gives it
    /// twice the texels along each side, 0.5 half. Unity's "Scale In
    /// Lightmap", Godot's `lightmap_scale`.
    pub resolution_scale: f32,
}

impl Default for LightmapStatic {
    fn default() -> Self {
        Self {
            resolution_scale: 1.0,
        }
    }
}

impl Validate for LightmapStatic {
    fn validate(&mut self) {
        // A negative scale has no meaning; zero is the bake's smallest square.
        self.resolution_scale = self.resolution_scale.max(0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_is_the_bakes_own_density() {
        assert_eq!(LightmapStatic::default().resolution_scale, 1.0);
    }

    #[test]
    fn a_negative_scale_is_none() {
        let mut s = LightmapStatic {
            resolution_scale: -2.0,
        };
        s.validate();
        assert_eq!(s.resolution_scale, 0.0);
    }
}
