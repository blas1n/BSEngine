use bevy_ecs::prelude::Component;

/// Active phase of a block / parry cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GuardPhase {
    /// Not blocking.
    None,
    /// Raising guard — optional wind-up period before full protection activates.
    Raising,
    /// Guard is fully up; damage is reduced by `damage_reduction`.
    Guarding,
    /// Perfect-parry window: full absorption + counter opportunity.
    Parrying,
    /// Guard stamina depleted; character is staggered / open.
    GuardBroken,
}

/// Active guard / block / parry component for melee combat.
///
/// Attach to characters that can actively block attacks. The combat system
/// queries `phase` and calls `take_hit(damage)` when an attack lands while
/// the guard is up; the component returns how much damage was absorbed.
///
/// Perfect parry is granted during the `parry_window` immediately after
/// `raise()` is called — timing-based parry windows.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Guard {
    pub phase: GuardPhase,
    /// Fraction [0, 1] of incoming damage absorbed while Guarding.
    pub damage_reduction: f32,
    /// Current guard stamina (depleted by blocked hits).
    pub guard_stamina: f32,
    /// Maximum guard stamina before guard breaks.
    pub max_guard_stamina: f32,
    /// Recovery rate of guard stamina per second when not guarding.
    pub stamina_recovery_rate: f32,
    /// Seconds of wind-up before Guarding begins (0 = instant).
    pub raise_duration: f32,
    pub raise_timer: f32,
    /// Seconds at the start of raise_duration in which a parry can trigger.
    pub parry_window: f32,
    /// Remaining time in the broken state before the guard can be raised again.
    pub broken_duration: f32,
    pub broken_timer: f32,
    /// True on the frame a perfect parry occurs.
    pub just_parried: bool,
    /// True on the frame the guard breaks.
    pub just_guard_broken: bool,
    pub enabled: bool,
}

impl Guard {
    pub fn new(max_guard_stamina: f32, damage_reduction: f32) -> Self {
        Self {
            phase: GuardPhase::None,
            damage_reduction: damage_reduction.clamp(0.0, 1.0),
            guard_stamina: max_guard_stamina.max(0.0),
            max_guard_stamina: max_guard_stamina.max(0.0),
            stamina_recovery_rate: 5.0,
            raise_duration: 0.0,
            raise_timer: 0.0,
            parry_window: 0.15,
            broken_duration: 1.5,
            broken_timer: 0.0,
            just_parried: false,
            just_guard_broken: false,
            enabled: true,
        }
    }

    pub fn with_parry_window(mut self, secs: f32) -> Self {
        self.parry_window = secs.max(0.0);
        self
    }

    pub fn with_raise_duration(mut self, secs: f32) -> Self {
        self.raise_duration = secs.max(0.0);
        self
    }

    pub fn with_broken_duration(mut self, secs: f32) -> Self {
        self.broken_duration = secs.max(0.0);
        self
    }

