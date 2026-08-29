//! Temporal antialiasing settings for a camera.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Temporal antialiasing applied by a camera.
///
/// Accumulates sub-pixel-jittered frames over time, reprojecting the
/// previous frame through the camera's motion, to smooth edges that would
/// otherwise stair-step and crawl.
///
/// **Absent means off.** A camera without this component renders exactly as
/// it always has — which is what lets every existing pixel test keep
/// passing unchanged.
///
/// Reprojection is depth-based and camera-only in this version, so it is
/// exact for static geometry under any camera motion. Moving objects
/// reproject incorrectly and are held in check by `clamp_strength` rather
/// than corrected; per-object motion vectors are a separate piece of work.
#[derive(Component, Debug, Clone, Copy, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct Taa {
    /// Whether the effect is applied at all.
    pub enabled: bool,
    /// Fraction of the reprojected history blended into each pixel, in
    /// `[0, 1)`. This is the quality knob: lower converges in fewer frames
    /// but antialiases less, higher is smoother but ghosts more readily.
    /// 0.9 is the usual starting point and the default here.
    pub history_blend: f32,
    /// How far outside the current frame's local 3x3 colour range the
    /// reprojected history is allowed to sit, as a multiple of that range.
    /// Lower clamps harder: less ghosting, less smoothing. 1.0 clamps to
    /// exactly the neighbourhood.
    pub clamp_strength: f32,
}

impl Default for Taa {
    fn default() -> Self {
        Self {
            enabled: true,
            history_blend: 0.9,
            clamp_strength: 1.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_enabled_and_blends_mostly_history() {
        let t = Taa::default();
        assert!(t.enabled);
        assert!(
            t.history_blend > 0.5 && t.history_blend < 1.0,
            "history_blend must favour history to antialias, but stay below \
             1.0 or the current frame never contributes and the image freezes"
        );
        assert!(t.clamp_strength > 0.0);
    }
}
