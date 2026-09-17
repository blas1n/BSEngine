//! A texture projected onto whatever geometry sits inside a box.
//!
//! Bullet holes, blood, tyre marks, graffiti — the things a game paints onto a
//! level after it is built. Unity spells this `DecalProjector`, Unreal
//! `DecalActor`, Godot `Decal`; all three are a box with a texture, and all
//! three fade the projection out as the receiving surface turns away from it.
//! This is that same shape.
//!
//! The entity's `Transform` places and orients the box. Projection runs down
//! the box's **local -Y**, the direction a decal placed with its default
//! rotation sprays at a floor.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Largest number of decals drawn in one frame, matching what the renderer
/// will draw before it stops.
///
/// Over-specifying is clamped rather than rejected, the same way
/// [`LightProbeVolume`](crate::LightProbeVolume) clamps its probe count: a
/// scene with too many decals should still render.
pub const MAX_DECALS: usize = 64;

/// A texture projected onto the geometry inside a box.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Decal {
    /// Texture to project, as a path, the way materials name theirs.
    pub texture_path: String,
    /// Optional normal map, projected alongside the colour.
    ///
    /// Tangent space, in the decal's own frame: the box's local X is the
    /// tangent, its local Z the bitangent, and the projection direction the
    /// normal. A decal carries its own frame, so unlike a material's normal map
    /// this needs no tangents on the receiving mesh -- which is what lets a
    /// decal put a dent in terrain, or in anything else that has none.
    ///
    /// Empty means colour only, which is what every decal authored before this
    /// existed says.
    pub normal_map_path: String,
    /// Full size of the projection box along each local axis.
    ///
    /// Full size rather than half-extents because that is the number an author
    /// measures — "this scorch mark is two metres across" — and it is what
    /// Unity's `DecalProjector.size` and Godot's `Decal.size` both mean.
    pub size: ReflectVec3,
    /// How strongly the decal replaces what is under it, `0.0..=1.0`.
    pub opacity: f32,
    /// Below this dot product between the surface normal and the projection
    /// direction, the decal fades out.
    ///
    /// Without it a decal projected at a wall it barely touches smears down the
    /// wall in long streaks, because a box has no idea which surfaces it was
    /// meant for. Unity calls this `angleFade` and Godot `normal_fade`; the
    /// default lets a decal cover a surface up to about 60 degrees off-axis,
    /// which keeps a mark on gently curved ground while cutting the streaks.
    pub normal_fade: f32,
}

impl Default for Decal {
    fn default() -> Self {
        Self {
            texture_path: String::new(),
            normal_map_path: String::new(),
            size: glam::Vec3::new(1.0, 1.0, 1.0).into(),
            opacity: 1.0,
            normal_fade: 0.5,
        }
    }
}

impl Decal {
    /// Opacity clamped to the range that means anything.
    ///
    /// A negative opacity would *add* base colour back through the blend and a
    /// value above one would over-saturate it — both silently, since neither
    /// produces an error anywhere down the pipeline.
    pub fn clamped_opacity(&self) -> f32 {
        self.opacity.clamp(0.0, 1.0)
    }

    /// Size with every axis at least a hair above zero.
    ///
    /// A zero axis makes the box matrix singular, and inverting it gives
    /// infinities that turn every pixel on screen into "inside the box".
    pub fn clamped_size(&self) -> glam::Vec3 {
        let s = *self.size;
        glam::Vec3::new(
            s.x.abs().max(1e-4),
            s.y.abs().max(1e-4),
            s.z.abs().max(1e-4),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_decal_projects_colour_only_until_a_normal_map_is_named() {
        // The property that lets every decal authored before normal maps
        // existed keep rendering exactly as it did.
        assert!(Decal::default().normal_map_path.is_empty());
    }

    #[test]
    fn the_default_decal_is_a_one_metre_box_at_full_opacity() {
        let d = Decal::default();
        assert_eq!(*d.size, glam::Vec3::ONE);
        assert_eq!(d.opacity, 1.0);
        assert!(
            d.texture_path.is_empty(),
            "nothing to project until authored"
        );
    }

    #[test]
    fn a_nonsense_opacity_is_clamped_rather_than_passed_on() {
        // Neither of these produces an error anywhere downstream; they just
        // make the blend do something nobody asked for.
        assert_eq!(
            Decal {
                opacity: -1.0,
                ..Default::default()
            }
            .clamped_opacity(),
            0.0
        );
        assert_eq!(
            Decal {
                opacity: 4.0,
                ..Default::default()
            }
            .clamped_opacity(),
            1.0
        );
    }

    #[test]
    fn a_zero_sized_axis_is_widened_so_the_box_stays_invertible() {
        // A singular box matrix inverts to infinities, and "is this point
        // inside the box" then answers yes for the whole screen.
        let d = Decal {
            size: glam::Vec3::new(2.0, 0.0, 3.0).into(),
            ..Default::default()
        };
        let s = d.clamped_size();
        assert!(s.y > 0.0, "{s:?}");
        assert_eq!(s.x, 2.0, "the axes that were fine must be left alone");
        assert_eq!(s.z, 3.0);
    }

    #[test]
    fn a_negative_size_is_read_as_its_magnitude() {
        // A mirrored box is the same box; a negative extent would otherwise
        // make "inside" and "outside" swap.
        let d = Decal {
            size: glam::Vec3::new(-2.0, 1.0, 1.0).into(),
            ..Default::default()
        };
        assert_eq!(d.clamped_size().x, 2.0);
    }
}
