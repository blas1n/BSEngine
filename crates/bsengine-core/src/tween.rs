use crate::{ReflectQuat, ReflectVec3};
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::Reflect;

/// Easing curve applied to a tween's normalized progress before interpolating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum EasingFn {
    /// Constant rate of change; no easing.
    Linear,
    /// Starts slow, accelerates toward the end.
    EaseInQuad,
    /// Starts fast, decelerates toward the end.
    EaseOutQuad,
    /// Starts slow, speeds up through the middle, ends slow.
    EaseInOutQuad,
}

impl EasingFn {
    /// Maps a linear progress value `t` in `[0, 1]` through this easing curve.
    pub fn apply(self, t: f32) -> f32 {
        match self {
            EasingFn::Linear => t,
            EasingFn::EaseInQuad => t * t,
            EasingFn::EaseOutQuad => t * (2.0 - t),
            EasingFn::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    -1.0 + (4.0 - 2.0 * t) * t
                }
            }
        }
    }
}

/// How a tween behaves once it reaches the end of its duration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Reflect)]
pub enum RepeatMode {
    /// Play once and stop.
    Once,
    /// Restart from the beginning each time it finishes.
    Loop,
    /// Reverse direction each time it finishes, bouncing between start and end.
    PingPong,
}

/// The property being animated and its start/end values.
#[derive(Debug, Clone, Reflect)]
pub enum TweenTarget {
    /// Animates `Transform.translation`.
    Translation {
        /// Starting position.
        from: ReflectVec3,
        /// Ending position.
        to: ReflectVec3,
    },
    /// Animates `Transform.rotation`.
    Rotation {
        /// Starting rotation.
        from: ReflectQuat,
        /// Ending rotation.
        to: ReflectQuat,
    },
    /// Animates `Transform.scale`.
    Scale {
        /// Starting scale.
        from: ReflectVec3,
        /// Ending scale.
        to: ReflectVec3,
    },
}

/// Animates a `Transform` property from one value to another over time.
#[derive(Component, Debug, Clone, Reflect)]
#[reflect(Component)]
pub struct Tween {
    /// Which property is being animated, and its start/end values.
    pub target: TweenTarget,
    /// Total time the tween takes to go from start to end, in seconds.
    pub duration: f32,
    /// Easing curve applied to progress before interpolating.
    pub easing: EasingFn,
    /// Behavior once the tween reaches the end.
    pub repeat: RepeatMode,
    /// Whether a non-repeating tween has completed.
    pub finished: bool,
    /// Time accumulated since the tween started, in seconds.
    pub elapsed: f32,
    /// Whether the tween is currently playing back-to-front (used by `PingPong`).
    pub reversed: bool,
}

impl Tween {
    /// Creates a linear, non-repeating tween for `target` over `duration` seconds.
    pub fn new(target: TweenTarget, duration: f32) -> Self {
        Self {
            target,
            duration,
            easing: EasingFn::Linear,
            repeat: RepeatMode::Once,
            finished: false,
            elapsed: 0.0,
            reversed: false,
        }
    }

    /// Sets the easing curve applied to progress.
    pub fn with_easing(mut self, easing: EasingFn) -> Self {
        self.easing = easing;
        self
    }

    /// Sets the behavior once the tween reaches the end.
    pub fn with_repeat(mut self, repeat: RepeatMode) -> Self {
        self.repeat = repeat;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn easing_linear_identity() {
        assert!((EasingFn::Linear.apply(0.0) - 0.0).abs() < 1e-6);
        assert!((EasingFn::Linear.apply(0.5) - 0.5).abs() < 1e-6);
        assert!((EasingFn::Linear.apply(1.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn easing_ease_in_quad_starts_slow() {
        assert!(EasingFn::EaseInQuad.apply(0.5) < 0.5);
    }

    #[test]
    fn easing_ease_out_quad_ends_slow() {
        assert!(EasingFn::EaseOutQuad.apply(0.5) > 0.5);
    }

    #[test]
    fn easing_ease_in_out_quad_symmetric() {
        let t1 = EasingFn::EaseInOutQuad.apply(0.25);
        let t2 = 1.0 - EasingFn::EaseInOutQuad.apply(0.75);
        assert!((t1 - t2).abs() < 1e-6);
    }

    #[test]
    fn tween_builder_sets_fields() {
        let tw = Tween::new(
            TweenTarget::Translation {
                from: Vec3::ZERO.into(),
                to: Vec3::X.into(),
            },
            1.0,
        )
        .with_easing(EasingFn::EaseOutQuad)
        .with_repeat(RepeatMode::Loop);

        assert_eq!(tw.easing, EasingFn::EaseOutQuad);
        assert_eq!(tw.repeat, RepeatMode::Loop);
        assert!(!tw.finished);
    }

    #[test]
    fn tween_new_defaults() {
        let tw = Tween::new(
            TweenTarget::Scale {
                from: Vec3::ONE.into(),
                to: Vec3::splat(2.0).into(),
            },
            0.5,
        );
        assert_eq!(tw.easing, EasingFn::Linear);
        assert_eq!(tw.repeat, RepeatMode::Once);
        assert!(!tw.finished);
        assert!((tw.elapsed - 0.0).abs() < 1e-6);
    }
}