    pub fn with_recovery_rate(mut self, rate: f32) -> Self {
        self.stamina_recovery_rate = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Begin raising the guard. Opens a parry window for `parry_window` seconds.
    pub fn raise(&mut self) {
        if !self.enabled || self.phase == GuardPhase::GuardBroken || self.guard_stamina <= 0.0 {
            return;
        }
        if matches!(self.phase, GuardPhase::None) {
            if self.raise_duration > 0.0 {
                self.phase = GuardPhase::Raising;
                self.raise_timer = self.raise_duration;
            } else {
                self.phase = GuardPhase::Guarding;
            }
        }
    }

    /// Lower the guard voluntarily.
    pub fn lower(&mut self) {
        if matches!(
            self.phase,
            GuardPhase::Raising | GuardPhase::Guarding | GuardPhase::Parrying
        ) {
            self.phase = GuardPhase::None;
        }
    }

    /// Apply an incoming hit. Returns the amount of damage absorbed.
    /// `damage_cost` is the guard-stamina cost of the hit (may differ from damage).
    pub fn take_hit(&mut self, damage: f32, stamina_cost: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        match self.phase {
            GuardPhase::Parrying => {
                // Perfect parry: full absorption, zero stamina cost.
                self.just_parried = true;
                damage
            }
            GuardPhase::Guarding | GuardPhase::Raising => {
                let absorbed = damage * self.damage_reduction;
                self.guard_stamina = (self.guard_stamina - stamina_cost.max(0.0)).max(0.0);
                if self.guard_stamina <= 0.0 {
                    self.phase = GuardPhase::GuardBroken;
                    self.broken_timer = self.broken_duration;
                    self.just_guard_broken = true;
                }
                absorbed
            }
            _ => 0.0,
        }
    }

    /// Advance timers. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_parried = false;
        self.just_guard_broken = false;

        match self.phase {
            GuardPhase::Raising => {
                self.raise_timer = (self.raise_timer - dt).max(0.0);
                // Parry window: the leading portion of the raise animation.
                if self.raise_timer > self.raise_duration - self.parry_window {
                    self.phase = GuardPhase::Parrying;
                } else {
                    self.phase = GuardPhase::Guarding;
                }
                // If raise_duration was already exhausted in a single tick:
                if self.raise_timer <= 0.0 && self.phase == GuardPhase::Raising {
                    self.phase = GuardPhase::Guarding;
                }
            }
            GuardPhase::Parrying => {
                // Parry window expires: transition to Guarding if guard is still held.
                self.raise_timer = (self.raise_timer - dt).max(0.0);
                if self.raise_timer <= (self.raise_duration - self.parry_window).max(0.0) {
                    self.phase = GuardPhase::Guarding;
                }
            }
            GuardPhase::GuardBroken => {
                self.broken_timer = (self.broken_timer - dt).max(0.0);
                if self.broken_timer <= 0.0 {
                    self.phase = GuardPhase::None;
                }
                // Slowly recover stamina while broken.
                self.guard_stamina = (self.guard_stamina + self.stamina_recovery_rate * dt * 0.5)
                    .min(self.max_guard_stamina);
            }
            GuardPhase::None => {
                // Recover stamina while idle.
                if self.guard_stamina < self.max_guard_stamina {
                    self.guard_stamina = (self.guard_stamina + self.stamina_recovery_rate * dt)
                        .min(self.max_guard_stamina);
                }
            }
            GuardPhase::Guarding => {} // No timer advancement needed while blocking.
        }
    }

    pub fn is_blocking(&self) -> bool {
        matches!(
            self.phase,
            GuardPhase::Guarding | GuardPhase::Parrying | GuardPhase::Raising
        )
    }

    pub fn is_in_parry_window(&self) -> bool {
        self.phase == GuardPhase::Parrying
    }

    pub fn stamina_fraction(&self) -> f32 {
        if self.max_guard_stamina > 0.0 {
            self.guard_stamina / self.max_guard_stamina
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raise_instant_guard() {
        let mut g = Guard::new(100.0, 0.8);
        g.raise();
        assert_eq!(g.phase, GuardPhase::Guarding);
    }

    #[test]
    fn take_hit_while_guarding_absorbs_damage() {
        let mut g = Guard::new(100.0, 0.8);
        g.raise();
        let absorbed = g.take_hit(50.0, 10.0);
        assert!((absorbed - 40.0).abs() < 1e-5); // 80% of 50
        assert!((g.guard_stamina - 90.0).abs() < 1e-5);
    }

    #[test]
    fn stamina_depletion_breaks_guard() {
        let mut g = Guard::new(10.0, 1.0);
        g.raise();
        g.take_hit(100.0, 15.0);
        assert_eq!(g.phase, GuardPhase::GuardBroken);
        assert!(g.just_guard_broken);
    }

    #[test]
    fn parry_absorbs_full_damage() {
        let mut g = Guard::new(100.0, 0.5)
            .with_raise_duration(0.3)
            .with_parry_window(0.15);
        g.raise();
        assert_eq!(g.phase, GuardPhase::Raising);
        g.tick(0.01); // still in parry window
        assert_eq!(g.phase, GuardPhase::Parrying);
        let absorbed = g.take_hit(50.0, 10.0);
        assert!((absorbed - 50.0).abs() < 1e-5); // full absorption
        assert!(g.just_parried);
        assert!((g.guard_stamina - 100.0).abs() < 1e-5); // no stamina cost
    }

    #[test]
    fn guard_breaks_then_recovers() {
        let mut g = Guard::new(10.0, 1.0).with_broken_duration(0.5);
        g.raise();
        g.take_hit(100.0, 20.0);
        assert_eq!(g.phase, GuardPhase::GuardBroken);
        g.tick(0.5);
        assert_eq!(g.phase, GuardPhase::None);
    }

    #[test]
    fn lower_guard_cancels_guard() {
        let mut g = Guard::new(100.0, 0.8);
        g.raise();
        g.lower();
        assert_eq!(g.phase, GuardPhase::None);
    }
}
