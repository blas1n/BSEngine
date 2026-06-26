use bevy_ecs::prelude::Component;

/// Which joints are driven by animation vs. physics.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RagdollMode {
    /// Skeleton is fully animation-driven; physics ignored.
    Animated,
    /// Full ragdoll: all joints are physics-driven.
    Physics,
    /// Partial blend: joints transition between animation and physics.
    Blended,
}

/// Ragdoll state on a character entity.
/// The physics/animation system reads `mode` each frame to decide how to
/// drive the skeleton.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Ragdoll {
    pub mode: RagdollMode,
    /// Blend factor in [0, 1]. 0 = fully animated, 1 = fully physics.
    /// Only meaningful in `Blended` mode.
    pub blend: f32,
    /// Rate at which `blend` transitions toward `target_blend` per second.
    pub blend_rate: f32,
    /// Target blend value the system is animating toward.
    pub target_blend: f32,
    /// Minimum velocity magnitude for an impact to trigger ragdoll.
    pub impact_threshold: f32,
    /// Time in seconds before returning to animated state. `None` = stay in ragdoll.
    pub recovery_time: Option<f32>,
    /// Accumulated time since ragdoll was triggered.
    pub time_in_ragdoll: f32,
    pub enabled: bool,
}

impl Ragdoll {
    pub fn new() -> Self {
        Self {
            mode: RagdollMode::Animated,
            blend: 0.0,
            blend_rate: 2.0,
            target_blend: 0.0,
            impact_threshold: 5.0,
            recovery_time: Some(3.0),
            time_in_ragdoll: 0.0,
            enabled: true,
        }
    }

    pub fn with_impact_threshold(mut self, threshold: f32) -> Self {
        self.impact_threshold = threshold.max(0.0);
        self
    }

    pub fn with_recovery_time(mut self, seconds: f32) -> Self {
        self.recovery_time = Some(seconds.max(0.0));
        self
    }

    /// Ragdoll stays active until manually reset.
    pub fn no_recovery(mut self) -> Self {
        self.recovery_time = None;
        self
    }

    pub fn with_blend_rate(mut self, rate: f32) -> Self {
        self.blend_rate = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Trigger ragdoll (switch to Physics mode with blend=1 target).
    pub fn trigger(&mut self) {
        self.mode = RagdollMode::Blended;
        self.target_blend = 1.0;
        self.time_in_ragdoll = 0.0;
    }

    /// Advance blend and recovery timer. Call once per physics step.
    /// Returns `true` when recovery completes and mode returns to Animated.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled {
            return false;
        }
        // Advance blend toward target.
        let diff = self.target_blend - self.blend;
        let step = self.blend_rate * dt;
        if diff.abs() <= step {
            self.blend = self.target_blend;
        } else {
            self.blend += step * diff.signum();
        }
        // When fully blended into physics, switch mode.
        if self.blend >= 1.0 {
            self.mode = RagdollMode::Physics;
        }
        // Track recovery.
        if self.mode == RagdollMode::Physics {
            self.time_in_ragdoll += dt;
            if let Some(recovery) = self.recovery_time {
                if self.time_in_ragdoll >= recovery {
                    self.reset();
                    return true;
                }
            }
        }
        false
    }

    /// Return to animated mode.
    pub fn reset(&mut self) {
        self.mode = RagdollMode::Animated;
        self.target_blend = 0.0;
        self.time_in_ragdoll = 0.0;
    }

    pub fn is_active(&self) -> bool {
        matches!(self.mode, RagdollMode::Physics | RagdollMode::Blended)
    }
}

impl Default for Ragdoll {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ragdoll_starts_animated() {
        let r = Ragdoll::new();
        assert_eq!(r.mode, RagdollMode::Animated);
        assert!(!r.is_active());
    }

    #[test]
    fn ragdoll_trigger_activates() {
        let mut r = Ragdoll::new();
        r.trigger();
        assert!(r.is_active());
        assert_eq!(r.target_blend, 1.0);
    }

    #[test]
    fn ragdoll_tick_blends_toward_physics() {
        let mut r = Ragdoll::new().with_blend_rate(1.0);
        r.trigger();
        r.tick(0.5);
        assert!((r.blend - 0.5).abs() < 0.001);
    }

    #[test]
    fn ragdoll_recovery_resets() {
        let mut r = Ragdoll::new()
            .with_recovery_time(1.0)
            .with_blend_rate(100.0);
        r.trigger();
        r.tick(0.01);
        let recovered = r.tick(2.0);
        assert!(recovered);
        assert_eq!(r.mode, RagdollMode::Animated);
    }

    #[test]
    fn ragdoll_disabled_tick_no_op() {
        let mut r = Ragdoll::new().disabled();
        r.trigger();
        let recovered = r.tick(10.0);
        assert!(!recovered);
    }
}
