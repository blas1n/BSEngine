use bevy_ecs::prelude::Component;

/// Temporary bonus HP pool that absorbs damage before the entity's main health.
///
/// When an entity receives healing beyond its health cap, the surplus is stored
/// in this overflow pool up to `max_pool`. The pool then decays at `decay_rate`
/// units per second (0.0 = no decay) and absorbs incoming damage first.
///
/// Typical uses: overheal from medics/items (Overwatch armour packs, WoW
/// Overheal), temporary barriers granted by abilities.
///
/// Usage pattern:
/// ```ignore
/// // On heal: store excess
/// let excess = healing - (health.max - health.current);
/// if excess > 0.0 { overflow.add(excess); }
/// // On damage: subtract from overflow first
/// let remaining = overflow.consume(damage);
/// health.apply_damage(remaining);
/// ```
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Overflow {
    pub current: f32,
    pub max_pool: f32,
    /// Units drained per second. 0.0 = no natural decay.
    pub decay_rate: f32,
    pub just_gained: bool,
    pub just_depleted: bool,
    pub enabled: bool,
}

impl Overflow {
    pub fn new(max_pool: f32) -> Self {
        Self {
            current: 0.0,
            max_pool: max_pool.max(0.0),
            decay_rate: 0.0,
            just_gained: false,
            just_depleted: false,
            enabled: true,
        }
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.decay_rate = rate.max(0.0);
        self
    }

    /// Fill the pool by `amount`. Returns how much was actually added.
    /// No-op when disabled or pool is already full.
    pub fn add(&mut self, amount: f32) -> f32 {
        if !self.enabled || amount <= 0.0 {
            return 0.0;
        }

        let space = (self.max_pool - self.current).max(0.0);
        let added = amount.min(space);
        if added > 0.0 {
            self.current += added;
            self.just_gained = true;
        }

        added
    }

    /// Drain up to `amount` from the pool. Returns the portion NOT covered
    /// (i.e. damage that should continue to the main health pool).
    pub fn consume(&mut self, amount: f32) -> f32 {
        if !self.enabled || amount <= 0.0 || self.current <= 0.0 {
            return amount;
        }

        let drained = amount.min(self.current);
        self.current -= drained;
        if self.current <= 0.0 {
            self.current = 0.0;
            self.just_depleted = true;
        }

        amount - drained
    }

    /// Advance natural decay by `dt` seconds; sets `just_depleted` when
    /// the pool empties.
    pub fn tick(&mut self, dt: f32) {
        self.just_gained = false;
        self.just_depleted = false;

        if self.current > 0.0 && self.decay_rate > 0.0 {
            self.current -= self.decay_rate * dt;
            if self.current <= 0.0 {
                self.current = 0.0;
                self.just_depleted = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.current > 0.0
    }

    /// Current overflow as a fraction of max [0.0, 1.0].
    pub fn fraction(&self) -> f32 {
        if self.max_pool <= 0.0 {
            return 0.0;
        }
        (self.current / self.max_pool).clamp(0.0, 1.0)
    }
}

impl Default for Overflow {
    fn default() -> Self {
        Self::new(50.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_fills_pool() {
        let mut o = Overflow::new(100.0);
        let added = o.add(50.0);
        assert!((added - 50.0).abs() < 1e-5);
        assert!((o.current - 50.0).abs() < 1e-5);
        assert!(o.just_gained);
    }

    #[test]
    fn add_caps_at_max() {
        let mut o = Overflow::new(100.0);
        let added = o.add(150.0);
        assert!((added - 100.0).abs() < 1e-5);
        assert!((o.current - 100.0).abs() < 1e-5);
    }

    #[test]
    fn consume_absorbs_damage() {
        let mut o = Overflow::new(100.0);
        o.add(60.0);
        let remaining = o.consume(40.0);
        assert!((remaining).abs() < 1e-5); // fully absorbed
        assert!((o.current - 20.0).abs() < 1e-5);
    }

    #[test]
    fn consume_partial_pool() {
        let mut o = Overflow::new(100.0);
        o.add(30.0);
        let remaining = o.consume(50.0);
        // 30 absorbed, 20 remaining damage
        assert!((remaining - 20.0).abs() < 1e-5);
        assert!((o.current).abs() < 1e-5);
        assert!(o.just_depleted);
    }

    #[test]
    fn consume_empty_pool_passthrough() {
        let mut o = Overflow::new(100.0);
        let remaining = o.consume(50.0);
        assert!((remaining - 50.0).abs() < 1e-5);
    }

    #[test]
    fn tick_decays_pool() {
        let mut o = Overflow::new(100.0).with_decay_rate(10.0);
        o.add(50.0);
        o.tick(2.0);
        // 50 - 10*2 = 30
        assert!((o.current - 30.0).abs() < 1e-5);
    }

    #[test]
    fn tick_depletes_fully() {
        let mut o = Overflow::new(100.0).with_decay_rate(20.0);
        o.add(10.0);
        o.tick(1.0);
        assert!((o.current).abs() < 1e-5);
        assert!(o.just_depleted);
    }

    #[test]
    fn fraction_at_half() {
        let mut o = Overflow::new(100.0);
        o.add(50.0);
        assert!((o.fraction() - 0.5).abs() < 1e-5);
    }

    #[test]
    fn disabled_add_no_op() {
        let mut o = Overflow::new(100.0);
        o.enabled = false;
        let added = o.add(50.0);
        assert!((added).abs() < 1e-5);
        assert!(!o.is_active());
    }

    #[test]
    fn disabled_consume_passthrough() {
        let mut o = Overflow::new(100.0);
        o.add(50.0);
        o.enabled = false;
        let remaining = o.consume(30.0);
        // disabled → all passes through
        assert!((remaining - 30.0).abs() < 1e-5);
    }
}
