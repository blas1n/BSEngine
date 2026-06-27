use bevy_ecs::prelude::Component;

/// Anti-taunt de-aggro field: while active, enemies targeting this entity have
/// a `avoidance_chance` probability of re-selecting a different target instead.
/// The aggro system should call `will_avoid(roll)` with a uniform [0.0, 1.0]
/// roll when resolving each targeting decision.
///
/// `plead(duration)` starts or extends the plea (high-watermark); sets
/// `just_began` on the inactive → active transition. `silence()` ends it
/// early. `tick(dt)` counts down and sets `just_ended` on expiry.
///
/// Distinct from `Taunt` (forces enemies to target this entity), `Stealth`
/// (removes the entity from targeting entirely), and `Demoralize` (reduces
/// enemy combat effectiveness): Plea is a **probabilistic de-aggro field** —
/// it doesn't make the entity invisible, it makes enemies prefer any other
/// target on each targeting roll while the plea is active.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Plea {
    pub duration: f32,
    pub timer: f32,
    /// Probability [0.0, 1.0] that an enemy will avoid targeting this entity.
    pub avoidance_chance: f32,
    pub just_began: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Plea {
    pub fn new(avoidance_chance: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            avoidance_chance: avoidance_chance.clamp(0.0, 1.0),
            just_began: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Begin or extend the plea for `duration` seconds. High-watermark: only
    /// replaces the timer when `duration > timer`. Sets `just_began` on the
    /// inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn plead(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_pleading = self.is_pleading();
            self.duration = duration;
            self.timer = duration;
            if !was_pleading {
                self.just_began = true;
            }
        }
    }

    /// End the plea early. Sets `just_ended`. No-op when not pleading.
    pub fn silence(&mut self) {
        if !self.is_pleading() {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_ended = true;
    }

    /// Advance the plea timer. Sets `just_ended` when the plea expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_began = false;
        self.just_ended = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_ended = true;
            }
        }
    }

    pub fn is_pleading(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` when an enemy should avoid targeting this entity.
    /// `roll` should be a uniform [0.0, 1.0] value supplied by the aggro
    /// system. No-op (returns `false`) when not pleading or disabled.
    pub fn will_avoid(&self, roll: f32) -> bool {
        self.is_pleading() && self.enabled && roll < self.avoidance_chance
    }

    /// Fraction of the plea duration remaining [1.0 = just began, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Plea {
    fn default() -> Self {
        Self::new(0.6)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plead_starts_plea() {
        let mut p = Plea::new(0.6);
        p.plead(5.0);
        assert!(p.is_pleading());
        assert!(p.just_began);
    }

    #[test]
    fn plead_extends_on_longer_duration() {
        let mut p = Plea::new(0.6);
        p.plead(3.0);
        p.tick(0.016);
        p.plead(8.0);
        assert!((p.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn plead_no_extend_on_shorter_duration() {
        let mut p = Plea::new(0.6);
        p.plead(8.0);
        p.plead(3.0);
        assert!((p.timer - 8.0).abs() < 1e-4);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut p = Plea::new(0.6);
        p.plead(3.0);
        p.tick(0.016);
        p.plead(8.0);
        assert!(!p.just_began);
    }

    #[test]
    fn silence_ends_plea() {
        let mut p = Plea::new(0.6);
        p.plead(5.0);
        p.silence();
        assert!(!p.is_pleading());
        assert!(p.just_ended);
    }

    #[test]
    fn silence_no_op_when_not_pleading() {
        let mut p = Plea::new(0.6);
        p.silence();
        assert!(!p.just_ended);
    }

    #[test]
    fn tick_expires_plea() {
        let mut p = Plea::new(0.6);
        p.plead(1.0);
        p.tick(1.1);
        assert!(!p.is_pleading());
        assert!(p.just_ended);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut p = Plea::new(0.6);
        p.plead(5.0);
        p.tick(0.016);
        assert!(!p.just_began);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut p = Plea::new(0.6);
        p.plead(0.5);
        p.tick(1.0);
        p.tick(0.016);
        assert!(!p.just_ended);
    }

    #[test]
    fn will_avoid_below_threshold() {
        let mut p = Plea::new(0.6);
        p.plead(5.0);
        assert!(p.will_avoid(0.59));
    }

    #[test]
    fn will_avoid_at_threshold_false() {
        let mut p = Plea::new(0.6);
        p.plead(5.0);
        assert!(!p.will_avoid(0.6));
    }

    #[test]
    fn will_avoid_false_when_not_pleading() {
        let p = Plea::new(0.9);
        assert!(!p.will_avoid(0.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut p = Plea::new(0.6);
        p.plead(4.0);
        p.tick(2.0);
        assert!((p.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_pleading() {
        let p = Plea::new(0.6);
        assert!((p.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_plead_no_op() {
        let mut p = Plea::new(0.6);
        p.enabled = false;
        p.plead(5.0);
        assert!(!p.is_pleading());
    }

    #[test]
    fn disabled_will_avoid_false() {
        let mut p = Plea::new(0.9);
        p.plead(5.0);
        p.enabled = false;
        assert!(!p.will_avoid(0.0));
    }
}
