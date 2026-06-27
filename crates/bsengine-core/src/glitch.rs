use bevy_ecs::prelude::Component;

/// Cyber/digital debuff that causes random system malfunctions each frame.
///
/// While glitched, game systems call `check_malfunction(rng, dt)` to test
/// whether a random error fires this frame. On a hit, the caller decides
/// which system to disrupt (skill activation, rendering, input, etc.).
/// VFX/audio hooks use `just_glitched` for the onset visual.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_cleared` on expiry.
///
/// The per-frame probability is approximated as `error_rate * dt`, so
/// `error_rate = 2.0` means roughly 2 malfunctions per second on average.
/// Values above 1.0 are valid (higher = more frequent errors).
///
/// Distinct from `Confuse` (random misfires on aimed actions) and `Silence`
/// (flat ability block): Glitch is probabilistic per-frame and not tied to
/// any specific action type.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Glitch {
    pub duration: f32,
    pub timer: f32,
    /// Expected malfunctions per second (can exceed 1.0).
    pub error_rate: f32,
    pub just_glitched: bool,
    pub just_cleared: bool,
    pub enabled: bool,
}

impl Glitch {
    pub fn new(error_rate: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            error_rate: error_rate.max(0.0),
            just_glitched: false,
            just_cleared: false,
            enabled: true,
        }
    }

    /// Apply or extend the glitch for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_glitched = true;
            }
        }
    }

    /// Clear the glitch immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_cleared = true;
        }
    }

    /// Advance the timer; sets `just_cleared` when the effect expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_glitched = false;
        self.just_cleared = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_cleared = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` if a malfunction fires this frame. `rng_value` is a
    /// uniform random in `[0.0, 1.0)`. Per-frame threshold: `error_rate * dt`.
    pub fn check_malfunction(&self, rng_value: f32, dt: f32) -> bool {
        self.is_active() && rng_value < (self.error_rate * dt)
    }

    /// Fraction of the glitch duration remaining [1.0 = just applied, 0.0 = cleared].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Glitch {
    fn default() -> Self {
        Self::new(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_glitch() {
        let mut g = Glitch::new(1.0);
        g.apply(3.0);
        assert!(g.is_active());
        assert!(g.just_glitched);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut g = Glitch::new(1.0);
        g.apply(2.0);
        g.tick(0.016);
        g.apply(5.0);
        assert!((g.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut g = Glitch::new(1.0);
        g.apply(5.0);
        g.apply(2.0);
        assert!((g.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_glitch() {
        let mut g = Glitch::new(1.0);
        g.apply(1.0);
        g.tick(1.1);
        assert!(!g.is_active());
        assert!(g.just_cleared);
    }

    #[test]
    fn clear_ends_early() {
        let mut g = Glitch::new(1.0);
        g.apply(5.0);
        g.clear();
        assert!(!g.is_active());
        assert!(g.just_cleared);
    }

    #[test]
    fn check_malfunction_true_when_rng_below_threshold() {
        let mut g = Glitch::new(2.0);
        g.apply(5.0);
        // error_rate=2.0, dt=1.0 → threshold=2.0; rng=0.5 < 2.0 → true
        assert!(g.check_malfunction(0.5, 1.0));
    }

    #[test]
    fn check_malfunction_false_when_rng_above_threshold() {
        let mut g = Glitch::new(0.1);
        g.apply(5.0);
        // error_rate=0.1, dt=0.016 → threshold=0.0016; rng=0.5 > 0.0016 → false
        assert!(!g.check_malfunction(0.5, 0.016));
    }

    #[test]
    fn check_malfunction_false_when_inactive() {
        let g = Glitch::new(2.0);
        assert!(!g.check_malfunction(0.0, 1.0));
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut g = Glitch::new(1.0);
        g.apply(2.0);
        g.tick(1.0);
        assert!((g.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut g = Glitch::new(1.0);
        g.enabled = false;
        g.apply(5.0);
        assert!(!g.is_active());
    }

    #[test]
    fn tick_clears_just_glitched() {
        let mut g = Glitch::new(1.0);
        g.apply(3.0);
        g.tick(0.016);
        assert!(!g.just_glitched);
    }
}
