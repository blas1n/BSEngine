use bevy_ecs::prelude::Component;

/// Victory-conquest accumulation tracker named after won, the
/// past tense and past participle of win, the verb meaning to
/// finish first in a contest or race; to achieve victory in
/// a battle, game, or struggle; to gain or secure by effort
/// — from the Old English winnan (to strive, to struggle, to
/// contend; to gain by effort), from the Proto-Germanic
/// winnaną (to strive, to work hard), from the Proto-Indo-
/// European root wen- (to desire, to strive for). The root
/// wen- gave Latin venus (desire, love) and venerate (to
/// revere, literally "to seek by effort"), and the same
/// Germanic root gave the Old Norse vinna (to work, to win,
/// to complete) and the modern German gewinnen (to win, to
/// gain). The journey of the word from "to strive, to labour"
/// to "to be victorious" reflects the ancient equation between
/// effort and achievement: the one who won was the one who
/// had worked hardest, strained longest, overcome the most.
/// To have won, in the oldest sense, is not merely to have
/// finished first but to have expended oneself fully in the
/// attempt. In game mechanics, a won mechanic models the slow
/// accumulation of victories — the fill of the conquest bar,
/// the build of the triumph ledger, the accumulation of wins
/// that eventually reaches the threshold at which a rank
/// advances, a reward unlocks, or a new challenge opens.
/// `victories` builds via `claim(amount)` and accumulates
/// passively at `streak_rate` per second in `tick(dt)` or
/// resets via `concede(amount)`.
///
/// Models victory-count fill levels, conquest-saturation bars,
/// win-streak accumulators, triumph-build gauges, rank-advance
/// fill levels, achievement-saturation indicators, medal-
/// accumulation bars, glory meters, prestige-completion fill
/// levels, or any mechanic where a character, team, or faction
/// slowly accumulates the victories, conquests, or wins
/// required to unlock a reward, advance a rank, or reach a
/// threshold of recognition.
///
/// `claim(amount)` adds victories; fires `just_won` when first
/// reaching `max_victories`. No-op when disabled.
///
/// `concede(amount)` reduces victories immediately; fires
/// `just_lost` when reaching 0. No-op when disabled or
/// already at zero.
///
/// `tick(dt)` clears both flags, then increases victories by
/// `streak_rate * dt` (capped at `max_victories`). Fires
/// `just_won` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_won()` returns `victories >= max_victories && enabled`.
///
/// `is_lost()` returns `victories == 0.0` (not gated by
/// `enabled`).
///
/// `victory_fraction()` returns
/// `(victories / max_victories).clamp(0, 1)`.
///
/// `effective_glory(scale)` returns `scale * victory_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — streaks at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Won {
    pub victories: f32,
    pub max_victories: f32,
    pub streak_rate: f32,
    pub just_won: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Won {
    pub fn new(max_victories: f32, streak_rate: f32) -> Self {
        Self {
            victories: 0.0,
            max_victories: max_victories.max(0.1),
            streak_rate: streak_rate.max(0.0),
            just_won: false,
            just_lost: false,
            enabled: true,
        }
    }

    /// Add victories; fires `just_won` when first reaching max.
    /// No-op when disabled.
    pub fn claim(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.victories < self.max_victories;
        self.victories = (self.victories + amount).min(self.max_victories);
        if was_below && self.victories >= self.max_victories {
            self.just_won = true;
        }
    }

    /// Reduce victories; fires `just_lost` when reaching 0.
    /// No-op when disabled or already at zero.
    pub fn concede(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.victories <= 0.0 {
            return;
        }
        self.victories = (self.victories - amount).max(0.0);
        if self.victories <= 0.0 {
            self.just_lost = true;
        }
    }

    /// Clear flags, then increase victories by `streak_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_won = false;
        self.just_lost = false;
        if self.enabled && self.streak_rate > 0.0 && self.victories < self.max_victories {
            let was_below = self.victories < self.max_victories;
            self.victories = (self.victories + self.streak_rate * dt).min(self.max_victories);
            if was_below && self.victories >= self.max_victories {
                self.just_won = true;
            }
        }
    }

    /// `true` when victories are at maximum and component is enabled.
    pub fn is_won(&self) -> bool {
        self.victories >= self.max_victories && self.enabled
    }

    /// `true` when victories are 0 (not gated by `enabled`).
    pub fn is_lost(&self) -> bool {
        self.victories == 0.0
    }

    /// Fraction of maximum victories [0.0, 1.0].
    pub fn victory_fraction(&self) -> f32 {
        (self.victories / self.max_victories).clamp(0.0, 1.0)
    }

    /// Returns `scale * victory_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_glory(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.victory_fraction()
    }
}

