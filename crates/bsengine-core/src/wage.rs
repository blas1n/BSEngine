use bevy_ecs::prelude::Component;

/// Earnings-income accumulation tracker named after wage, the noun
/// and verb meaning a regular payment for labour or services; to
/// carry on or engage in (a conflict or campaign) — from the Old
/// North French wage (a pledge, a security), from the Frankish
/// wadja or Old High German wetti (a pledge, a wager), from the
/// Proto-Germanic wadją, related to the Latin vas, vadis (a surety,
/// a bail). The connection between "earnings" and "pledge" reflects
/// the original economic structure: a wage was not simply payment
/// after work was done but a deposit, a pledge, a guarantee put
/// forward before work began — the employer's commitment to pay
/// at the end of the term. The related words wager and gage (a
/// pledge) preserve this original pledging sense that the modern
/// wage has shed. In political economy, the wage is the price of
/// labour-power — not of labour itself, which cannot be bought, but
/// of the capacity for labour, which is sold in advance and exercised
/// over time. In game mechanics, a wage mechanic models the slow
/// accumulation of earned income — the drip of coin as time passes,
/// the fill of the treasury as work is performed, the build of
/// earnings that eventually reaches a threshold at which a meaningful
/// purchase, upgrade, or action becomes available. `earnings` builds
/// via `earn(amount)` and accumulates passively at `income_rate` per
/// second in `tick(dt)` or is spent via `spend(amount)`.
///
/// Models earnings-income fill levels, treasury-saturation bars,
/// coin-accumulation trackers, salary-advance gauges, passive-income
/// fill levels, bounty-saturation indicators, tribute-accumulation
/// bars, toll-income meters, prize-completion fill levels, or any
/// mechanic where a character, faction, or entity slowly accumulates
/// the earned income required to trigger a reward, unlock a purchase,
/// or meet a threshold — each moment of labour adding a fraction
/// to the ledger until the wage is fully earned and the transaction
/// can be completed.
///
/// `earn(amount)` adds earnings; fires `just_paid` when first
/// reaching `max_earnings`. No-op when disabled.
///
/// `spend(amount)` reduces earnings immediately; fires `just_broke`
/// when reaching 0. No-op when disabled or already broke.
///
/// `tick(dt)` clears both flags, then increases earnings by
/// `income_rate * dt` (capped at `max_earnings`). Fires `just_paid`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_paid()` returns `earnings >= max_earnings && enabled`.
///
/// `is_broke()` returns `earnings == 0.0` (not gated by `enabled`).
///
/// `earnings_fraction()` returns
/// `(earnings / max_earnings).clamp(0, 1)`.
///
/// `effective_income(scale)` returns `scale * earnings_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — earns at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Wage {
    pub earnings: f32,
    pub max_earnings: f32,
    pub income_rate: f32,
    pub just_paid: bool,
    pub just_broke: bool,
    pub enabled: bool,
}

impl Wage {
    pub fn new(max_earnings: f32, income_rate: f32) -> Self {
        Self {
            earnings: 0.0,
            max_earnings: max_earnings.max(0.1),
            income_rate: income_rate.max(0.0),
            just_paid: false,
            just_broke: false,
            enabled: true,
        }
    }

    /// Add earnings; fires `just_paid` when first reaching max.
    /// No-op when disabled.
    pub fn earn(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.earnings < self.max_earnings;
        self.earnings = (self.earnings + amount).min(self.max_earnings);
        if was_below && self.earnings >= self.max_earnings {
            self.just_paid = true;
        }
    }

    /// Reduce earnings; fires `just_broke` when reaching 0.
    /// No-op when disabled or already broke.
    pub fn spend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.earnings <= 0.0 {
            return;
        }
        self.earnings = (self.earnings - amount).max(0.0);
        if self.earnings <= 0.0 {
            self.just_broke = true;
        }
    }

    /// Clear flags, then increase earnings by `income_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_paid = false;
        self.just_broke = false;
        if self.enabled && self.income_rate > 0.0 && self.earnings < self.max_earnings {
            let was_below = self.earnings < self.max_earnings;
            self.earnings = (self.earnings + self.income_rate * dt).min(self.max_earnings);
            if was_below && self.earnings >= self.max_earnings {
                self.just_paid = true;
            }
        }
    }

    /// `true` when earnings are at maximum and component is enabled.
    pub fn is_paid(&self) -> bool {
        self.earnings >= self.max_earnings && self.enabled
    }

    /// `true` when earnings are 0 (not gated by `enabled`).
    pub fn is_broke(&self) -> bool {
        self.earnings == 0.0
    }

    /// Fraction of maximum earnings [0.0, 1.0].
    pub fn earnings_fraction(&self) -> f32 {
        (self.earnings / self.max_earnings).clamp(0.0, 1.0)
    }

    /// Returns `scale * earnings_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_income(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.earnings_fraction()
    }
}

