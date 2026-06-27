use bevy_ecs::prelude::Component;

/// Concentration-channel heal: while knitting, the entity recovers
/// `heal_rate` HP per second. Any single hit that meets or exceeds
/// `interruption_threshold` damage breaks the channel immediately, setting
/// `just_interrupted`. If the channel completes naturally, `just_completed`
/// is set.
///
/// `begin(duration)` starts or extends the channel (high-watermark); sets
/// `just_began` on the inactive → active transition. `interrupt_if(damage)`
/// should be called by the damage system: it cancels the channel and sets
/// `just_interrupted` when `damage >= interruption_threshold`.
/// `tick(dt)` advances the timer and returns the HP healed this frame.
///
/// Distinct from `Regen` (always-on passive, never interrupted), `Revive`
/// (post-death), and `Survival` (killing-blow negation): Knit is a
/// **fragile concentration channel** — it heals fast but cancels on any
/// significant hit, forcing the entity to find a safe window to use it.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Knit {
    pub duration: f32,
    pub timer: f32,
    /// HP restored per second while the channel is active. Clamped ≥ 0.0.
    pub heal_rate: f32,
    /// Minimum damage that breaks the channel. Clamped ≥ 0.0.
    pub interruption_threshold: f32,
    pub just_began: bool,
    pub just_completed: bool,
    pub just_interrupted: bool,
    pub enabled: bool,
}

impl Knit {
    pub fn new(heal_rate: f32, interruption_threshold: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            heal_rate: heal_rate.max(0.0),
            interruption_threshold: interruption_threshold.max(0.0),
            just_began: false,
            just_completed: false,
            just_interrupted: false,
            enabled: true,
        }
    }

    /// Start or extend the heal channel for `duration` seconds. High-watermark:
    /// only replaces the timer when `duration > timer`. Sets `just_began` on
    /// the inactive → active transition. No-op when disabled or `duration ≤ 0`.
    pub fn begin(&mut self, duration: f32) {
        if !self.enabled || duration <= 0.0 {
            return;
        }
        if duration > self.timer {
            let was_knitting = self.is_knitting();
            self.duration = duration;
            self.timer = duration;
            if !was_knitting {
                self.just_began = true;
            }
        }
    }

    /// Called by the damage system. Breaks the channel and sets
    /// `just_interrupted` if `damage >= interruption_threshold`. No-op when
    /// not knitting or `damage < threshold`.
    pub fn interrupt_if(&mut self, damage: f32) {
        if !self.is_knitting() || damage < self.interruption_threshold {
            return;
        }
        self.timer = 0.0;
        self.duration = 0.0;
        self.just_interrupted = true;
    }

    /// Advance the channel timer. Returns the HP healed this frame
    /// (`heal_rate * dt` when active and enabled, 0.0 otherwise). Sets
    /// `just_completed` when the channel expires naturally. Clears one-frame
    /// flags at the start of each tick.
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_began = false;
        self.just_completed = false;
        self.just_interrupted = false;

        if self.timer <= 0.0 || !self.enabled {
            return 0.0;
        }

        let healed = self.heal_rate * dt;
        self.timer -= dt;
        if self.timer <= 0.0 {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_completed = true;
        }
        healed
    }

    pub fn is_knitting(&self) -> bool {
        self.timer > 0.0
    }

    /// Fraction of the channel duration remaining [1.0 = just began, 0.0 = ended].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Knit {
    fn default() -> Self {
        Self::new(10.0, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_starts_channel() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(3.0);
        assert!(k.is_knitting());
        assert!(k.just_began);
    }

    #[test]
    fn begin_extends_on_longer_duration() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(2.0);
        k.tick(0.016);
        k.begin(6.0);
        assert!((k.timer - 6.0).abs() < 1e-4);
    }

    #[test]
    fn begin_no_extend_on_shorter_duration() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(6.0);
        k.begin(2.0);
        assert!((k.timer - 6.0).abs() < 1e-4);
    }

    #[test]
    fn just_began_not_set_on_extend() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(2.0);
        k.tick(0.016);
        k.begin(6.0);
        assert!(!k.just_began);
    }

    #[test]
    fn interrupt_if_breaks_channel_at_threshold() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(3.0);
        k.interrupt_if(5.0);
        assert!(!k.is_knitting());
        assert!(k.just_interrupted);
    }

    #[test]
    fn interrupt_if_no_op_below_threshold() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(3.0);
        k.interrupt_if(4.9);
        assert!(k.is_knitting());
        assert!(!k.just_interrupted);
    }

    #[test]
    fn interrupt_if_no_op_when_not_knitting() {
        let mut k = Knit::new(10.0, 5.0);
        k.interrupt_if(100.0);
        assert!(!k.just_interrupted);
    }

    #[test]
    fn tick_returns_heal_amount() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(5.0);
        let healed = k.tick(0.5);
        assert!((healed - 5.0).abs() < 1e-4); // 10.0 * 0.5
    }

    #[test]
    fn tick_returns_zero_when_not_knitting() {
        let mut k = Knit::new(10.0, 5.0);
        let healed = k.tick(1.0);
        assert!((healed).abs() < 1e-5);
    }

    #[test]
    fn tick_sets_just_completed_on_expiry() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(1.0);
        k.tick(1.1);
        assert!(!k.is_knitting());
        assert!(k.just_completed);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(5.0);
        k.tick(0.016);
        assert!(!k.just_began);
    }

    #[test]
    fn tick_clears_just_completed() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(0.5);
        k.tick(1.0);
        k.tick(0.016);
        assert!(!k.just_completed);
    }

    #[test]
    fn tick_clears_just_interrupted() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(5.0);
        k.interrupt_if(10.0);
        k.tick(0.016);
        assert!(!k.just_interrupted);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(4.0);
        k.tick(2.0);
        assert!((k.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_zero_when_not_knitting() {
        let k = Knit::new(10.0, 5.0);
        assert!((k.remaining_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_begin_no_op() {
        let mut k = Knit::new(10.0, 5.0);
        k.enabled = false;
        k.begin(5.0);
        assert!(!k.is_knitting());
    }

    #[test]
    fn disabled_tick_returns_zero() {
        let mut k = Knit::new(10.0, 5.0);
        k.begin(5.0);
        k.enabled = false;
        let healed = k.tick(1.0);
        assert!((healed).abs() < 1e-5);
    }
}
