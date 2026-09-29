//! Depth of field: blurring what is nearer or farther than the focus.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Blurs the scene by distance from the camera, in two bands: everything
/// past `far_distance` and everything nearer than `near_distance`, each
/// ramping in over its `transition`.
///
/// This is the distance-band model Godot's `CameraAttributesPractical` and
/// Unity's Gaussian depth of field use -- say where the blur starts, not what
/// lens would cause it. Unreal's cinematic DOF and Unity's Bokeh mode derive
/// the same bands from a physical lens (focal length, aperture); a game
/// usually wants the bands directly, and a lens model can be layered on top
/// of them later.
///
/// The blur is a disk gather over the HDR image before tonemapping, as
/// Unity and Unreal place it, so bright highlights blur into bright discs
/// rather than grey smudges. A neighbour contributes only as far as its own
/// blur reaches, so a sharp subject leaves no halo on the blurred background
/// around it, while a blurred foreground still spreads over what is behind.
///
/// Absent means off: a camera without this component renders exactly as it
/// did before.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct DepthOfField {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// Blur what is farther than [`Self::far_distance`].
    pub far_enabled: bool,
    /// World distance from the camera where the far blur begins.
    pub far_distance: f32,
    /// Distance over which the far blur ramps from none to full.
    pub far_transition: f32,
    /// Blur what is nearer than [`Self::near_distance`].
    pub near_enabled: bool,
    /// World distance from the camera where the near blur ends: nearer than
    /// this is blurred, ramping to full over [`Self::near_transition`].
    pub near_distance: f32,
    /// Distance over which the near blur ramps from full to none.
    pub near_transition: f32,
    /// Radius, in pixels at full blur, of the disk a pixel is spread over.
    pub max_radius: f32,
}

impl Default for DepthOfField {
    /// Godot's defaults for the bands (far blur from 10 over 5, near blur
    /// off, starting 2 units out over 1) and an 8-pixel disk.
    fn default() -> Self {
        Self {
            enabled: true,
            far_enabled: true,
            far_distance: 10.0,
            far_transition: 5.0,
            near_enabled: false,
            near_distance: 2.0,
            near_transition: 1.0,
            max_radius: 8.0,
        }
    }
}
