use bevy_ecs::prelude::Component;

/// Social-gossip and notoriety tracker. `gossip` accumulates as rumours
/// spread via `spread(amount)` and fades passively at `fade_rate` per second
/// via `tick(dt)`. Active suppression is available via `quell(amount)`.
///
/// Models reputation spread, rumour propagation, social heat, NPC awareness
/// of a player's misdeeds, or any mechanic where notoriety builds from events
/// and fades when nothing happens.
///
/// `spread(amount)` adds to gossip (capped at `max_gossip`). Fires
/// `just_notorious` on first reaching max. No-op when disabled.
///
/// `quell(amount)` reduces gossip when above 0. Fires `just_forgotten`
/// when gossip reaches 0. No-op when disabled.
///
/// `tick(dt)` clears `just_notorious` and `just_forgotten`. Then (when
/// enabled and `fade_rate > 0`) reduces gossip by `fade_rate * dt`, floored
/// at 0. Fires `just_forgotten` if gossip reaches 0 via fading.
///
/// `is_notorious()` returns `gossip >= max_gossip && enabled`.
///
/// `is_forgotten()` returns `gossip == 0.0` (not gated by `enabled`).
///
/// `gossip_fraction()` returns `(gossip / max_gossip).clamp(0, 1)`.
///
/// `effective_influence(base)` returns `base * gossip_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 5.0)` — starts forgotten, fades at 5/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Yenta {
    pub gossip: f32,
    pub max_gossip: f32,
    pub fade_rate: f32,
    pub just_notorious: bool,
    pub just_forgotten: bool,
    pub enabled: bool,
}

impl Yenta {
    pub fn new(max_gossip: f32, fade_rate: f32) -> Self {
        Self {
            gossip: 0.0,
            max_gossip: max_gossip.max(0.1),
            fade_rate: fade_rate.max(0.0),
            just_notorious: false,
            just_forgotten: false,
            enabled: true,
        }
    }

    /// Spread rumours; fires `just_notorious` on first reaching max.
    /// No-op when disabled or already at max.
    pub fn spread(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.gossip >= self.max_gossip {
            return;
        }
        self.gossip = (self.gossip + amount).min(self.max_gossip);
        if self.gossip >= self.max_gossip {
            self.just_notorious = true;
        }
    }

    /// Suppress rumours; fires `just_forgotten` when reaching 0.
    /// No-op when disabled or already forgotten.
    pub fn quell(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.gossip <= 0.0 {
            return;
        }
        self.gossip = (self.gossip - amount).max(0.0);
        if self.gossip <= 0.0 {
            self.just_forgotten = true;
        }
    }

    /// Advance one frame: clear flags, then fade gossip passively when
    /// enabled and `fade_rate > 0`. Fires `just_forgotten` if gossip hits 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_notorious = false;
        self.just_forgotten = false;
        if self.enabled && self.fade_rate > 0.0 && self.gossip > 0.0 {
            self.gossip = (self.gossip - self.fade_rate * dt).max(0.0);
            if self.gossip <= 0.0 {
                self.just_forgotten = true;
            }
        }
    }

    /// `true` when gossip is at maximum and component is enabled.
    pub fn is_notorious(&self) -> bool {
        self.gossip >= self.max_gossip && self.enabled
    }

    /// `true` when gossip is 0 (not gated by `enabled`).
    pub fn is_forgotten(&self) -> bool {
        self.gossip == 0.0
    }

    /// Fraction of maximum gossip [0.0, 1.0].
    pub fn gossip_fraction(&self) -> f32 {
        (self.gossip / self.max_gossip).clamp(0.0, 1.0)
    }

    /// Returns `base * gossip_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_influence(&self, base: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        base * self.gossip_fraction()
    }
}

