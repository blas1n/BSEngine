use bevy_ecs::prelude::Component;

/// Irony-cynicism accumulation tracker named after wry, the
/// adjective meaning bent, twisted, or turned to one side;
/// using or expressing dry, especially mocking, humor; marked
/// by a grimly or bitterly ironic quality — from the Old
/// English wrīgian (to bend, to turn, to go), from the Proto-
/// Germanic wrīgōną (to press on, to bend), from the Proto-
/// Indo-European root wrei- (to turn, to twist). The physical
/// sense — twisted, bent, crooked — underlies all the derived
/// meanings. A wry face is one twisted by displeasure; a wry
/// neck is one turned or twisted involuntarily; wry humor is
/// humor whose shape has been bent by experience, adversity,
/// or disappointment until it can no longer emerge straight-
/// faced. The path from "twisted" to "ironically humorous"
/// follows the logic of experience: a person who has been
/// through enough discovers that direct responses — earnest
/// outrage, sincere enthusiasm, undefended optimism — are
/// no longer quite available to them; what remains is the
/// twist, the sideways acknowledgment that things are as
/// they are and one has long since stopped being surprised
/// by it. Wry humor is the survival mechanism of the
/// repeatedly disappointed — it neither ignores what is
/// wrong nor collapses under it, but twists just enough
/// to find the absurdity bearable. In game mechanics, a
/// wry mechanic models the accumulation of ironic detachment
/// — the build of cynicism, bemused worldliness, or sardonic
/// observation that eventually reaches the threshold at which
/// a character refuses naive responses and gains the ability
/// to see through pretense, recognize manipulation, or
/// deliver the withering remark that deflates pomposity.
/// `irony` builds via `quip(amount)` and accumulates
/// passively at `twist_rate` per second in `tick(dt)` or
/// is disarmed via `sincere(amount)`.
///
/// Models irony-cynicism fill levels, detachment-saturation
/// bars, wit-accumulation trackers, sardonic-build gauges,
/// worldliness fill levels, bemused-saturation indicators,
/// cynicism-accumulation bars, jaded-observation meters,
/// wryness-completion fill levels, or any mechanic where
/// a character slowly accumulates the ironic distance,
/// sardonic experience, or detached wisdom that grants
/// them the ability to see through pretense, deflect
/// manipulation, or deliver the remark that cuts through
/// self-important nonsense with a single twist of phrase.
///
/// `quip(amount)` adds irony; fires `just_wry` when first
/// reaching `max_irony`. No-op when disabled.
///
/// `sincere(amount)` reduces irony immediately; fires
/// `just_earnest` when reaching 0. No-op when disabled or
/// already earnest.
///
/// `tick(dt)` clears both flags, then increases irony by
/// `twist_rate * dt` (capped at `max_irony`). Fires
/// `just_wry` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_wry()` returns `irony >= max_irony && enabled`.
///
/// `is_earnest()` returns `irony == 0.0` (not gated by
/// `enabled`).
///
/// `irony_fraction()` returns
/// `(irony / max_irony).clamp(0, 1)`.
///
/// `effective_wit(scale)` returns `scale * irony_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — twists at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wry {
    pub irony: f32,
    pub max_irony: f32,
    pub twist_rate: f32,
    pub just_wry: bool,
    pub just_earnest: bool,
    pub enabled: bool,
}

impl Wry {
    pub fn new(max_irony: f32, twist_rate: f32) -> Self {
        Self {
            irony: 0.0,
            max_irony: max_irony.max(0.1),
            twist_rate: twist_rate.max(0.0),
            just_wry: false,
            just_earnest: false,
            enabled: true,
        }
    }

    /// Add irony; fires `just_wry` when first reaching max.
    /// No-op when disabled.
    pub fn quip(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.irony < self.max_irony;
        self.irony = (self.irony + amount).min(self.max_irony);
        if was_below && self.irony >= self.max_irony {
            self.just_wry = true;
        }
    }

    /// Reduce irony; fires `just_earnest` when reaching 0.
    /// No-op when disabled or already earnest.
    pub fn sincere(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.irony <= 0.0 {
            return;
        }
        self.irony = (self.irony - amount).max(0.0);
        if self.irony <= 0.0 {
            self.just_earnest = true;
        }
    }

    /// Clear flags, then increase irony by `twist_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_wry = false;
        self.just_earnest = false;
        if self.enabled && self.twist_rate > 0.0 && self.irony < self.max_irony {
            let was_below = self.irony < self.max_irony;
            self.irony = (self.irony + self.twist_rate * dt).min(self.max_irony);
            if was_below && self.irony >= self.max_irony {
                self.just_wry = true;
            }
        }
    }

    /// `true` when irony is at maximum and component is enabled.
    pub fn is_wry(&self) -> bool {
        self.irony >= self.max_irony && self.enabled
    }

    /// `true` when irony is 0 (not gated by `enabled`).
    pub fn is_earnest(&self) -> bool {
        self.irony == 0.0
    }

    /// Fraction of maximum irony [0.0, 1.0].
    pub fn irony_fraction(&self) -> f32 {
        (self.irony / self.max_irony).clamp(0.0, 1.0)
    }

    /// Returns `scale * irony_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_wit(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.irony_fraction()
    }
}

