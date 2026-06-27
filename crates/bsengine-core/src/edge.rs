use bevy_ecs::prelude::Component;

/// Uninterrupted-offense streak that rewards sustained pressure. While
/// the entity fights without taking hits, `edge_streak` climbs toward
/// `max_streak_time`. `effective_outgoing()` scales linearly from zero
/// bonus (no streak) up to `max_bonus` multiplier (full streak), so the
/// entity deals increasingly more damage the longer it stays untouched.
///
/// `tick(dt, in_combat)` clears one-frame flags first; increments
/// `edge_streak` while `in_combat` (capped at `max_streak_time`); fires
/// `just_peaked` on the first tick that reaches the cap. Streak holds
/// unchanged when `!in_combat`. No-op when disabled.
///
/// `on_hit_taken()` resets `edge_streak` to 0 and fires `just_broken`
/// when a hit interrupts an active streak (`edge_streak > 0`). No-op
/// when disabled.
///
/// `is_edged()` returns `edge_streak >= max_streak_time && enabled`.
///
/// `edge_fraction()` returns `(edge_streak / max_streak_time).clamp(0, 1)`.
///
/// `effective_outgoing(base)` returns
/// `base * (1.0 + max_bonus * edge_fraction())` when enabled; returns
/// `base` otherwise.
///
/// Distinct from `Combo` (hit-count milestone bonuses — every Nth hit),
/// `Rampage` (kill-count momentum burst), `Strife` (incoming-hit
/// accumulator — MORE hits = more damage), and `Proud` (high-HP flat
/// damage bonus): Edge is an **uninterrupted-offense streak multiplier** —
/// rewards sustained attacking *without* getting hit; any contact resets
/// the clock.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Edge {
    /// Seconds of uninterrupted offense accumulated so far.
    pub edge_streak: f32,
    /// Seconds needed to reach the full bonus. Clamped ≥ 1.0.
    pub max_streak_time: f32,
    /// Outgoing damage multiplier bonus at full streak. Clamped ≥ 0.0.
    /// At full edge, effective = base * (1 + max_bonus).
    pub max_bonus: f32,
    pub just_peaked: bool,
    pub just_broken: bool,
    pub enabled: bool,
}

impl Edge {
    pub fn new(max_streak_time: f32, max_bonus: f32) -> Self {
        Self {
            edge_streak: 0.0,
            max_streak_time: max_streak_time.max(1.0),
            max_bonus: max_bonus.max(0.0),
            just_peaked: false,
            just_broken: false,
            enabled: true,
        }
    }

    /// Advance the streak timer. Clears `just_peaked` and `just_broken`
    /// first; increments `edge_streak` while `in_combat` (capped at
    /// `max_streak_time`); fires `just_peaked` on first reach. Streak
    /// holds unchanged when `!in_combat`. No-op when disabled.
    pub fn tick(&mut self, dt: f32, in_combat: bool) {
        self.just_peaked = false;
        self.just_broken = false;

        if !self.enabled || !in_combat {
            return;
        }

        let was_below_peak = self.edge_streak < self.max_streak_time;
        self.edge_streak = (self.edge_streak + dt).min(self.max_streak_time);
        if was_below_peak && self.edge_streak >= self.max_streak_time {
            self.just_peaked = true;
        }
    }

    /// Register an incoming hit. Resets `edge_streak` to 0 and fires
    /// `just_broken` when a nonzero streak is interrupted. No-op when
    /// disabled.
    pub fn on_hit_taken(&mut self) {
        if !self.enabled {
            return;
        }
        if self.edge_streak > 0.0 {
            self.edge_streak = 0.0;
            self.just_broken = true;
        }
    }

    /// `true` when the streak has reached `max_streak_time` and the
    /// component is enabled.
    pub fn is_edged(&self) -> bool {
        self.edge_streak >= self.max_streak_time && self.enabled
    }

    /// Streak fill fraction [0.0 = none, 1.0 = full]. Always in [0, 1].
    pub fn edge_fraction(&self) -> f32 {
        (self.edge_streak / self.max_streak_time).clamp(0.0, 1.0)
    }

    /// Effective outgoing damage scaled by current streak.
    /// Returns `base * (1.0 + max_bonus * edge_fraction())` when enabled.
    /// Returns `base` when disabled.
    pub fn effective_outgoing(&self, base: f32) -> f32 {
        if !self.enabled {
            return base;
        }
        base * (1.0 + self.max_bonus * self.edge_fraction())
    }
}

