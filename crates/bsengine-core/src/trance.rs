use bevy_ecs::prelude::Component;

/// Mesmerized or meditative state that immobilizes the entity but boosts regeneration.
///
/// While in trance, the entity cannot move or act. The regeneration pipeline
/// should multiply its regen rate by `regen_multiplier()`. Used for both
/// hostile CC (mesmerize, hypnosis) and voluntary abilities (meditation).
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_exited` when the trance ends.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Trance {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to the entity's regeneration while in trance.
    /// e.g. 3.0 = entity regens at triple speed. Must be >= 1.0.
    pub regen_multiplier: f32,
    pub just_entered: bool,
    pub just_exited: bool,
    pub enabled: bool,
}

impl Trance {
    pub fn new(regen_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            regen_multiplier: regen_multiplier.max(1.0),
            just_entered: false,
            just_exited: false,
            enabled: true,
        }
    }

    /// Apply or extend the trance for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_entered = true;
            }
        }
    }

    /// Break the trance immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_exited = true;
        }
    }

    /// Advance the timer; sets `just_exited` when the trance ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered = false;
        self.just_exited = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_exited = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Regen multiplier to apply while in trance; `1.0` when inactive.
    pub fn effective_regen_multiplier(&self) -> f32 {
        if self.is_active() {
            self.regen_multiplier
        } else {
            1.0
        }
    }

    /// Fraction of the trance duration remaining [1.0 = just entered, 0.0 = exited].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Trance {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_trance() {
        let mut t = Trance::new(2.0);
        t.apply(3.0);
        assert!(t.is_active());
        assert!(t.just_entered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut t = Trance::new(2.0);
        t.apply(2.0);
        t.tick(0.016);
        t.apply(5.0);
        assert!((t.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut t = Trance::new(2.0);
        t.apply(5.0);
        t.apply(2.0);
        assert!((t.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_trance() {
        let mut t = Trance::new(2.0);
        t.apply(1.0);
        t.tick(1.1);
        assert!(!t.is_active());
        assert!(t.just_exited);
    }

    #[test]
    fn clear_ends_early() {
        let mut t = Trance::new(2.0);
        t.apply(5.0);
        t.clear();
        assert!(!t.is_active());
        assert!(t.just_exited);
    }

    #[test]
    fn effective_regen_multiplier_while_active() {
        let mut t = Trance::new(3.0);
        t.apply(5.0);
        assert!((t.effective_regen_multiplier() - 3.0).abs() < 1e-5);
    }

    #[test]
    fn effective_regen_multiplier_when_inactive() {
        let t = Trance::new(3.0);
        assert!((t.effective_regen_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn regen_multiplier_clamped_to_one() {
        let t = Trance::new(0.5); // < 1.0 → clamped to 1.0
        assert!((t.regen_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut t = Trance::new(2.0);
        t.apply(2.0);
        t.tick(1.0);
        assert!((t.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut t = Trance::new(2.0);
        t.enabled = false;
        t.apply(5.0);
        assert!(!t.is_active());
    }

    #[test]
    fn tick_clears_just_entered() {
        let mut t = Trance::new(2.0);
        t.apply(3.0);
        t.tick(0.016);
        assert!(!t.just_entered);
    }
}
