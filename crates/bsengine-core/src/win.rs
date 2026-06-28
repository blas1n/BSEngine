use bevy_ecs::prelude::Component;

/// Victory-accumulation tracker named after win, the verb
/// meaning to be successful or victorious in a contest or
/// conflict; to gain or acquire by effort or competition;
/// to reach a desired end — from the Old English winnan
/// (to strive, to struggle, to fight, to endure, to toil
/// for), from the Proto-Germanic winnaną (to struggle, to
/// suffer, to strive), from the Proto-Indo-European root
/// wen- (to desire, to strive for, to win). The root wen-
/// also gave wish, want, Venus, and venerate — all words
/// about directed desire and its pursuit. Win, in its
/// oldest English sense, is not primarily about receiving
/// a prize but about surviving a struggle: the Old English
/// winna was one who endured, one who persisted through
/// difficulty, one who continued to strive after others
/// had stopped. The modern sense — winning as the moment
/// of triumph rather than the process of endurance — is
/// a narrowing: to win is now to arrive at the successful
/// outcome of a contest, not to describe the sustained
/// effort that produced it. In game mechanics, a win
/// mechanic models the accumulation of competitive
/// advantage — the build of points, victories, streaks,
/// or dominance that eventually reaches the threshold at
/// which a match is decided, a tournament is claimed, or
/// a competitive state is achieved. `score` builds via
/// `gain(amount)` and accumulates passively at `win_rate`
/// per second in `tick(dt)` or is forfeited via
/// `forfeit(amount)`.
///
/// Models score-accumulation fill levels, victory-saturation
/// bars, points-accumulation trackers, dominance-build gauges,
/// competitive-advantage fill levels, triumph-saturation
/// indicators, match-point accumulation bars, bracket-
/// completion meters, championship-fill levels, or any
/// mechanic where a character, team, or entity slowly
/// accumulates the points, victories, or competitive
/// dominance required to win a match, claim a tournament,
/// or achieve a threshold of outright victory.
///
/// `gain(amount)` adds score; fires `just_won` when first
/// reaching `max_score`. No-op when disabled.
///
/// `forfeit(amount)` reduces score immediately; fires
/// `just_lost` when reaching 0. No-op when disabled or
/// already lost.
///
/// `tick(dt)` clears both flags, then increases score by
/// `win_rate * dt` (capped at `max_score`). Fires
/// `just_won` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_won()` returns `score >= max_score && enabled`.
///
/// `is_lost()` returns `score == 0.0` (not gated by
/// `enabled`).
///
/// `score_fraction()` returns
/// `(score / max_score).clamp(0, 1)`.
///
/// `effective_lead(scale)` returns `scale * score_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — gains at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Win {
    pub score: f32,
    pub max_score: f32,
    pub win_rate: f32,
    pub just_won: bool,
    pub just_lost: bool,
    pub enabled: bool,
}

impl Win {
    pub fn new(max_score: f32, win_rate: f32) -> Self {
        Self {
            score: 0.0,
            max_score: max_score.max(0.1),
            win_rate: win_rate.max(0.0),
            just_won: false,
            just_lost: false,
            enabled: true,
        }
    }

    /// Add score; fires `just_won` when first reaching max.
    /// No-op when disabled.
    pub fn gain(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.score < self.max_score;
        self.score = (self.score + amount).min(self.max_score);
        if was_below && self.score >= self.max_score {
            self.just_won = true;
        }
    }

    /// Reduce score; fires `just_lost` when reaching 0.
    /// No-op when disabled or already lost.
    pub fn forfeit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.score <= 0.0 {
            return;
        }
        self.score = (self.score - amount).max(0.0);
        if self.score <= 0.0 {
            self.just_lost = true;
        }
    }

    /// Clear flags, then increase score by `win_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_won = false;
        self.just_lost = false;
        if self.enabled && self.win_rate > 0.0 && self.score < self.max_score {
            let was_below = self.score < self.max_score;
            self.score = (self.score + self.win_rate * dt).min(self.max_score);
            if was_below && self.score >= self.max_score {
                self.just_won = true;
            }
        }
    }

    /// `true` when score is at maximum and component is enabled.
    pub fn is_won(&self) -> bool {
        self.score >= self.max_score && self.enabled
    }

    /// `true` when score is 0 (not gated by `enabled`).
    pub fn is_lost(&self) -> bool {
        self.score == 0.0
    }

    /// Fraction of maximum score [0.0, 1.0].
    pub fn score_fraction(&self) -> f32 {
        (self.score / self.max_score).clamp(0.0, 1.0)
    }

    /// Returns `scale * score_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_lead(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.score_fraction()
    }
}

impl Default for Win {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w() -> Win {
        Win::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_lost() {
        let w = w();
        assert_eq!(w.score, 0.0);
        assert!(w.is_lost());
        assert!(!w.is_won());
    }