impl Default for Won {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Won {
        Won::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_lost() {
        let w = w();
        assert_eq!(w.victories, 0.0);
        assert!(w.is_lost());
        assert!(!w.is_won());
    }

    #[test]
    fn new_clamps_max_victories() {
        let w = Won::new(-5.0, 1.5);
        assert!((w.max_victories - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_streak_rate() {
        let w = Won::new(100.0, -1.5);
        assert_eq!(w.streak_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Won::default();
        assert!((w.max_victories - 100.0).abs() < 1e-5);
        assert!((w.streak_rate - 1.5).abs() < 1e-5);
    }

    // --- claim ---

    #[test]
    fn claim_adds_victories() {
        let mut w = w();
        w.claim(40.0);
        assert!((w.victories - 40.0).abs() < 1e-3);
    }

    #[test]
    fn claim_clamps_at_max() {
        let mut w = w();
        w.claim(200.0);
        assert!((w.victories - 100.0).abs() < 1e-3);
    }

    #[test]
    fn claim_fires_just_won_at_max() {
        let mut w = w();
        w.claim(100.0);
        assert!(w.just_won);
        assert!(w.is_won());
    }

    #[test]
    fn claim_no_just_won_when_already_at_max() {
        let mut w = w();
        w.victories = 100.0;
        w.claim(10.0);
        assert!(!w.just_won);
    }

    #[test]
    fn claim_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.claim(50.0);
        assert_eq!(w.victories, 0.0);
    }

    #[test]
    fn claim_no_op_when_amount_zero() {
        let mut w = w();
        w.claim(0.0);
        assert_eq!(w.victories, 0.0);
    }

    // --- concede ---

    #[test]
    fn concede_reduces_victories() {
        let mut w = w();
        w.victories = 60.0;
        w.concede(20.0);
        assert!((w.victories - 40.0).abs() < 1e-3);
    }

    #[test]
    fn concede_clamps_at_zero() {
        let mut w = w();
        w.victories = 30.0;
        w.concede(200.0);
        assert_eq!(w.victories, 0.0);
    }

    #[test]
    fn concede_fires_just_lost_at_zero() {
        let mut w = w();
        w.victories = 30.0;
        w.concede(30.0);
        assert!(w.just_lost);
    }

    #[test]
    fn concede_no_op_when_already_lost() {
        let mut w = w();
        w.concede(10.0);
        assert!(!w.just_lost);
    }

    #[test]
    fn concede_no_op_when_disabled() {
        let mut w = w();
        w.victories = 50.0;
        w.enabled = false;
        w.concede(50.0);
        assert!((w.victories - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_victories() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.victories - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_won_on_victories_to_max() {
        let mut w = Won::new(100.0, 200.0);
        w.victories = 95.0;
        w.tick(1.0);
        assert!(w.just_won);
        assert!(w.is_won());
    }

    #[test]
    fn tick_no_build_when_already_won() {
        let mut w = w();
        w.victories = 100.0;
        w.tick(1.0);
        assert!(!w.just_won);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Won::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.victories, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.victories, 0.0);
    }

    #[test]
    fn tick_clears_just_won() {
        let mut w = Won::new(100.0, 200.0);
        w.victories = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_won);
    }

    #[test]
    fn tick_clears_just_lost() {
        let mut w = w();
        w.victories = 10.0;
        w.concede(10.0);
        w.tick(0.016);
        assert!(!w.just_lost);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.victories - 9.0).abs() < 1e-3);
    }

    // --- is_won / is_lost ---

    #[test]
    fn is_won_false_when_disabled() {
        let mut w = w();
        w.victories = 100.0;
        w.enabled = false;
        assert!(!w.is_won());
    }

    #[test]
    fn is_lost_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_lost());
    }

    // --- victory_fraction / effective_glory ---

    #[test]
    fn victory_fraction_zero_when_lost() {
        assert_eq!(w().victory_fraction(), 0.0);
    }

    #[test]
    fn victory_fraction_half_at_midpoint() {
        let mut w = w();
        w.victories = 50.0;
        assert!((w.victory_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_glory_zero_when_lost() {
        assert_eq!(w().effective_glory(100.0), 0.0);
    }

    #[test]
    fn effective_glory_scales_with_victories() {
        let mut w = w();
        w.victories = 75.0;
        assert!((w.effective_glory(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_glory_zero_when_disabled() {
        let mut w = w();
        w.victories = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_glory(100.0), 0.0);
    }
}