impl Default for Edge {
    fn default() -> Self {
        Self::new(10.0, 0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_at_zero_streak() {
        let e = Edge::new(10.0, 0.5);
        assert_eq!(e.edge_streak, 0.0);
        assert!(!e.is_edged());
    }

    #[test]
    fn tick_increments_streak_in_combat() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(3.0, true);
        assert!((e.edge_streak - 3.0).abs() < 1e-5);
    }

    #[test]
    fn tick_caps_at_max_streak_time() {
        let mut e = Edge::new(5.0, 0.5);
        e.tick(100.0, true);
        assert!((e.edge_streak - 5.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_peaked_on_first_reach() {
        let mut e = Edge::new(5.0, 0.5);
        e.tick(5.0, true);
        assert!(e.just_peaked);
        assert!(e.is_edged());
    }

    #[test]
    fn tick_no_just_peaked_when_already_edged() {
        let mut e = Edge::new(3.0, 0.5);
        e.tick(3.0, true); // peaks
        e.tick(0.016, true); // still edged, flag cleared
        assert!(!e.just_peaked);
    }

    #[test]
    fn tick_holds_streak_when_not_in_combat() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(4.0, true);
        e.tick(2.0, false); // streak should hold at 4
        assert!((e.edge_streak - 4.0).abs() < 1e-5);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut e = Edge::new(10.0, 0.5);
        e.enabled = false;
        e.tick(10.0, true);
        assert_eq!(e.edge_streak, 0.0);
    }

    #[test]
    fn tick_clears_just_peaked_each_frame() {
        let mut e = Edge::new(3.0, 0.5);
        e.tick(3.0, true); // just_peaked = true
        e.tick(0.016, true); // cleared
        assert!(!e.just_peaked);
    }

    #[test]
    fn tick_clears_just_broken_each_frame() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(5.0, true);
        e.on_hit_taken(); // just_broken = true
        e.tick(0.016, true); // cleared
        assert!(!e.just_broken);
    }

    #[test]
    fn on_hit_taken_resets_streak() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(5.0, true);
        e.on_hit_taken();
        assert_eq!(e.edge_streak, 0.0);
        assert!(e.just_broken);
    }

    #[test]
    fn on_hit_taken_no_just_broken_when_streak_zero() {
        let mut e = Edge::new(10.0, 0.5);
        e.on_hit_taken();
        assert!(!e.just_broken);
        assert_eq!(e.edge_streak, 0.0);
    }

    #[test]
    fn on_hit_taken_no_op_when_disabled() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(5.0, true);
        e.enabled = false;
        e.on_hit_taken();
        assert!((e.edge_streak - 5.0).abs() < 1e-5);
        assert!(!e.just_broken);
    }

    #[test]
    fn on_hit_taken_resets_from_edged() {
        let mut e = Edge::new(5.0, 0.5);
        e.tick(5.0, true); // edged
        e.on_hit_taken();
        assert!(!e.is_edged());
        assert!(e.just_broken);
    }

    #[test]
    fn is_edged_false_before_peak() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(5.0, true);
        assert!(!e.is_edged());
    }

    #[test]
    fn is_edged_false_when_disabled() {
        let mut e = Edge::new(5.0, 0.5);
        e.edge_streak = 5.0;
        e.enabled = false;
        assert!(!e.is_edged());
    }

    #[test]
    fn is_edged_at_exact_max() {
        let mut e = Edge::new(5.0, 0.5);
        e.tick(5.0, true);
        assert!(e.is_edged());
    }

    #[test]
    fn edge_fraction_at_zero() {
        let e = Edge::new(10.0, 0.5);
        assert_eq!(e.edge_fraction(), 0.0);
    }

    #[test]
    fn edge_fraction_at_half() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(5.0, true);
        assert!((e.edge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn edge_fraction_at_full() {
        let mut e = Edge::new(4.0, 0.5);
        e.tick(4.0, true);
        assert!((e.edge_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_at_zero_streak() {
        let e = Edge::new(10.0, 0.5);
        assert!((e.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_at_half_streak() {
        let mut e = Edge::new(10.0, 0.5);
        e.tick(5.0, true); // 0.5 fraction → bonus = 0.5 * 0.5 = 0.25
                           // 100 * (1 + 0.25) = 125
        assert!((e.effective_outgoing(100.0) - 125.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_at_full_streak() {
        let mut e = Edge::new(5.0, 0.5);
        e.tick(5.0, true); // full → 100 * 1.5 = 150
        assert!((e.effective_outgoing(100.0) - 150.0).abs() < 1e-3);
    }

    #[test]
    fn effective_outgoing_base_when_disabled() {
        let mut e = Edge::new(5.0, 0.5);
        e.edge_streak = 5.0;
        e.enabled = false;
        assert!((e.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_outgoing_base_after_hit_resets_streak() {
        let mut e = Edge::new(5.0, 1.0);
        e.tick(5.0, true); // full bonus
        e.on_hit_taken(); // reset
        assert!((e.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn streak_rebuilds_after_hit() {
        let mut e = Edge::new(5.0, 0.5);
        e.tick(5.0, true); // edged
        e.on_hit_taken(); // broken
        e.tick(2.5, true); // half way back
        assert!((e.edge_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn max_streak_time_clamped_to_one() {
        let e = Edge::new(0.0, 0.5);
        assert!((e.max_streak_time - 1.0).abs() < 1e-5);
    }

    #[test]
    fn max_bonus_clamped_non_negative() {
        let e = Edge::new(10.0, -1.0);
        assert_eq!(e.max_bonus, 0.0);
    }

    #[test]
    fn zero_bonus_outgoing_unchanged() {
        let mut e = Edge::new(5.0, 0.0);
        e.tick(5.0, true);
        assert!((e.effective_outgoing(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn re_peaks_after_hit_and_rebuild() {
        let mut e = Edge::new(3.0, 0.5);
        e.tick(3.0, true); // peaked
        e.tick(0.016, true); // flag cleared
        e.on_hit_taken(); // broken
        e.tick(0.016, true); // flag cleared
        e.tick(3.0, true); // peaks again
        assert!(e.just_peaked);
    }
}
