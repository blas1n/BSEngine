use bevy_ecs::prelude::Component;

/// What attribute the boost amplifies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoostStat {
    /// Multiplies movement speed.
    Speed,
    /// Multiplies damage output.
    Damage,
    /// Multiplies fire rate (divides cooldowns).
    FireRate,
    /// Multiplies defense / damage-reduction factor.
    Defense,
    /// Multiplies jump impulse.
    JumpHeight,
}

/// A timed stat multiplier applied to an entity.
///
/// Multiple boosts may coexist on an entity (use a `Vec<Boost>` component or separate entities).
/// Call `tick(dt)` each frame; the boost expires when `remaining <= 0`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Boost {
    pub stat: BoostStat,
    /// Multiplicative factor applied to the stat. 1.5 = +50%, 0.5 = -50%.
    pub multiplier: f32,
    /// Remaining duration in seconds. Decremented by `tick`.
    pub remaining: f32,
    /// Total original duration (for computing fraction remaining).
    pub duration: f32,
    /// Whether the boost stacks with other boosts of the same stat.
    pub stackable: bool,
    pub enabled: bool,
}

impl Boost {
    pub fn new(stat: BoostStat, multiplier: f32, duration: f32) -> Self {
        Self {
            stat,
            multiplier: multiplier.max(0.0),
            remaining: duration.max(0.0),
            duration: duration.max(0.0),
            stackable: true,
            enabled: true,
        }
    }

    pub fn speed(multiplier: f32, duration: f32) -> Self {
        Self::new(BoostStat::Speed, multiplier, duration)
    }

    pub fn damage(multiplier: f32, duration: f32) -> Self {
        Self::new(BoostStat::Damage, multiplier, duration)
    }

    pub fn non_stackable(mut self) -> Self {
        self.stackable = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Decrement `remaining`. Returns `true` when the boost expires.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || self.remaining <= 0.0 {
            return true;
        }
        self.remaining -= dt;
        self.remaining <= 0.0
    }

    pub fn is_expired(&self) -> bool {
        self.remaining <= 0.0
    }

    /// Fraction of duration remaining [0, 1].
    pub fn fraction_remaining(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.remaining / self.duration).clamp(0.0, 1.0)
    }

    /// Refresh duration without changing the multiplier.
    pub fn refresh(&mut self) {
        self.remaining = self.duration;
    }
}

impl Default for Boost {
    fn default() -> Self {
        Self::new(BoostStat::Speed, 1.5, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boost_expires_after_duration() {
        let mut b = Boost::speed(1.5, 1.0);
        assert!(!b.tick(0.5));
        assert!(b.tick(0.6));
        assert!(b.is_expired());
    }

    #[test]
    fn fraction_remaining_correct() {
        let b = Boost::speed(2.0, 4.0);
        assert!((b.fraction_remaining() - 1.0).abs() < 0.001);
        let mut b2 = Boost::speed(2.0, 4.0);
        b2.remaining = 2.0;
        assert!((b2.fraction_remaining() - 0.5).abs() < 0.001);
    }

    #[test]
    fn refresh_resets_duration() {
        let mut b = Boost::damage(1.5, 3.0);
        b.remaining = 0.5;
        b.refresh();
        assert!((b.remaining - 3.0).abs() < 0.001);
    }

    #[test]
    fn disabled_boost_immediately_expired() {
        let mut b = Boost::speed(2.0, 10.0).disabled();
        assert!(b.tick(0.0));
    }

    #[test]
    fn non_stackable_flag_preserved() {
        let b = Boost::speed(1.2, 5.0).non_stackable();
        assert!(!b.stackable);
    }
}
