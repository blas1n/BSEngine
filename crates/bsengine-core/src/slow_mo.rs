use bevy_ecs::prelude::Component;

/// Source that requested the slow-motion effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlowMoSource {
    /// Triggered by player ability (e.g. bullet-time, focus mode).
    PlayerAbility,
    /// Triggered by environmental hazard or cutscene.
    Environment,
    /// Triggered by a hit-stop effect on damage.
    HitStop,
}

/// Bullet-time / time-dilation component.
///
/// Attach to a "global slow-mo controller" entity or directly to the camera entity.
/// The time system reads `effective_scale()` and multiplies `dt` before passing it to
/// physics and animation — entities *without* `SlowMo` run at full speed if desired.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct SlowMo {
    /// Target time scale [0 = frozen, 1 = normal]. Clamped internally to [0, 4].
    pub target_scale: f32,
    /// Current interpolated time scale (written by the time system).
    pub current_scale: f32,
    /// Lerp speed when easing toward `target_scale` (units per second).
    pub blend_speed: f32,
    /// Maximum duration the effect can stay active (seconds). `None` = unlimited.
    pub max_duration: Option<f32>,
    /// Elapsed time since the effect became active.
    pub elapsed: f32,
    /// What triggered this slow-mo instance.
    pub source: SlowMoSource,
    /// Remaining stamina / resource charge (arbitrary units). Effect auto-ends at 0.
    pub charge: f32,
    /// Rate at which charge drains per real-time second while active.
    pub drain_rate: f32,
    pub enabled: bool,
}

impl SlowMo {
    pub fn new(target_scale: f32, blend_speed: f32) -> Self {
        Self {
            target_scale: target_scale.clamp(0.0, 4.0),
            current_scale: 1.0,
            blend_speed: blend_speed.max(0.0),
            max_duration: None,
            elapsed: 0.0,
            source: SlowMoSource::PlayerAbility,
            charge: f32::INFINITY,
            drain_rate: 0.0,
            enabled: true,
        }
    }

    pub fn with_duration(mut self, secs: f32) -> Self {
        self.max_duration = Some(secs.max(0.0));
        self
    }

    pub fn with_charge(mut self, charge: f32, drain_rate: f32) -> Self {
        self.charge = charge.max(0.0);
        self.drain_rate = drain_rate.max(0.0);
        self
    }

    pub fn with_source(mut self, source: SlowMoSource) -> Self {
        self.source = source;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the effect. `real_dt` is unscaled wall-clock delta time.
    /// Returns true if the effect expired this frame and should be cleaned up.
    pub fn tick(&mut self, real_dt: f32) -> bool {
        if !self.enabled {
            self.current_scale = 1.0;
            return false;
        }

        self.elapsed += real_dt;

        // Drain charge.
        if self.drain_rate > 0.0 {
            self.charge = (self.charge - self.drain_rate * real_dt).max(0.0);
        }

        let expired_duration = self.max_duration.map_or(false, |d| self.elapsed >= d);
        let out_of_charge = self.drain_rate > 0.0 && self.charge <= 0.0;

        if expired_duration || out_of_charge {
            // Snap back toward normal speed.
            self.target_scale = 1.0;
        }

        // Blend current toward target.
        let diff = self.target_scale - self.current_scale;
        let delta = diff.signum() * (self.blend_speed * real_dt).min(diff.abs());
        self.current_scale = (self.current_scale + delta).clamp(0.0, 4.0);

        // Signal expired once blended back to 1.
        (expired_duration || out_of_charge) && (self.current_scale - 1.0).abs() < 0.01
    }

    /// The time scale callers should apply to their `dt`.
    pub fn effective_scale(&self) -> f32 {
        if self.enabled {
            self.current_scale
        } else {
            1.0
        }
    }

    pub fn is_slowing(&self) -> bool {
        self.enabled && self.current_scale < 0.999
    }

    /// Fraction of charge remaining [0, 1] or 1 if unlimited.
    pub fn charge_fraction(&self, max_charge: f32) -> f32 {
        if max_charge <= 0.0 || self.charge.is_infinite() {
            1.0
        } else {
            (self.charge / max_charge).clamp(0.0, 1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blends_toward_target() {
        let mut s = SlowMo::new(0.2, 10.0);
        s.tick(0.1); // blend by 10 * 0.1 = 1.0, capped to diff = 0.8
        assert!((s.current_scale - 0.2).abs() < 0.01);
    }

    #[test]
    fn effective_scale_normal_when_disabled() {
        let s = SlowMo::new(0.1, 5.0).disabled();
        assert!((s.effective_scale() - 1.0).abs() < 1e-6);
    }

    #[test]
    fn duration_resets_target_to_normal() {
        let mut s = SlowMo::new(0.2, 100.0).with_duration(1.0);
        s.tick(1.5);
        assert!((s.target_scale - 1.0).abs() < 1e-6);
    }

    #[test]
    fn charge_drains_over_time() {
        let mut s = SlowMo::new(0.5, 100.0).with_charge(10.0, 5.0);
        s.tick(0.5); // drains 2.5
        assert!((s.charge - 7.5).abs() < 1e-5);
    }

    #[test]
    fn is_slowing_true_below_one() {
        let mut s = SlowMo::new(0.3, 100.0);
        s.tick(1.0);
        assert!(s.is_slowing());
    }

    #[test]
    fn charge_fraction_clamped() {
        let s = SlowMo::new(0.5, 5.0).with_charge(5.0, 1.0);
        assert!((s.charge_fraction(10.0) - 0.5).abs() < 1e-6);
    }
}