    #[test]
    fn new_clamps_max_score() {
        let w = Win::new(-5.0, 1.5);
        assert!((w.max_score - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_win_rate() {
        let w = Win::new(100.0, -1.5);
        assert_eq!(w.win_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let w = Win::default();
        assert!((w.max_score - 100.0).abs() < 1e-5);
        assert!((w.win_rate - 1.5).abs() < 1e-5);
    }

    // --- gain ---

    #[test]
    fn gain_adds_score() {
        let mut w = w();
        w.gain(40.0);
        assert!((w.score - 40.0).abs() < 1e-3);
    }

    #[test]
    fn gain_clamps_at_max() {
        let mut w = w();
        w.gain(200.0);
        assert!((w.score - 100.0).abs() < 1e-3);
    }

    #[test]
    fn gain_fires_just_won_at_max() {
        let mut w = w();
        w.gain(100.0);
        assert!(w.just_won);
        assert!(w.is_won());
    }

    #[test]
    fn gain_no_just_won_when_already_at_max() {
        let mut w = w();
        w.score = 100.0;
        w.gain(10.0);
        assert!(!w.just_won);
    }

    #[test]
    fn gain_no_op_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.gain(50.0);
        assert_eq!(w.score, 0.0);
    }

    #[test]
    fn gain_no_op_when_amount_zero() {
        let mut w = w();
        w.gain(0.0);
        assert_eq!(w.score, 0.0);
    }

    // --- forfeit ---

    #[test]
    fn forfeit_reduces_score() {
        let mut w = w();
        w.score = 60.0;
        w.forfeit(20.0);
        assert!((w.score - 40.0).abs() < 1e-3);
    }

    #[test]
    fn forfeit_clamps_at_zero() {
        let mut w = w();
        w.score = 30.0;
        w.forfeit(200.0);
        assert_eq!(w.score, 0.0);
    }

    #[test]
    fn forfeit_fires_just_lost_at_zero() {
        let mut w = w();
        w.score = 30.0;
        w.forfeit(30.0);
        assert!(w.just_lost);
    }

    #[test]
    fn forfeit_no_op_when_already_lost() {
        let mut w = w();
        w.forfeit(10.0);
        assert!(!w.just_lost);
    }

    #[test]
    fn forfeit_no_op_when_disabled() {
        let mut w = w();
        w.score = 50.0;
        w.enabled = false;
        w.forfeit(50.0);
        assert!((w.score - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_score() {
        let mut w = w(); // rate=1.5
        w.tick(4.0); // 0 + 1.5*4 = 6
        assert!((w.score - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_won_on_score_to_max() {
        let mut w = Win::new(100.0, 200.0);
        w.score = 95.0;
        w.tick(1.0);
        assert!(w.just_won);
        assert!(w.is_won());
    }

    #[test]
    fn tick_no_build_when_already_won() {
        let mut w = w();
        w.score = 100.0;
        w.tick(1.0);
        assert!(!w.just_won);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut w = Win::new(100.0, 0.0);
        w.tick(100.0);
        assert_eq!(w.score, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut w = w();
        w.enabled = false;
        w.tick(1.0);
        assert_eq!(w.score, 0.0);
    }

    #[test]
    fn tick_clears_just_won() {
        let mut w = Win::new(100.0, 200.0);
        w.score = 95.0;
        w.tick(1.0);
        w.tick(0.016);
        assert!(!w.just_won);
    }

    #[test]
    fn tick_clears_just_lost() {
        let mut w = w();
        w.score = 10.0;
        w.forfeit(10.0);
        w.tick(0.016);
        assert!(!w.just_lost);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut w = w(); // rate=1.5
        w.tick(6.0); // 1.5*6 = 9
        assert!((w.score - 9.0).abs() < 1e-3);
    }

    // --- is_won / is_lost ---

    #[test]
    fn is_won_false_when_disabled() {
        let mut w = w();
        w.score = 100.0;
        w.enabled = false;
        assert!(!w.is_won());
    }

    #[test]
    fn is_lost_not_gated_by_enabled() {
        let mut w = w();
        w.enabled = false;
        assert!(w.is_lost());
    }

    // --- score_fraction / effective_lead ---

    #[test]
    fn score_fraction_zero_when_lost() {
        assert_eq!(w().score_fraction(), 0.0);
    }

    #[test]
    fn score_fraction_half_at_midpoint() {
        let mut w = w();
        w.score = 50.0;
        assert!((w.score_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_lead_zero_when_lost() {
        assert_eq!(w().effective_lead(100.0), 0.0);
    }

    #[test]
    fn effective_lead_scales_with_score() {
        let mut w = w();
        w.score = 75.0;
        assert!((w.effective_lead(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_lead_zero_when_disabled() {
        let mut w = w();
        w.score = 50.0;
        w.enabled = false;
        assert_eq!(w.effective_lead(100.0), 0.0);
    }
}
