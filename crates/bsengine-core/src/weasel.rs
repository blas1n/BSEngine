use bevy_ecs::prelude::Component;

/// Squirminess reserve that regenerates over time and discharges in a single
/// escape attempt. Systems read `just_slipped` and `weasel_fraction()` to
/// detect when an escape occurs and how effective it is.
///
/// `slip()` spends the reserve: fires `just_slipped` if `weasel_level > 0`,
/// then resets `weasel_level` to 0. No-op when disabled.
///
/// `tick(dt)` clears `just_slipped`, then increases `weasel_level` by
/// `regen_rate * dt` (capped at `max_weasel`). No-op when disabled.
///
/// `is_ready()` returns `weasel_level >= max_weasel && enabled`.
///
/// `weasel_fraction()` returns `(weasel_level / max_weasel).clamp(0.0, 1.0)`.
///
/// `effective_escape(base)` returns
/// `base * (1.0 + escape_bonus * weasel_fraction())` when enabled; returns
/// `base` unchanged otherwise. Query before `slip()` to capture full reserve
/// quality.
///
/// Distinct from `Dodge` (one-shot directional avoid), `Evade` (stamina-gated
/// dash), and `Deflect` (angling attacks away): Weasel models a **passive
/// squirminess reserve** — the entity slowly builds readiness to slip free
/// from constraints, grabs, or entanglement without any movement input,
/// with escape quality scaling to how fully charged the reserve is.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Weasel {
    /// Current squirminess reserve [0.0, max_weasel].
    pub weasel_level: f32,
    /// Maximum reserve. Clamped >= 1.0.
    pub max_weasel: f32,
    /// Reserve recovery per second. Clamped >= 0.0.
    pub regen_rate: f32,
    /// Escape quality bonus at full reserve. Clamped >= 0.0.
    pub escape_bonus: f32,
    pub just_slipped: bool,
    pub enabled: bool,
}

impl Weasel {
    pub fn new(max_weasel: f32, regen_rate: f32, escape_bonus: f32) -> Self {
        Self {
            weasel_level: 0.0,
            max_weasel: max_weasel.max(1.0),
            regen_rate: regen_rate.max(0.0),
            escape_bonus: escape_bonus.max(0.0),
            just_slipped: false,
            enabled: true,
        }
    }

    /// Spend the reserve in an escape attempt. Fires `just_slipped` if
    /// `weasel_level > 0`, then resets reserve to 0. No-op when disabled.
    pub fn slip(&mut self) {
        if !self.enabled {
            return;
        }
        if self.weasel_level > 0.0 {
            self.just_slipped = true;
            self.weasel_level = 0.0;
        }
    }

    /// Advance one frame: clear `just_slipped`, then regenerate reserve.
    /// No-op when disabled.
    pub fn tick(&mut self, dt: f32) {
        self.just_slipped = false;

        if !self.enabled {
            return;
        }
        self.weasel_level = (self.weasel_level + self.regen_rate * dt).min(self.max_weasel);
    }

    /// `true` when reserve is full and the component is enabled.
    pub fn is_ready(&self) -> bool {
        self.weasel_level >= self.max_weasel && self.enabled
    }

    /// Reserve as a fraction of maximum [0.0, 1.0].
    pub fn weasel_fraction(&self) -> f32 {
        (self.weasel_level / self.max_weasel).clamp(0.0, 1.0)
    }

    /// Scale escape `base` by reserve quality. Returns
    /// `base * (1 + escape_bonus * fraction)` when enabled; `base` otherwise.
    /// Query before `slip()` to capture current reserve level.
    pub fn effective_escape(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.escape_bonus * self.weasel_fraction())
    }
}

