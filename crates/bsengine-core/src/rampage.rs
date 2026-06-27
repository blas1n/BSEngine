use bevy_ecs::prelude::Component;

/// Kill-triggered escalating frenzy: each kill adds a stack (up to
/// `max_stacks`), boosting damage and speed. If `decay_interval` seconds pass
/// without a kill, one stack is shed; the entity calms down gradually.
///
/// `kill()` registers a kill: adds a stack and resets the decay timer.
/// `tick(dt)` advances the timer and removes one stack per elapsed
/// `decay_interval`. Sets `just_stacked` on the frame a kill is registered,
/// and `just_ended` on the frame stacks drop to 0. One-frame flags are
/// cleared at the start of each `tick`.
///
/// Distinct from `Rage` (anger/berserk state, not kill-gated), `Fervor`
/// (spiritual intensity), and `Surge` (one-time stat burst): Rampage is
/// **kill-triggered escalation** — the entity grows progressively more
/// dangerous as long as it keeps killing, and cools down when it stops.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rampage {
    pub stacks: u32,
    pub max_stacks: u32,
    /// Fraction of base damage added per stack. Clamped ≥ 0.0.
    /// e.g. 0.2 at 3 stacks → 160% damage.
    pub damage_per_stack: f32,
    /// Fraction of base speed added per stack. Clamped ≥ 0.0.
    /// e.g. 0.1 at 3 stacks → 130% speed.
    pub speed_per_stack: f32,
    /// Seconds without a kill before one stack decays. Clamped ≥ 0.0.
    pub decay_interval: f32,
    /// Time elapsed since the last kill (or since the last decay event).
    pub decay_timer: f32,
    pub just_stacked: bool,
    pub just_ended: bool,
    pub enabled: bool,
}

impl Rampage {
    pub fn new(
        max_stacks: u32,
        damage_per_stack: f32,
        speed_per_stack: f32,
        decay_interval: f32,
    ) -> Self {
        Self {
            stacks: 0,
            max_stacks: max_stacks.max(1),
            damage_per_stack: damage_per_stack.max(0.0),
            speed_per_stack: speed_per_stack.max(0.0),
            decay_interval: decay_interval.max(0.0),
            decay_timer: 0.0,
            just_stacked: false,
            just_ended: false,
            enabled: true,
        }
    }

    /// Register a kill: add one stack (capped at `max_stacks`) and reset the
    /// decay timer. Sets `just_stacked`. No-op when disabled.
    pub fn kill(&mut self) {
        if !self.enabled {
            return;
        }
        if self.stacks < self.max_stacks {
            self.stacks += 1;
        }
        self.decay_timer = 0.0;
        self.just_stacked = true;
    }

    /// Advance the rampage state. Clears one-frame flags first, then advances
    /// the decay timer. When the timer reaches `decay_interval`, one stack is
    /// shed and the timer resets. Sets `just_ended` when stacks drop to 0.
    pub fn tick(&mut self, dt: f32) {
        self.just_stacked = false;
        self.just_ended = false;

        if self.stacks > 0 {
            self.decay_timer += dt;
            if self.decay_interval > 0.0 && self.decay_timer >= self.decay_interval {
                self.decay_timer -= self.decay_interval;
                self.stacks -= 1;
                if self.stacks == 0 {
                    self.just_ended = true;
                    self.decay_timer = 0.0;
                }
            }
        }
    }

    pub fn is_rampaging(&self) -> bool {
        self.stacks > 0
    }

    /// Effective outgoing damage while rampaging.
    /// Returns `base * (1 + damage_per_stack * stacks)` when rampaging and
    /// enabled, `base` otherwise.
    pub fn effective_damage(&self, base: f32) -> f32 {
        if self.is_rampaging() && self.enabled {
            base * (1.0 + self.damage_per_stack * self.stacks as f32)
        } else {
            base
        }
    }

    /// Effective movement speed while rampaging.
    /// Returns `base * (1 + speed_per_stack * stacks)` when rampaging and
    /// enabled, `base` otherwise.
    pub fn effective_speed(&self, base: f32) -> f32 {
        if self.is_rampaging() && self.enabled {
            base * (1.0 + self.speed_per_stack * self.stacks as f32)
        } else {
            base
        }
    }