impl Default for Wry {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wry {
        Wry::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_earnest() {
        let w = w();
        assert_eq!(w.irony, 0.0);
        assert!(w.is_earnest());
        assert!(!w.is_wry());
    }

    #[test]
    fn new_clamps_max_irony() {
        let w = Wry::new(-5.0, 1.5);
        assert!((w.max_irony - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_twist_rate() {
        let w = Wry::new(100.0, -1.5);
        assert_eq!(w.twist_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wry::default();
        assert!((w.max_irony - 100.0).abs() < 1e-5);
        assert!((w.twist_rate - 1.5).abs() < 1e-5);
    }

    // --- quip ---

    #[test]
    fn quip_adds_irony() {
        let mut w = w();
        w.quip(40.0);
        assert!((w.irony - 40.0).abs() < 1e-3);
    }

    #[test]
    fn quip_clamps_at_max() {
        let mut w = w();
        w.quip(200.0);
        assert!((w.irony - 100.0).abs() < 1e-3);
    }

    #[test]
    fn quip_fires_just_wry_at_max() {
        let mut w = w();
        w.quip(100.0);
        assert!(w.just_wry);
        assert!(w.is_wry());
    }

    #[test]
    fn quip_no_just_wry_when_already_at_max() {
        let mut w = w();
        w.irony = 100.0;
        w.quip(10.0);
        assert!(!w.just_wry);
    }

    #[test]
    fn quip_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.quip(50.0);
        assert_eq!(w.irony, 0.0);
    }

    #[test]
    fn quip_no_op_when_amount_zero() {
        let mut w = w();
        w.quip(0.0);
        assert_eq!(w.irony, 0.0);
    }

    // --- sincere ---

    #[test]
    fn sincere_reduces_irony() {
        let mut w = w();
        w.irony = 60.0;
        w.sincere(20.0);
        assert!((w.irony - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sincere_clamps_at_zero() {
        let mut w = w();
        w.irony = 30.0;
        w.sincere(200.0);
        assert_eq!(w.irony, 0.0);
    }

    #[test]
    fn sincere_fires_just_earnest_at_zero() {
        let mut w = w();
        w.irony = 30.0;
        w.sincere(30.0);
        assert!(w.just_earnest);
    }

    #[test]
    fn sincere_no_op_when_already_earnest() {
        let mut w = w();
        w.sincere(10.0);
        assert!(!w.just_earnest);
    }

    #[test]
    fn sincere_no_op_when_disabled() {
        let mut w = w();
        w.irony = 50.0;
        w.enabled = false;
        w.sincere(50.0);
        assert!((w.irony - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_irony() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.irony - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_wry_on_irony_to_max() {
        let mut w = Wry::new(100.0, 200.0);
        w.irony = 95.0;
        w.tick(1.0);
        assert!(w.just_wry);
        assert!(w.is_wry());
    }

    #[test]
    fn tick_no_build_when_already_wry() {
        let mut w = w();
        w.irony = 100.0;
        w.tick(1.0);
        assert!(!w.just_wry);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wry::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.irony, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.irony, 0.0);
    }

    #[test]
    fn tick_clears_just_wry() {
        let mut w = Wry::new(100.0, 200.0);
        w.irony = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_wry);
    }

    #[test]
    fn tick_clears_just_earnest() {
        let mut w = w();
        w.irony = 10.0;
        w.sincere(10.0);
        w.tick(0.016);
        assert!(!w.just_earnest);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.irony - 9.0).abs() < 1e-3);
    }

    // --- is_wry / is_earnest ---

    #[test]
    fn is_wry_false_when_disabled() {
        let mut w = w();
        w.irony = 100.0;
        w.enabled = false;
        assert!(!w.is_wry());
    }

    #[test]
    fn is_earnest_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_earnest());
    }

    // --- irony_fraction / effective_wit ---

    #[test]
    fn irony_fraction_zero_when_earnest() {
        assert_eq!(w().irony_fraction(), 0.0);
    }

    #[test]
    fn irony_fraction_half_at_midpoint() {
        let mut w = w();
        w.irony = 50.0;
        assert!((w.irony_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_wit_zero_when_earnest() {
        assert_eq!(w().effective_wit(100.0), 0.0);
    }

    #[test]
    fn effective_wit_scales_with_irony() {
        let mut w = w();
        w.irony = 75.0;
        assert!((w.effective_wit(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_wit_zero_when_disabled() {
        let mut w = w();
        w.irony = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_wit(100.0), 0.0);
    }
}