impl Default for Weasel {
    fn default() -> Self {
        Self::new(10.0, 2.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Weasel {
        Weasel::new(10.0, 3.0, 0.5)
    }

    #[test]
    fn new_starts_empty() {
        let w = Weasel::new(10.0, 3.0, 0.5);
        assert_eq!(w.weasel_level, 0.0);
        assert!(!w.just_slipped);
        assert!(!w.is_ready());
    }

    #[test]
    fn tick_regenerates_reserve() {
        let mut w = w(); // regen_rate = 3.0
        w.tick(1.0); // 3.0 * 1.0 = 3.0
        assert!((w.weasel_level - 3.0).abs() < 1e-4);
    }

    #[test]
    fn tick_caps_at_max() {
        let mut w = w();
        w.tick(100.0); // 3.0 * 100 → capped at 10
        assert!((w.weasel_level - 10.0).abs() < 1e-4);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(5.0);
        assert_eq!(w.weasel_level, 0.0);
    }

    #[test]
    fn tick_clears_just_slipped() {
        let mut w = w();
        w.tick(4.0); // build some
        w.slip(); // just_slipped fires
        w.tick(0.016); // cleared
        assert!(!w.just_slipped);
    }

    #[test]
    fn slip_fires_just_slipped_when_charged() {
        let mut w = w();
        w.tick(2.0); // 6.0, some reserve
        w.slip();
        assert!(w.just_slipped);
    }

    #[test]
    fn slip_resets_reserve_to_zero() {
        let mut w = w();
        w.tick(2.0); // 6.0
        w.slip();
        assert_eq!(w.weasel_level, 0.0);
    }

    #[test]
    fn slip_no_op_when_empty() {
        let mut w = w();
        w.slip(); // nothing to spend
        assert!(!w.just_slipped);
        assert_eq!(w.weasel_level, 0.0);
    }

    #[test]
    fn slip_no_op_when_disabled() {
        let mut w = w();
        w.tick(3.0); // 9.0
        w.enabled = false;
        w.slip();
        assert!(!w.just_slipped);
        assert!((w.weasel_level - 9.0).abs() < 1e-4); // unchanged
    }

    #[test]
    fn is_ready_true_at_max() {
        let mut w = w();
        w.tick(100.0);
        assert!(w.is_ready());
    }

    #[test]
    fn is_ready_false_below_max() {
        let mut w = w();
        w.tick(1.0); // 3.0 < 10.0
        assert!(!w.is_ready());
    }

    #[test]
    fn is_ready_false_when_disabled() {
        let mut w = w();
        w.tick(100.0);
        w.enabled = false;
        assert!(!w.is_ready());
    }

    #[test]
    fn weasel_fraction_zero_when_empty() {
        let w = w();
        assert_eq!(w.weasel_fraction(), 0.0);
    }

    #[test]
    fn weasel_fraction_half_at_midpoint() {
        let mut w = w();
        w.weasel_level = 5.0;
        assert!((w.weasel_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn weasel_fraction_one_at_max() {
        let mut w = w();
        w.tick(100.0);
        assert!((w.weasel_fraction() - 1.0).abs() < 1e-4);
    }

    #[test]
    fn effective_escape_base_when_empty() {
        let w = w(); // escape_bonus=0.5, fraction=0
        assert!((w.effective_escape(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn effective_escape_boosted_at_half_reserve() {
        let mut w = Weasel::new(10.0, 3.0, 0.5);
        w.weasel_level = 5.0; // fraction=0.5
                              // 100 * (1 + 0.5*0.5) = 125
        assert!((w.effective_escape(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_escape_fully_boosted_at_max() {
        let mut w = Weasel::new(10.0, 3.0, 0.5);
        w.tick(100.0); // max
                       // 100 * (1 + 0.5*1.0) = 150
        assert!((w.effective_escape(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_escape_passthrough_when_disabled() {
        let mut w = w();
        w.tick(100.0);
        w.enabled = false;
        assert!((w.effective_escape(100.0) - 100.0).abs() < 1e-4);
    }

    #[test]
    fn max_weasel_clamped_to_one() {
        let w = Weasel::new(0.0, 3.0, 0.5);
        assert!((w.max_weasel - 1.0).abs() < 1e-5);
    }

    #[test]
    fn regen_rate_clamped_to_zero() {
        let w = Weasel::new(10.0, -3.0, 0.5);
        assert_eq!(w.regen_rate, 0.0);
    }

    #[test]
    fn escape_bonus_clamped_to_zero() {
        let w = Weasel::new(10.0, 3.0, -1.0);
        assert_eq!(w.escape_bonus, 0.0);
    }

    #[test]
    fn slip_then_regen_and_slip_again() {
        let mut w = w();
        w.tick(4.0); // 12 → max 10
        let quality = w.effective_escape(100.0); // 150.0 at full
        w.slip(); // just_slipped, level=0
        assert!(w.just_slipped);
        assert!((quality - 150.0).abs() < 1e-3);
        w.tick(0.016); // clear
        w.tick(3.5); // 10.5 → max 10
        w.slip(); // just_slipped again at full reserve
        assert!(w.just_slipped);
    }

    #[test]
    fn partial_reserve_still_enables_escape() {
        let mut w = w();
        w.tick(1.0); // 3.0 (partial)
        w.slip();
        assert!(w.just_slipped);
        assert_eq!(w.weasel_level, 0.0);
    }

    #[test]
    fn effective_escape_before_slip_captures_quality() {
        let mut w = Weasel::new(10.0, 3.0, 1.0);
        w.tick(100.0); // full — bonus=100%
        let quality = w.effective_escape(100.0); // 200.0
        w.slip();
        assert!((quality - 200.0).abs() < 1e-3);
        // after slip, reserve=0 → effective_escape returns 100.0
        assert!((w.effective_escape(100.0) - 100.0).abs() < 1e-3);
    }
}