impl Default for Yenta {
    fn default() -> Self {
        Self::new(100.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn y() -> Yenta {
        Yenta::new(100.0, 10.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_forgotten() {
        let y = y();
        assert_eq!(y.gossip, 0.0);
        assert!(y.is_forgotten());
        assert!(!y.is_notorious());
    }

    #[test]
    fn new_clamps_max_gossip() {
        let y = Yenta::new(-5.0, 1.0);
        assert!((y.max_gossip - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fade_rate() {
        let y = Yenta::new(100.0, -3.0);
        assert_eq!(y.fade_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let y = Yenta::default();
        assert!((y.max_gossip - 100.0).abs() < 1e-5);
        assert!((y.fade_rate - 5.0).abs() < 1e-5);
        assert_eq!(y.gossip, 0.0);
    }

    // --- spread ---

    #[test]
    fn spread_increases_gossip() {
        let mut y = y();
        y.spread(40.0);
        assert!((y.gossip - 40.0).abs() < 1e-4);
    }

    #[test]
    fn spread_clamps_at_max() {
        let mut y = y();
        y.spread(200.0);
        assert!((y.gossip - 100.0).abs() < 1e-5);
    }

    #[test]
    fn spread_fires_just_notorious_at_max() {
        let mut y = y();
        y.spread(100.0);
        assert!(y.just_notorious);
        assert!(y.is_notorious());
    }

    #[test]
    fn spread_no_refire_when_at_max() {
        let mut y = y();
        y.spread(100.0);
        y.spread(10.0); // already at max
        assert!(y.just_notorious);
    }

    #[test]
    fn spread_no_op_when_disabled() {
        let mut y = y();
        y.enabled = false;
        y.spread(50.0);
        assert_eq!(y.gossip, 0.0);
    }

    #[test]
    fn spread_no_op_for_zero_amount() {
        let mut y = y();
        y.spread(0.0);
        assert_eq!(y.gossip, 0.0);
    }

    #[test]
    fn spread_accumulates() {
        let mut y = y();
        y.spread(30.0);
        y.spread(25.0);
        assert!((y.gossip - 55.0).abs() < 1e-3);
    }

    // --- quell ---

    #[test]
    fn quell_reduces_gossip() {
        let mut y = y();
        y.spread(70.0);
        y.quell(20.0);
        assert!((y.gossip - 50.0).abs() < 1e-3);
    }

    #[test]
    fn quell_clamps_at_zero() {
        let mut y = y();
        y.spread(30.0);
        y.quell(200.0);
        assert_eq!(y.gossip, 0.0);
    }

    #[test]
    fn quell_fires_just_forgotten_at_zero() {
        let mut y = y();
        y.spread(30.0);
        y.quell(30.0);
        assert!(y.just_forgotten);
        assert!(y.is_forgotten());
    }

    #[test]
    fn quell_no_op_when_already_forgotten() {
        let mut y = y();
        y.quell(10.0); // already 0
        assert!(!y.just_forgotten);
    }

    #[test]
    fn quell_no_op_when_disabled() {
        let mut y = y();
        y.spread(50.0);
        y.enabled = false;
        y.quell(50.0);
        assert!((y.gossip - 50.0).abs() < 1e-3);
    }

    #[test]
    fn quell_no_op_for_zero_amount() {
        let mut y = y();
        y.spread(50.0);
        y.quell(0.0);
        assert!((y.gossip - 50.0).abs() < 1e-3);
    }

    // --- tick (passive fade) ---

    #[test]
    fn tick_fades_gossip() {
        let mut y = y(); // fade_rate = 10
        y.spread(60.0);
        y.tick(1.0); // 60 - 10*1 = 50
        assert!((y.gossip - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clamps_at_zero() {
        let mut y = y();
        y.spread(5.0);
        y.tick(100.0);
        assert_eq!(y.gossip, 0.0);
    }

    #[test]
    fn tick_fires_just_forgotten_on_fade_to_zero() {
        let mut y = y();
        y.spread(5.0);
        y.tick(1.0); // fades 10*1 = 10 → 0
        assert!(y.just_forgotten);
    }

    #[test]
    fn tick_no_fade_when_forgotten() {
        let mut y = y();
        y.tick(100.0); // gossip=0, nothing to fade
        assert!(!y.just_forgotten);
    }

    #[test]
    fn tick_no_fade_when_rate_zero() {
        let mut y = Yenta::new(100.0, 0.0);
        y.spread(50.0);
        y.tick(100.0);
        assert!((y.gossip - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_no_fade_when_disabled() {
        let mut y = y();
        y.spread(50.0);
        y.enabled = false;
        y.tick(1.0);
        assert!((y.gossip - 50.0).abs() < 1e-3);
    }

    #[test]
    fn tick_clears_just_notorious() {
        let mut y = y();
        y.spread(100.0);
        y.tick(0.016);
        assert!(!y.just_notorious);
    }

    #[test]
    fn tick_clears_just_forgotten() {
        let mut y = y();
        y.spread(5.0);
        y.tick(1.0); // just_forgotten fires
        y.tick(0.016); // cleared
        assert!(!y.just_forgotten);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut y = y();
        y.spread(80.0);
        y.tick(2.0); // 80 - 10*2 = 60
        assert!((y.gossip - 60.0).abs() < 1e-2);
    }

    // --- is_notorious / is_forgotten ---

    #[test]
    fn is_notorious_false_below_max() {
        let mut y = y();
        y.spread(50.0);
        assert!(!y.is_notorious());
    }

    #[test]
    fn is_notorious_false_when_disabled() {
        let mut y = y();
        y.spread(100.0);
        y.enabled = false;
        assert!(!y.is_notorious());
    }

    #[test]
    fn is_forgotten_true_at_start() {
        assert!(y().is_forgotten());
    }

    #[test]
    fn is_forgotten_not_gated_by_enabled() {
        let mut y = y();
        y.enabled = false;
        assert!(y.is_forgotten());
    }

    // --- fractions / effective ---

    #[test]
    fn gossip_fraction_zero_when_forgotten() {
        assert_eq!(y().gossip_fraction(), 0.0);
    }

    #[test]
    fn gossip_fraction_half_at_midpoint() {
        let mut y = y();
        y.spread(50.0);
        assert!((y.gossip_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_influence_zero_when_forgotten() {
        assert_eq!(y().effective_influence(100.0), 0.0);
    }

    #[test]
    fn effective_influence_scales_with_fraction() {
        let mut y = y();
        y.spread(75.0);
        assert!((y.effective_influence(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_influence_zero_when_disabled() {
        let mut y = y();
        y.spread(50.0);
        y.enabled = false;
        assert_eq!(y.effective_influence(100.0), 0.0);
    }
}
