use bevy_ecs::prelude::Component;

/// Retaliatory-strike window: after a successful block or taking a hit, the
/// entity enters a brief counter-attack window during which any offensive
/// action deals `effective_damage(base)` — bonus damage proportional to
/// `counter_damage_multiplier`.
///
/// `open(window)` starts or extends the window (high-watermark); sets
/// `just_opened` on inactive → active. `close()` ends it early. `tick(dt)`
/// counts down and sets `just_closed` when the window expires. Damage systems
/// should multiply outgoing melee damage when `is_open()` and enabled.
///
/// Distinct from `Parry` (intercepts and negates an incoming attack), `Deflect`
/// (redirects projectiles), and `Recoil` (knockback on hit): Counter is purely
/// about the **retaliatory timing window** after surviving an attack — a short
/// interval in which the entity can hit back with amplified force.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Counter {
    pub window: f32,
    pub timer: f32,
    /// Damage multiplier while countering. Clamped ≥ 1.0.
    pub counter_damage_multiplier: f32,
    pub just_opened: bool,
    pub just_closed: bool,
    pub enabled: bool,
}

impl Counter {
    pub fn new(counter_damage_multiplier: f32) -> Self {
        Self {
            window: 0.0,
            timer: 0.0,
            counter_damage_multiplier: counter_damage_multiplier.max(1.0),
            just_opened: false,
            just_closed: false,
            enabled: true,
        }
    }

    /// Open or extend the counter window for `window` seconds. High-watermark:
    /// only replaces the current timer when `window > timer`. Sets `just_opened`
    /// on the inactive → active transition. No-op when disabled or `window ≤ 0`.
    pub fn open(&mut self, window: f32) {
        if !self.enabled || window <= 0.0 {
            return;
        }
        if window > self.timer {
            let was_open = self.is_open();
            self.window = window;
            self.timer = window;
            if !was_open {
                self.just_opened = true;
            }
        }
    }

    /// Close the window early (e.g., entity was stunned or missed the window).
    /// Sets `just_closed`. No-op when the window is already closed.
    pub fn close(&mut self) {
        if !self.is_open() {
            return;
        }
        self.timer = 0.0;
        self.window = 0.0;
        self.just_closed = true;
    }

    /// Advance the counter timer. Sets `just_closed` when the window expires
    /// naturally. Clears one-frame flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_opened = false;
        self.just_closed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.window = 0.0;
                self.just_closed = true;
            }
        }
    }

    pub fn is_open(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective outgoing melee damage during the counter window.
    /// Returns `base * counter_damage_multiplier` while open and enabled,
    /// `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_open() && self.enabled {
            base * self.counter_damage_multiplier
        } else {
            base
        }
    }

    /// Fraction of the window remaining [1.0 = just opened, 0.0 = closed].
    pub fn remaining_fraction(&self) -> f32 {
        if self.window <= 0.0 {
            return 0.0;
        }
        (self.timer / self.window).clamp(0.0, 1.0)
    }
}

impl Default for Counter {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn open_starts_window() {
        let mut c = Counter::new(1.5);
        c.open(0.5);
        assert!(c.is_open());
        assert!(c.just_opened);
    }

    #[test]
    fn open_extends_on_longer_window() {
        let mut c = Counter::new(1.5);
        c.open(0.3);
        c.tick(0.016);
        c.open(0.8);
        assert!((c.timer - 0.8).abs() < 1e-4);
    }

    #[test]
    fn open_no_extend_on_shorter_window() {
        let mut c = Counter::new(1.5);
        c.open(0.8);
        c.open(0.3);
        assert!((c.timer - 0.8).abs() < 1e-4);
    }

    #[test]
    fn just_opened_not_set_on_extend() {
        let mut c = Counter::new(1.5);
        c.open(0.3);
        c.tick(0.016);
        c.open(0.8);
        assert!(!c.just_opened);
    }

    #[test]
    fn close_ends_window() {
        let mut c = Counter::new(1.5);
        c.open(0.5);
        c.close();
        assert!(!c.is_open());
        assert!(c.just_closed);
    }

    #[test]
    fn close_no_op_when_already_closed() {
        let mut c = Counter::new(1.5);
        c.close();
        assert!(!c.just_closed);
    }

    #[test]
    fn tick_expires_window() {
        let mut c = Counter::new(1.5);
        c.open(0.2);
        c.tick(0.3);
        assert!(!c.is_open());
        assert!(c.just_closed);
    }

    #[test]
    fn tick_clears_just_opened() {
        let mut c = Counter::new(1.5);
        c.open(0.5);
        c.tick(0.016);
        assert!(!c.just_opened);
    }

    #[test]
    fn tick_clears_just_closed() {
        let mut c = Counter::new(1.5);
        c.open(0.1);
        c.tick(0.2);
        c.tick(0.016);
        assert!(!c.just_closed);
    }

    #[test]
    fn effective_damage_multiplied_while_open() {
        let mut c = Counter::new(1.5);
        c.open(0.5);
        assert!((c.effective_damage(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_closed() {
        let c = Counter::new(1.5);
        assert!((c.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut c = Counter::new(1.5);
        c.open(0.4);
        c.tick(0.2);
        assert!((c.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_closed() {
        let c = Counter::new(1.5);
        assert!((c.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_open_no_op() {
        let mut c = Counter::new(1.5);
        c.enabled = false;
        c.open(0.5);
        assert!(!c.is_open());
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut c = Counter::new(1.5);
        c.open(0.5);
        c.enabled = false;
        assert!((c.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn multiplier_clamped_to_one() {
        let c = Counter::new(0.5); // below 1.0 → clamped to 1.0
        assert!((c.counter_damage_multiplier - 1.0).abs() < 1e-5);
    }
}
