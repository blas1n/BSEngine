use bevy_ecs::prelude::Component;

/// Detection visibility level — how visible the entity is to enemy AI.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Visibility(pub f32); // 0.0 = fully hidden, 1.0 = fully visible

impl Visibility {
    pub const HIDDEN: Self = Self(0.0);
    pub const FULL: Self = Self(1.0);

    pub fn is_detected(&self, threshold: f32) -> bool {
        self.0 >= threshold
    }
}

/// Stealth state on an entity.
/// The detection system reads `effective_visibility()` and accumulates noise
/// on observer detectors.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stealth {
    /// Base visibility in [0, 1]. 0 = perfectly invisible, 1 = fully visible.
    pub base_visibility: f32,
    /// Additive modifier applied on top of base (e.g. from light, movement).
    pub visibility_modifier: f32,
    /// Noise level produced by movement/actions in [0, 1].
    pub noise_level: f32,
    /// Rate at which noise decays per second when no new noise is added.
    pub noise_decay_rate: f32,
    /// True while the entity is actively sneaking.
    pub sneaking: bool,
    /// Multiplier applied to visibility while sneaking (typically < 1).
    pub sneak_visibility_scale: f32,
    pub enabled: bool,
}

impl Stealth {
    pub fn new(base_visibility: f32) -> Self {
        Self {
            base_visibility: base_visibility.clamp(0.0, 1.0),
            visibility_modifier: 0.0,
            noise_level: 0.0,
            noise_decay_rate: 0.5,
            sneaking: false,
            sneak_visibility_scale: 0.3,
            enabled: true,
        }
    }

    /// A fully visible, non-sneaking entity (no stealth).
    pub fn none() -> Self {
        Self::new(1.0)
    }

    /// A highly stealthy entity.
    pub fn high() -> Self {
        Self::new(0.1)
    }

    pub fn with_sneak_scale(mut self, scale: f32) -> Self {
        self.sneak_visibility_scale = scale.clamp(0.0, 1.0);
        self
    }

    pub fn with_noise_decay(mut self, rate: f32) -> Self {
        self.noise_decay_rate = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Final visibility in [0, 1] after applying modifiers and sneaking.
    pub fn effective_visibility(&self) -> f32 {
        if !self.enabled {
            return 1.0;
        }
        let v = (self.base_visibility + self.visibility_modifier).clamp(0.0, 1.0);
        if self.sneaking {
            (v * self.sneak_visibility_scale).clamp(0.0, 1.0)
        } else {
            v
        }
    }

    /// Add noise (e.g. footstep, gunshot). Clamps to [0, 1].
    pub fn add_noise(&mut self, amount: f32) {
        self.noise_level = (self.noise_level + amount).clamp(0.0, 1.0);
    }

    /// Decay noise over `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.noise_level = (self.noise_level - self.noise_decay_rate * dt).max(0.0);
    }

    pub fn start_sneaking(&mut self) {
        self.sneaking = true;
    }

    pub fn stop_sneaking(&mut self) {
        self.sneaking = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stealth_effective_visibility_base() {
        let s = Stealth::new(0.4);
        assert!((s.effective_visibility() - 0.4).abs() < 0.001);
    }

    #[test]
    fn stealth_sneak_reduces_visibility() {
        let mut s = Stealth::new(1.0).with_sneak_scale(0.25);
        s.start_sneaking();
        assert!((s.effective_visibility() - 0.25).abs() < 0.001);
    }

    #[test]
    fn stealth_disabled_returns_full() {
        let s = Stealth::new(0.0).disabled();
        assert!((s.effective_visibility() - 1.0).abs() < 0.001);
    }

    #[test]
    fn stealth_noise_decays() {
        let mut s = Stealth::new(0.5);
        s.add_noise(1.0);
        s.tick(1.0);
        assert!(s.noise_level < 1.0);
    }

    #[test]
    fn stealth_modifier_clamped() {
        let s = Stealth {
            base_visibility: 0.8,
            visibility_modifier: 0.5,
            ..Stealth::new(0.8)
        };
        assert!(s.effective_visibility() <= 1.0);
    }
}
