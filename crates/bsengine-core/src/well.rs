use bevy_ecs::prelude::Component;

/// Resource-reservoir accumulation tracker named after well, the
/// noun meaning a shaft or hole sunk into the ground to obtain
/// water, oil, or other liquid; a plentiful supply or source —
/// from the Old English wella, wylla (a spring, a well, a stream),
/// from the Proto-Germanic wallan (to bubble up, to well up), from
/// the Proto-Indo-European root wel- (to turn, to roll). The
/// well's mechanism is conceptually elegant: instead of carrying
/// water from a distant source, you penetrate the earth to where
/// water naturally collects and rise, and the pressure differential
/// does the delivery work. The first wells, dug in the Neolithic
/// period, represent a fundamental shift in settlement patterns —
/// populations could now inhabit areas without surface water, and
/// the range of permanent settlement expanded dramatically. The
/// well became the social centre of the village: the place where
/// water was collected, where people gathered, where news circulated,
/// where the marriageable young met under the supervision of the
/// elderly who had no better excuse to linger. In metaphor, a well
/// of emotion is a deep reserve that can be drawn upon, that
/// refills from underground sources, that can be exhausted by too
/// many demands but returns given time. A well of patience, a well
/// of knowledge, a well of grief — all share the structure of a
/// replenishing reservoir rather than a fixed stock. In game
/// mechanics, a well mechanic models the slow fill of a resource
/// reservoir — the natural accumulation of water, mana, oil, or
/// power that collects in a deposit point and can be drawn upon
/// when needed. `reserve` builds via `fill(amount)` and accumulates
/// passively at `seep_rate` per second in `tick(dt)` or is drawn
/// via `draw(amount)`.
///
/// Models resource-reservoir fill levels, mana-well saturation
/// bars, power-spring accumulators, underground-deposit gauges,
/// natural-accumulation fill levels, replenishment-saturation
/// indicators, reservoir-build accumulation bars, supply-source
/// meters, spring-completion fill levels, or any mechanic where
/// a point in the world or a character's inner reserve slowly
/// accumulates a resource from a deep source — dripping in
/// drop by drop until the reservoir is full and ready to be
/// drawn upon by anyone who comes to collect.
///
/// `fill(amount)` adds reserve; fires `just_full` when first
/// reaching `max_reserve`. No-op when disabled.
///
/// `draw(amount)` reduces reserve immediately; fires `just_dry`
/// when reaching 0. No-op when disabled or already dry.
///
/// `tick(dt)` clears both flags, then increases reserve by
/// `seep_rate * dt` (capped at `max_reserve`). Fires `just_full`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_full()` returns `reserve >= max_reserve && enabled`.
///
/// `is_dry()` returns `reserve == 0.0` (not gated by `enabled`).
///
/// `reserve_fraction()` returns
/// `(reserve / max_reserve).clamp(0, 1)`.
///
/// `effective_yield(scale)` returns `scale * reserve_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — seeps at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Well {
    pub reserve: f32,
    pub max_reserve: f32,
    pub seep_rate: f32,
    pub just_full: bool,
    pub just_dry: bool,
    pub enabled: bool,
}

impl Well {
    pub fn new(max_reserve: f32, seep_rate: f32) -> Self {
        Self {
            reserve: 0.0,
            max_reserve: max_reserve.max(0.1),
            seep_rate: seep_rate.max(0.0),
            just_full: false,
            just_dry: false,
            enabled: true,
        }
    }

    /// Add reserve; fires `just_full` when first reaching max.
    /// No-op when disabled.
    pub fn fill(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.reserve < self.max_reserve;
        self.reserve = (self.reserve + amount).min(self.max_reserve);
        if was_below && self.reserve >= self.max_reserve {
            self.just_full = true;
        }
    }

    /// Reduce reserve; fires `just_dry` when reaching 0.
    /// No-op when disabled or already dry.
    pub fn draw(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.reserve <= 0.0 {
            return;
        }
        self.reserve = (self.reserve - amount).max(0.0);
        if self.reserve <= 0.0 {
            self.just_dry = true;
        }
    }

    /// Clear flags, then increase reserve by `seep_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_full = false;
        self.just_dry = false;
        if self.enabled && self.seep_rate > 0.0 && self.reserve < self.max_reserve {
            let was_below = self.reserve < self.max_reserve;
            self.reserve = (self.reserve + self.seep_rate * dt).min(self.max_reserve);
            if was_below && self.reserve >= self.max_reserve {
                self.just_full = true;
            }
        }
    }

    /// `true` when reserve is at maximum and component is enabled.
    pub fn is_full(&self) -> bool {
        self.reserve >= self.max_reserve && self.enabled
    }

    /// `true` when reserve is 0 (not gated by `enabled`).
    pub fn is_dry(&self) -> bool {
        self.reserve == 0.0
    }

    /// Fraction of maximum reserve [0.0, 1.0].
    pub fn reserve_fraction(&self) -> f32 {
        (self.reserve / self.max_reserve).clamp(0.0, 1.0)
    }

    /// Returns `scale * reserve_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_yield(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.reserve_fraction()
    }
}