    /// Fraction of max stacks built up [0.0 = none, 1.0 = capped].
    pub fn stack_fraction(&self) -> f32 {
        if self.max_stacks == 0 {
            return 0.0;
        }
        (self.stacks as f32 / self.max_stacks as f32).clamp(0.0, 1.0)
    }
}

impl Default for Rampage {
    fn default() -> Self {
        Self::new(5, 0.2, 0.1, 3.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kill_adds_stack() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        assert_eq!(r.stacks, 1);
        assert!(r.is_rampaging());
        assert!(r.just_stacked);
    }

    #[test]
    fn kill_caps_at_max_stacks() {
        let mut r = Rampage::new(3, 0.2, 0.1, 3.0);
        for _ in 0..5 {
            r.kill();
        }
        assert_eq!(r.stacks, 3);
    }

    #[test]
    fn kill_resets_decay_timer() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        r.tick(1.5);
        r.kill(); // should reset timer
        assert!((r.decay_timer).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_one_stack_per_interval() {
        let mut r = Rampage::new(5, 0.2, 0.1, 2.0);
        r.kill();
        r.kill();
        r.kill();
        r.tick(2.1); // one interval elapsed → lose 1 stack
        assert_eq!(r.stacks, 2);
    }

    #[test]
    fn tick_sets_just_ended_when_stacks_reach_zero() {
        let mut r = Rampage::new(5, 0.2, 0.1, 1.0);
        r.kill();
        r.tick(1.1);
        assert_eq!(r.stacks, 0);
        assert!(r.just_ended);
        assert!(!r.is_rampaging());
    }

    #[test]
    fn tick_clears_just_stacked() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        r.tick(0.016);
        assert!(!r.just_stacked);
    }

    #[test]
    fn tick_clears_just_ended() {
        let mut r = Rampage::new(5, 0.2, 0.1, 1.0);
        r.kill();
        r.tick(1.1); // just_ended = true
        r.tick(0.016);
        assert!(!r.just_ended);
    }

    #[test]
    fn effective_damage_boosted_while_rampaging() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        r.kill(); // 2 stacks
                  // 100 * (1 + 0.2 * 2) = 140
        assert!((r.effective_damage(100.0) - 140.0).abs() < 1e-3);
    }

    #[test]
    fn effective_damage_base_when_not_rampaging() {
        let r = Rampage::new(5, 0.2, 0.1, 3.0);
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn effective_speed_boosted_while_rampaging() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        r.kill(); // 2 stacks
                  // 100 * (1 + 0.1 * 2) = 120
        assert!((r.effective_speed(100.0) - 120.0).abs() < 1e-3);
    }

    #[test]
    fn effective_speed_base_when_not_rampaging() {
        let r = Rampage::new(5, 0.2, 0.1, 3.0);
        assert!((r.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_max() {
        let mut r = Rampage::new(4, 0.2, 0.1, 3.0);
        for _ in 0..4 {
            r.kill();
        }
        assert!((r.stack_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn stack_fraction_at_half() {
        let mut r = Rampage::new(4, 0.2, 0.1, 3.0);
        r.kill();
        r.kill(); // 2 of 4
        assert!((r.stack_fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_kill_no_op() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.enabled = false;
        r.kill();
        assert_eq!(r.stacks, 0);
        assert!(!r.just_stacked);
    }

    #[test]
    fn disabled_effective_damage_base() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        r.enabled = false;
        assert!((r.effective_damage(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_effective_speed_base() {
        let mut r = Rampage::new(5, 0.2, 0.1, 3.0);
        r.kill();
        r.enabled = false;
        assert!((r.effective_speed(100.0) - 100.0).abs() < 1e-5);
    }

    #[test]
    fn no_decay_when_zero_stacks() {
        let mut r = Rampage::new(5, 0.2, 0.1, 1.0);
        r.tick(10.0); // no stacks, should not panic or change state
        assert_eq!(r.stacks, 0);
        assert!(!r.just_ended);
    }

    #[test]
    fn kill_after_end_restarts_rampage() {
        let mut r = Rampage::new(5, 0.2, 0.1, 1.0);
        r.kill();
        r.tick(1.1); // end
        r.tick(0.016);
        r.kill(); // restart
        assert!(r.is_rampaging());
        assert!(r.just_stacked);
    }
}
