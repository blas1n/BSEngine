use bevy_ecs::prelude::Component;

/// Energy barrier that absorbs incoming damage before it reaches `Health`.
///
/// The combat system should call `absorb(damage)` when the entity takes a hit.
/// Whatever is not absorbed returns as remaining damage to apply to `Health`.
/// Barrier regenerates at `regen_rate` per second after `regen_delay` seconds
/// without damage; the health system calls `tick(dt)` each frame to advance.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Barrier {
    /// Maximum barrier capacity.
    pub capacity: f32,
    /// Current barrier amount. Absorbed damage reduces this.
    pub current: f32,
    /// Regeneration rate in units per second (once delay has elapsed).
    pub regen_rate: f32,
    /// Seconds of no-damage before regen begins.
    pub regen_delay: f32,
    /// Counts up while no damage is taken; regen starts when ≥ `regen_delay`.
    pub regen_timer: f32,
    /// True on the first frame `current` drops to 0.
    pub just_broken: bool,
    /// True on the first frame `current` returns to `capacity`.
    pub just_restored: bool,
    pub enabled: bool,
}

impl Barrier {
    pub fn new(capacity: f32) -> Self {
        Self {
            capacity: capacity.max(0.0),
            current: capacity.max(0.0),
            regen_rate: 0.0,
            regen_delay: 3.0,
            regen_timer: 0.0,
            just_broken: false,
            just_restored: false,
            enabled: true,
        }
    }

    pub fn with_regen(mut self, rate: f32, delay: f32) -> Self {
        self.regen_rate = rate.max(0.0);
        self.regen_delay = delay.max(0.0);
        self
    }

    /// Absorb up to `damage` points of incoming damage.
    ///
    /// Returns the unabsorbed remainder that should be applied to `Health`.
    /// Does nothing and returns `damage` unchanged if `enabled` is false.
    pub fn absorb(&mut self, damage: f32) -> f32 {
        if !self.enabled || self.current <= 0.0 {
            return damage;
        }

        let was_full = (self.current - self.capacity).abs() < f32::EPSILON;
        let _ = was_full; // suppresses lint; used implicitly via regen reset

        self.regen_timer = 0.0;

        if damage <= self.current {
            self.current -= damage;
            if self.current <= 0.0 {
                self.current = 0.0;
                self.just_broken = true;
            }
            0.0
        } else {
            let remainder = damage - self.current;
            self.current = 0.0;
            self.just_broken = true;
            remainder
        }
    }

    /// Advance regeneration timers by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.just_broken = false;
        self.just_restored = false;

        if !self.enabled || self.current >= self.capacity || self.regen_rate <= 0.0 {
            return;
        }

        self.regen_timer += dt;
        if self.regen_timer >= self.regen_delay {
            let was_depleted = self.current <= 0.0;
            self.current = (self.current + self.regen_rate * dt).min(self.capacity);
            if self.current >= self.capacity && !was_depleted {
                self.just_restored = true;
            } else if self.current >= self.capacity {
                self.just_restored = true;
            }
        }
    }

    /// Instantly set the barrier to full.
    pub fn restore(&mut self) {
        self.current = self.capacity;
        self.regen_timer = 0.0;
    }

    /// Instantly drain the barrier to zero.
    pub fn drain(&mut self) {
        if self.current > 0.0 {
            self.current = 0.0;
            self.just_broken = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.current > 0.0
    }

    pub fn is_full(&self) -> bool {
        self.current >= self.capacity
    }

    /// Fraction of capacity remaining [0.0, 1.0].
    pub fn fraction(&self) -> f32 {
        if self.capacity <= 0.0 {
            return 0.0;
        }
        (self.current / self.capacity).clamp(0.0, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absorb_reduces_current() {
        let mut b = Barrier::new(100.0);
        let remainder = b.absorb(30.0);
        assert_eq!(remainder, 0.0);
        assert!((b.current - 70.0).abs() < 1e-5);
    }

    #[test]
    fn absorb_overflow_returns_remainder() {
        let mut b = Barrier::new(50.0);
        let remainder = b.absorb(80.0);
        assert!((remainder - 30.0).abs() < 1e-5);
        assert_eq!(b.current, 0.0);
        assert!(b.just_broken);
    }

    #[test]
    fn absorb_exact_drains_to_zero() {
        let mut b = Barrier::new(50.0);
        let remainder = b.absorb(50.0);
        assert_eq!(remainder, 0.0);
        assert_eq!(b.current, 0.0);
        assert!(b.just_broken);
    }

    #[test]
    fn disabled_passes_through_damage() {
        let mut b = Barrier::new(100.0);
        b.enabled = false;
        let remainder = b.absorb(50.0);
        assert!((remainder - 50.0).abs() < 1e-5);
        assert!((b.current - 100.0).abs() < 1e-5);
    }

    #[test]
    fn regen_after_delay() {
        let mut b = Barrier::new(100.0).with_regen(20.0, 2.0);
        b.absorb(50.0);
        b.tick(1.0); // below delay → no regen
        assert!((b.current - 50.0).abs() < 1e-4);
        b.tick(1.5); // now past delay → regen 1.5 * 20 = 30
        assert!(b.current > 50.0);
    }

    #[test]
    fn damage_resets_regen_timer() {
        let mut b = Barrier::new(100.0).with_regen(20.0, 2.0);
        b.absorb(10.0);
        b.tick(1.9); // almost enough
        b.absorb(5.0); // damage resets timer
        b.tick(1.0); // only 1.0 since last damage → no regen yet
        assert!((b.current - 85.0).abs() < 1e-4);
    }

    #[test]
    fn restore_fills_to_capacity() {
        let mut b = Barrier::new(100.0);
        b.absorb(70.0);
        b.restore();
        assert!(b.is_full());
    }

    #[test]
    fn drain_empties_barrier() {
        let mut b = Barrier::new(100.0);
        b.drain();
        assert!(!b.is_active());
        assert!(b.just_broken);
    }

    #[test]
    fn fraction_correct() {
        let mut b = Barrier::new(100.0);
        b.absorb(25.0);
        assert!((b.fraction() - 0.75).abs() < 1e-5);
    }
}