impl Default for Well {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Well {
        Well::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_dry() {
        let w = w();
        assert_eq!(w.reserve, 0.0);
        assert!(w.is_dry());
        assert!(!w.is_full());
    }

    #[test]
    fn new_clamps_max_reserve() {
        let w = Well::new(-5.0, 1.5);
        assert!((w.max_reserve - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_seep_rate() {
        let w = Well::new(100.0, -1.5);
        assert_eq!(w.seep_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Well::default();
        assert!((w.max_reserve - 100.0).abs() < 1e-5);
        assert!((w.seep_rate - 1.5).abs() < 1e-5);
    }

    // --- fill ---

    #[test]
    fn fill_adds_reserve() {
        let mut w = w();
        w.fill(40.0);
        assert!((w.reserve - 40.0).abs() < 1e-3);
    }

    #[test]
    fn fill_clamps_at_max() {
        let mut w = w();
        w.fill(200.0);
        assert!((w.reserve - 100.0).abs() < 1e-3);
    }

    #[test]
    fn fill_fires_just_full_at_max() {
        let mut w = w();
        w.fill(100.0);
        assert!(w.just_full);
        assert!(w.is_full());
    }

    #[test]
    fn fill_no_just_full_when_already_at_max() {
        let mut w = w();
        w.reserve = 100.0;
        w.fill(10.0);
        assert!(!w.just_full);
    }

    #[test]
    fn fill_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.fill(50.0);
        assert_eq!(w.reserve, 0.0);
    }

    #[test]
    fn fill_no_op_when_amount_zero() {
        let mut w = w();
        w.fill(0.0);
        assert_eq!(w.reserve, 0.0);
    }

    // --- draw ---

    #[test]
    fn draw_reduces_reserve() {
        let mut w = w();
        w.reserve = 60.0;
        w.draw(20.0);
        assert!((w.reserve - 40.0).abs() < 1e-3);
    }

    #[test]
    fn draw_clamps_at_zero() {
        let mut w = w();
        w.reserve = 30.0;
        w.draw(200.0);
        assert_eq!(w.reserve, 0.0);
    }

    #[test]
    fn draw_fires_just_dry_at_zero() {
        let mut w = w();
        w.reserve = 30.0;
        w.draw(30.0);
        assert!(w.just_dry);
    }

    #[test]
    fn draw_no_op_when_already_dry() {
        let mut w = w();
        w.draw(10.0);
        assert!(!w.just_dry);
    }

    #[test]
    fn draw_no_op_when_disabled() {
        let mut w = w();
        w.reserve = 50.0;
        w.enabled = false;
        w.draw(50.0);
        assert!((w.reserve - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_reserve() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.reserve - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_full_on_reserve_to_max() {
        let mut w = Well::new(100.0, 200.0);
        w.reserve = 95.0;
        w.tick(1.0);
        assert!(w.just_full);
        assert!(w.is_full());
    }

    #[test]
    fn tick_no_build_when_already_full() {
        let mut w = w();
        w.reserve = 100.0;
        w.tick(1.0);
        assert!(!w.just_full);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Well::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.reserve, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.reserve, 0.0);
    }

    #[test]
    fn tick_clears_just_full() {
        let mut w = Well::new(100.0, 200.0);
        w.reserve = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_full);
    }

    #[test]
    fn tick_clears_just_dry() {
        let mut w = w();
        w.reserve = 10.0;
        w.draw(10.0);
        w.tick(0.016);
        assert!(!w.just_dry);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.reserve - 9.0).abs() < 1e-3);
    }

    // --- is_full / is_dry ---

    #[test]
    fn is_full_false_when_disabled() {
        let mut w = w();
        w.reserve = 100.0;
        w.enabled = false;
        assert!(!w.is_full());
    }

    #[test]
    fn is_dry_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_dry());
    }

    // --- reserve_fraction / effective_yield ---

    #[test]
    fn reserve_fraction_zero_when_dry() {
        assert_eq!(w().reserve_fraction(), 0.0);
    }

    #[test]
    fn reserve_fraction_half_at_midpoint() {
        let mut w = w();
        w.reserve = 50.0;
        assert!((w.reserve_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_yield_zero_when_dry() {
        assert_eq!(w().effective_yield(100.0), 0.0);
    }

    #[test]
    fn effective_yield_scales_with_reserve() {
        let mut w = w();
        w.reserve = 75.0;
        assert!((w.effective_yield(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_yield_zero_when_disabled() {
        let mut w = w();
        w.reserve = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_yield(100.0), 0.0);
    }
}
