use bevy_ecs::prelude::Component;

/// Buff that amplifies the entity's outgoing ability and skill potency.
///
/// While empowered, ability systems multiply their base power by
/// `effective_potency(base)` — e.g. a multiplier of 1.5 means abilities deal
/// 50% more damage or have 50% stronger secondary effects (heal, shield, etc.).
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_faded` when the effect ends. `clear()` removes the buff early.
///
/// Distinct from `Amplify` (specific damage-type amplification on a target)
/// and `Galvanize` (shield/bulk stat boost): Empower raises the caster's own
/// outgoing skill output — ability damage, heal strength, and buff magnitude.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Empower {
    pub duration: f32,
    pub timer: f32,
    /// Multiplier applied to outgoing ability potency while empowered.
    /// Values > 1.0 increase potency; 1.0 = no change; 0.0 = zero output.
    pub potency_multiplier: f32,
    pub just_empowered: bool,
    pub just_faded: bool,
    pub enabled: bool,
}

impl Empower {
    pub fn new(potency_multiplier: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            potency_multiplier: potency_multiplier.max(0.0),
            just_empowered: false,
            just_faded: false,
            enabled: true,
        }
    }

    /// Apply or extend the empower for `duration` seconds. High-watermark:
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
                self.just_empowered = true;
            }
        }
    }

    /// Remove the empower immediately (dispel, silence, etc.).
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_faded = true;
        }
    }

    /// Advance the timer; sets `just_faded` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_empowered = false;
        self.just_faded = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_faded = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Apply the empower multiplier to a base potency value.
    /// Returns `base * potency_multiplier` while active, `base` otherwise.
    pub fn effective_potency(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.potency_multiplier
        } else {
            base
        }
    }

    /// Fraction of the empower duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Empower {
    fn default() -> Self {
        Self::new(1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_empower() {
        let mut e = Empower::new(1.5);
        e.apply(3.0);
        assert!(e.is_active());
        assert!(e.just_empowered);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut e = Empower::new(1.5);
        e.apply(2.0);
        e.tick(0.016);
        e.apply(5.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut e = Empower::new(1.5);
        e.apply(5.0);
        e.apply(2.0);
        assert!((e.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_empower() {
        let mut e = Empower::new(1.5);
        e.apply(1.0);
        e.tick(1.1);
        assert!(!e.is_active());
        assert!(e.just_faded);
    }

    #[test]
    fn clear_ends_early() {
        let mut e = Empower::new(1.5);
        e.apply(5.0);
        e.clear();
        assert!(!e.is_active());
        assert!(e.just_faded);
    }

    #[test]
    fn effective_potency_while_active() {
        let mut e = Empower::new(2.0);
        e.apply(3.0);
        assert!((e.effective_potency(50.0) - 100.0).abs() < 1e-4); // 50 * 2
    }

    #[test]
    fn effective_potency_when_inactive() {
        let e = Empower::new(2.0);
        assert!((e.effective_potency(50.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut e = Empower::new(1.5);
        e.apply(2.0);
        e.tick(1.0);
        assert!((e.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut e = Empower::new(1.5);
        e.enabled = false;
        e.apply(5.0);
        assert!(!e.is_active());
    }

    #[test]
    fn tick_clears_just_empowered() {
        let mut e = Empower::new(1.5);
        e.apply(3.0);
        e.tick(0.016);
        assert!(!e.just_empowered);
    }
}
