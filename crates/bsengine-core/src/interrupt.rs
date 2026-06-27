use bevy_ecs::prelude::Component;

/// Tracks an entity's susceptibility to action interruption from incoming damage.
///
/// When the entity takes damage ≥ `threshold`, the ability system should call
/// `notify_hit(damage)` which returns `true` if the current action is interrupted.
/// `resistance` [0, 1] reduces the effective damage for the threshold check
/// (higher resistance = harder to interrupt). `just_interrupted` provides a
/// single-frame hook for animation cancels and VFX.
///
/// `reset()` clears the interrupted state after the ability system has handled it
/// (or call `tick(dt)` each frame to auto-clear after one frame).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Interrupt {
    /// Damage amount required to trigger an interrupt.
    pub threshold: f32,
    /// Fraction [0.0, 1.0] that reduces effective damage before comparing to threshold.
    /// 0.0 = no resistance (full damage counts); 1.0 = immune.
    pub resistance: f32,
    /// True on the first frame an interrupt fires.
    pub just_interrupted: bool,
    /// Total interrupts received (for tracking/statistics).
    pub interrupt_count: u32,
    pub enabled: bool,
}

impl Interrupt {
    pub fn new(threshold: f32) -> Self {
        Self {
            threshold: threshold.max(0.0),
            resistance: 0.0,
            just_interrupted: false,
            interrupt_count: 0,
            enabled: true,
        }
    }

    pub fn with_resistance(mut self, resistance: f32) -> Self {
        self.resistance = resistance.clamp(0.0, 1.0);
        self
    }

    /// Check whether `damage` triggers an interrupt.
    ///
    /// Returns `true` if the action should be cancelled; sets `just_interrupted`
    /// and increments `interrupt_count`. Does nothing if `enabled` is false or
    /// `threshold` is 0 (immune to interrupt by zero-threshold convention).
    pub fn notify_hit(&mut self, damage: f32) -> bool {
        if !self.enabled || self.threshold <= 0.0 {
            return false;
        }

        let effective = damage * (1.0 - self.resistance);
        if effective >= self.threshold {
            self.just_interrupted = true;
            self.interrupt_count += 1;
            return true;
        }

        false
    }

    /// Clear `just_interrupted`; call once per frame (or after handling the interrupt).
    pub fn tick(&mut self, _dt: f32) {
        self.just_interrupted = false;
    }

    /// Force an interrupt regardless of damage (stun, CC break, etc.).
    pub fn force_interrupt(&mut self) {
        if self.enabled {
            self.just_interrupted = true;
            self.interrupt_count += 1;
        }
    }

    /// Reset the interrupt state without advancing the frame.
    pub fn reset(&mut self) {
        self.just_interrupted = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notify_hit_above_threshold() {
        let mut i = Interrupt::new(10.0);
        let interrupted = i.notify_hit(15.0);
        assert!(interrupted);
        assert!(i.just_interrupted);
        assert_eq!(i.interrupt_count, 1);
    }

    #[test]
    fn notify_hit_below_threshold() {
        let mut i = Interrupt::new(10.0);
        let interrupted = i.notify_hit(5.0);
        assert!(!interrupted);
        assert!(!i.just_interrupted);
    }

    #[test]
    fn notify_hit_at_exact_threshold() {
        let mut i = Interrupt::new(10.0);
        let interrupted = i.notify_hit(10.0);
        assert!(interrupted);
    }

    #[test]
    fn resistance_reduces_effective_damage() {
        let mut i = Interrupt::new(10.0).with_resistance(0.5);
        // effective = 15.0 * 0.5 = 7.5 < 10.0 → no interrupt
        let interrupted = i.notify_hit(15.0);
        assert!(!interrupted);
        // effective = 25.0 * 0.5 = 12.5 >= 10.0 → interrupt
        let interrupted2 = i.notify_hit(25.0);
        assert!(interrupted2);
    }

    #[test]
    fn full_resistance_immune() {
        let mut i = Interrupt::new(10.0).with_resistance(1.0);
        let interrupted = i.notify_hit(1000.0);
        assert!(!interrupted);
    }

    #[test]
    fn tick_clears_just_interrupted() {
        let mut i = Interrupt::new(10.0);
        i.notify_hit(15.0);
        i.tick(0.016);
        assert!(!i.just_interrupted);
    }

    #[test]
    fn force_interrupt_bypasses_threshold() {
        let mut i = Interrupt::new(100.0);
        i.force_interrupt();
        assert!(i.just_interrupted);
        assert_eq!(i.interrupt_count, 1);
    }

    #[test]
    fn disabled_ignores_notify_hit() {
        let mut i = Interrupt::new(5.0);
        i.enabled = false;
        let interrupted = i.notify_hit(100.0);
        assert!(!interrupted);
        assert!(!i.just_interrupted);
    }
}
