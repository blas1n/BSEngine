use bevy_ecs::prelude::Component;

/// Delayed-release melee strike: while `is_winding_up()`, the entity is
/// committed to a powerful blow and cannot cancel. The strike fires when
/// `tick(dt)` returns `true` (natural expiry) or `release()` is called early.
/// `effective_damage(base)` returns the amplified hit while winding up.
///
/// `begin(duration)` starts or extends the windup (high-watermark); sets
/// `just_started` on the inactive → active transition.
///
/// Distinct from `Charge` (power-building over time), `Parry` (defensive
/// reaction window), and `Poise` (interrupt resistance): Windup is a
/// **committed one-shot melee amplifier** — the entity locks into the animation
/// and the damage multiplier applies when the blow lands.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Windup {
    pub duration: f32,
    pub timer: f32,
    /// Damage multiplier applied when the strike lands. Clamped ≥ 1.0.
    pub damage_multiplier: f32,
    pub just_started: bool,
    pub just_released: bool,
    pub enabled: bool,
}

impl Windup {
    pub fn new(damage_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            damage_multiplier: damage_multiplier.max(1.0),
            just_started: false,
            just_released: false,
            enabled: true,
        }
    }

    /// Begin or extend the windup for `duration` seconds. High-watermark: only
    /// replaces the current timer when `duration > timer`. Sets `just_started`
    /// on the inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn begin(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_winding = self.is_winding_up();
            self.duration = duration;
            self.timer = duration;
            if !was_winding {
                self.just_started = true;
            }
        }
    }

    /// Fire the strike early (before the timer expires). Sets `just_released`.
    /// Returns `true` if the entity was winding up, `false` if already idle.
    pub fn release(&mut self) -> bool {
        if !self.is_winding_up() {
            return false;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_released = true;
        true
    }

    /// Advance the windup timer. Returns `true` and sets `just_released` when
    /// the windup completes naturally. Clears one-frame flags at the start of
    /// each tick.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_started = false;
        self.just_released = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_released = true;
                return true;
            }
        }
        false
    }

    pub fn is_winding_up(&self) -> bool {
        self.timer > 0.0
    }

    /// Amplified strike damage while winding up and enabled.
    /// Returns `base * damage_multiplier` while active; `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_winding_up() && self.enabled {
            base * self.damage_multiplier
        } else {
            base
        }
    }

    /// Fraction of the windup remaining [1.0 = just began, 0.0 = complete].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Windup {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_starts_windup() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        assert!(w.is_winding_up());
        assert!(w.just_started);
    }

    #[test]
    fn begin_extends_on_longer_duration() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        w.tick(0.016);
        w.begin(3.0);
        assert!((w.timer - 3.0).abs() < 1e-4);
    }

    #[test]
    fn begin_no_extend_on_shorter_duration() {
        let mut w = Windup::new(2.0);
        w.begin(3.0);
        w.begin(1.0);
        assert!((w.timer - 3.0).abs() < 1e-4);
    }

    #[test]
    fn just_started_not_set_on_extend() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        w.tick(0.016);
        w.begin(3.0);
        assert!(!w.just_started);
    }

    #[test]
    fn release_fires_strike_early() {
        let mut w = Windup::new(2.0);
        w.begin(3.0);
        assert!(w.release());
        assert!(!w.is_winding_up());
        assert!(w.just_released);
    }

    #[test]
    fn release_returns_false_when_idle() {
        let mut w = Windup::new(2.0);
        assert!(!w.release());
    }

    #[test]
    fn tick_returns_true_on_natural_expiry() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        let fired = w.tick(1.1);
        assert!(fired);
        assert!(!w.is_winding_up());
        assert!(w.just_released);
    }

    #[test]
    fn tick_returns_false_before_expiry() {
        let mut w = Windup::new(2.0);
        w.begin(2.0);
        assert!(!w.tick(0.5));
    }

    #[test]
    fn tick_clears_just_started() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        w.tick(0.016);
        assert!(!w.just_started);
    }

    #[test]
    fn tick_clears_just_released() {
        let mut w = Windup::new(2.0);
        w.begin(0.5);
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_released);
    }

    #[test]
    fn effective_damage_amplified_while_winding() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        assert!((w.effective_damage(100.0) - 200.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_idle() {
        let w = Windup::new(2.0);
        assert!((w.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut w = Windup::new(2.0);
        w.begin(4.0);
        w.tick(2.0);
        assert!((w.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_idle() {
        let w = Windup::new(2.0);
        assert!((w.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_begin_no_op() {
        let mut w = Windup::new(2.0);
        w.enabled = false;
        w.begin(1.0);
        assert!(!w.is_winding_up());
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut w = Windup::new(2.0);
        w.begin(1.0);
        w.enabled = false;
        assert!((w.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }
}