impl Default for Wage {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Wage {
        Wage::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_broke() {
        let w = w();
        assert_eq!(w.earnings, 0.0);
        assert!(w.is_broke());
        assert!(!w.is_paid());
    }

    #[test]
    fn new_clamps_max_earnings() {
        let w = Wage::new(-5.0, 1.5);
        assert!((w.max_earnings - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_income_rate() {
        let w = Wage::new(100.0, -1.5);
        assert_eq!(w.income_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Wage::default();
        assert!((w.max_earnings - 100.0).abs() < 1e-5);
        assert!((w.income_rate - 1.5).abs() < 1e-5);
    }

    // --- earn ---

    #[test]
    fn earn_adds_earnings() {
        let mut w = w();
        w.earn(40.0);
        assert!((w.earnings - 40.0).abs() < 1e-3);
    }

    #[test]
    fn earn_clamps_at_max() {
        let mut w = w();
        w.earn(200.0);
        assert!((w.earnings - 100.0).abs() < 1e-3);
    }

    #[test]
    fn earn_fires_just_paid_at_max() {
        let mut w = w();
        w.earn(100.0);
        assert!(w.just_paid);
        assert!(w.is_paid());
    }

    #[test]
    fn earn_no_just_paid_when_already_at_max() {
        let mut w = w();
        w.earnings = 100.0;
        w.earn(10.0);
        assert!(!w.just_paid);
    }

    #[test]
    fn earn_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.earn(50.0);
        assert_eq!(w.earnings, 0.0);
    }

    #[test]
    fn earn_no_op_when_amount_zero() {
        let mut w = w();
        w.earn(0.0);
        assert_eq!(w.earnings, 0.0);
    }

    // --- spend ---

    #[test]
    fn spend_reduces_earnings() {
        let mut w = w();
        w.earnings = 60.0;
        w.spend(20.0);
        assert!((w.earnings - 40.0).abs() < 1e-3);
    }

    #[test]
    fn spend_clamps_at_zero() {
        let mut w = w();
        w.earnings = 30.0;
        w.spend(200.0);
        assert_eq!(w.earnings, 0.0);
    }

    #[test]
    fn spend_fires_just_broke_at_zero() {
        let mut w = w();
        w.earnings = 30.0;
        w.spend(30.0);
        assert!(w.just_broke);
    }

    #[test]
    fn spend_no_op_when_already_broke() {
        let mut w = w();
        w.spend(10.0);
        assert!(!w.just_broke);
    }

    #[test]
    fn spend_no_op_when_disabled() {
        let mut w = w();
        w.earnings = 50.0;
        w.enabled = false;
        w.spend(50.0);
        assert!((w.earnings - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_earnings() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.earnings - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_paid_on_earnings_to_max() {
        let mut w = Wage::new(100.0, 200.0);
        w.earnings = 95.0;
        w.tick(1.0);
        assert!(w.just_paid);
        assert!(w.is_paid());
    }

    #[test]
    fn tick_no_build_when_already_paid() {
        let mut w = w();
        w.earnings = 100.0;
        w.tick(1.0);
        assert!(!w.just_paid);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Wage::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.earnings, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.earnings, 0.0);
    }

    #[test]
    fn tick_clears_just_paid() {
        let mut w = Wage::new(100.0, 200.0);
        w.earnings = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_paid);
    }

    #[test]
    fn tick_clears_just_broke() {
        let mut w = w();
        w.earnings = 10.0;
        w.spend(10.0);
        w.tick(0.016);
        assert!(!w.just_broke);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.earnings - 9.0).abs() < 1e-3);
    }

    // --- is_paid / is_broke ---

    #[test]
    fn is_paid_false_when_disabled() {
        let mut w = w();
        w.earnings = 100.0;
        w.enabled = false;
        assert!(!w.is_paid());
    }

    #[test]
    fn is_broke_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_broke());
    }

    // --- earnings_fraction / effective_income ---

    #[test]
    fn earnings_fraction_zero_when_broke() {
        assert_eq!(w().earnings_fraction(), 0.0);
    }

    #[test]
    fn earnings_fraction_half_at_midpoint() {
        let mut w = w();
        w.earnings = 50.0;
        assert!((w.earnings_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_income_zero_when_broke() {
        assert_eq!(w().effective_income(100.0), 0.0);
    }

    #[test]
    fn effective_income_scales_with_earnings() {
        let mut w = w();
        w.earnings = 75.0;
        assert!((w.effective_income(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_income_zero_when_disabled() {
        let mut w = w();
        w.earnings = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_income(100.0), 0.0);
    }
}
