use bevy_ecs::prelude::Component;
use glam::{Quat, Vec3};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EasingFn {
    Linear,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
}

impl EasingFn {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatMode {
    Once,
    Loop,
    PingPong,
}

#[derive(Debug, Clone)]
pub enum TweenTarget {
    Translation { from: Vec3, to: Vec3 },
    Rotation { from: Quat, to: Quat },
    Scale { from: Vec3, to: Vec3 },
}

#[derive(Component, Debug, Clone)]
pub struct Tween {
    pub target: TweenTarget,
    pub duration: f32,
    pub easing: EasingFn,
    pub repeat: RepeatMode,
    pub finished: bool,
    pub elapsed: f32,
    pub reversed: bool,
}

impl Tween {
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

    pub fn with_easing(mut self, easing: EasingFn) -> Self {
        self.easing = easing;
        self
    }

    pub fn with_repeat(mut self, repeat: RepeatMode) -> Self {
        self.repeat = repeat;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
                from: Vec3::ZERO,
                to: Vec3::X,
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
                from: Vec3::ONE,
                to: Vec3::splat(2.0),
            },
            0.5,
        );
        assert_eq!(tw.easing, EasingFn::Linear);
        assert_eq!(tw.repeat, RepeatMode::Once);
        assert!(!tw.finished);
        assert!((tw.elapsed - 0.0).abs() < 1e-6);
    }
}
