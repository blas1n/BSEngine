use bevy_ecs::prelude::Component;

/// Energy-overload debuff that multiplies the resource cost of all abilities.
///
/// While active, ability systems should multiply their mana/energy cost by
/// `effective_cost_multiplier()` (> 1.0). At `cost_multiplier == 2.0` all
/// abilities cost twice as much; at `1.0` there is no effect.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` when the effect expires.
///
/// Distinct from `Overheat` (temperature-based heat damage), `Exhaustion`
/// (reduces resource capacity), and `Drain` (ongoing resource bleed): Overload
/// specifically multiplies ability cast costs without affecting the pool.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Overload {
    pub duration: f32,
    pub timer: f32,
    /// Factor applied to all ability costs while overloaded (must be >= 1.0).
    /// e.g. 2.0 = abilities cost double; 1.5 = 50% more.
    pub cost_multiplier: f32,
    pub just_overloaded: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Overload {
    pub fn new(cost_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            cost_multiplier: cost_multiplier.max(1.0),
            just_overloaded: false,
            just_recovered: false,
            enabled: true,
        }
    }

    /// Apply or extend the overload for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_overloaded = true;
            }
        }
    }

    /// End the overload immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_overloaded = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Cost multiplier to apply to all abilities. Returns `cost_multiplier`
    /// while active, `1.0` otherwise.
    pub fn effective_cost_multiplier(&self) -> f32 {
        if self.is_active() {
            self.cost_multiplier
        } else {
            1.0
        }
    }

    /// Fraction of the overload duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Overload {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_overload() {
        let mut o = Overload::new(2.0);
        o.apply(3.0);
        assert!(o.is_active());
        assert!(o.just_overloaded);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut o = Overload::new(2.0);
        o.apply(2.0);
        o.tick(0.016);
        o.apply(5.0);
        assert!((o.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut o = Overload::new(2.0);
        o.apply(5.0);
        o.apply(2.0);
        assert!((o.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_overload() {
        let mut o = Overload::new(2.0);
        o.apply(1.0);
        o.tick(1.1);
        assert!(!o.is_active());
        assert!(o.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut o = Overload::new(2.0);
        o.apply(5.0);
        o.clear();
        assert!(!o.is_active());
        assert!(o.just_recovered);
    }

    #[test]
    fn effective_cost_multiplier_while_active() {
        let mut o = Overload::new(1.5);
        o.apply(3.0);
        assert!((o.effective_cost_multiplier() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn effective_cost_multiplier_when_inactive() {
        let o = Overload::new(1.5);
        assert!((o.effective_cost_multiplier() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn cost_multiplier_clamped_to_one() {
        let o = Overload::new(0.5);
        assert!((o.cost_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut o = Overload::new(2.0);
        o.apply(2.0);
        o.tick(1.0);
        assert!((o.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut o = Overload::new(2.0);
        o.enabled = false;
        o.apply(5.0);
        assert!(!o.is_active());
    }

    #[test]
    fn tick_clears_just_overloaded() {
        let mut o = Overload::new(2.0);
        o.apply(3.0);
        o.tick(0.016);
        assert!(!o.just_overloaded);
    }
}
