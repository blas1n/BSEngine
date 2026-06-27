use bevy_ecs::prelude::Component;

/// What kind of effect the aura emits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuraKind {
    /// Restores health to entities within radius.
    Healing,
    /// Deals damage to entities within radius.
    Damage,
    /// Applies a stat multiplier buff to entities within radius.
    Buff,
    /// Applies a stat multiplier debuff to entities within radius.
    Debuff,
}

/// Pulse-based area-of-effect aura component.
///
/// Each `pulse_interval` seconds `tick()` returns `true` (a "pulse fired"
/// signal). The aura system then queries all entities within `radius` whose
/// faction matches `affects_self / affects_allies / affects_enemies` and
/// applies `effect_value` (healing, damage, or multiplier) to them.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Aura {
    pub kind: AuraKind,
    /// World-space influence radius (metres).
    pub radius: f32,
    /// Value applied per pulse (HP for healing/damage; multiplier for buff/debuff).
    pub effect_value: f32,
    /// How often (seconds) the aura pulses.
    pub pulse_interval: f32,
    /// Time until the next pulse.
    pub pulse_timer: f32,
    /// Whether the aura affects the entity that owns it.
    pub affects_self: bool,
    /// Whether the aura affects allied entities.
    pub affects_allies: bool,
    /// Whether the aura affects enemy entities.
    pub affects_enemies: bool,
    pub enabled: bool,
}

impl Aura {
    pub fn new(kind: AuraKind, radius: f32, effect_value: f32, pulse_interval: f32) -> Self {
        let (affects_self, affects_allies, affects_enemies) = match kind {
            AuraKind::Healing | AuraKind::Buff => (true, true, false),
            AuraKind::Damage | AuraKind::Debuff => (false, false, true),
        };
        Self {
            kind,
            radius: radius.max(0.0),
            effect_value,
            pulse_interval: pulse_interval.max(0.0),
            pulse_timer: pulse_interval.max(0.0),
            affects_self,
            affects_allies,
            affects_enemies,
            enabled: true,
        }
    }

    pub fn with_affects_self(mut self, v: bool) -> Self {
        self.affects_self = v;
        self
    }

    pub fn with_affects_allies(mut self, v: bool) -> Self {
        self.affects_allies = v;
        self
    }

    pub fn with_affects_enemies(mut self, v: bool) -> Self {
        self.affects_enemies = v;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance the pulse timer. Returns `true` when a pulse fires this frame.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || self.pulse_interval <= 0.0 {
            return false;
        }
        self.pulse_timer -= dt;
        if self.pulse_timer <= 0.0 {
            self.pulse_timer += self.pulse_interval;
            return true;
        }
        false
    }

    /// Fraction of the current pulse interval elapsed (0.0–1.0).
    pub fn pulse_fraction(&self) -> f32 {
        if self.pulse_interval > 0.0 {
            let elapsed = self.pulse_interval - self.pulse_timer;
            (elapsed / self.pulse_interval).clamp(0.0, 1.0)
        } else {
            0.0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn healing_aura_fires_after_interval() {
        let mut a = Aura::new(AuraKind::Healing, 5.0, 10.0, 1.0);
        assert!(!a.tick(0.5));
        assert!(a.tick(0.6)); // total 1.1 s
    }

    #[test]
    fn damage_aura_fires_on_interval() {
        let mut a = Aura::new(AuraKind::Damage, 3.0, 5.0, 2.0);
        let fired = a.tick(2.1);
        assert!(fired);
    }

    #[test]
    fn disabled_never_fires() {
        let mut a = Aura::new(AuraKind::Healing, 5.0, 10.0, 0.5).disabled();
        assert!(!a.tick(1.0));
        assert!(!a.tick(1.0));
    }

    #[test]
    fn healing_aura_default_affects_allies_not_enemies() {
        let a = Aura::new(AuraKind::Healing, 5.0, 10.0, 1.0);
        assert!(a.affects_self);
        assert!(a.affects_allies);
        assert!(!a.affects_enemies);
    }

    #[test]
    fn damage_aura_default_affects_enemies_only() {
        let a = Aura::new(AuraKind::Damage, 5.0, 10.0, 1.0);
        assert!(!a.affects_self);
        assert!(!a.affects_allies);
        assert!(a.affects_enemies);
    }

    #[test]
    fn pulse_timer_resets_for_next_interval() {
        let mut a = Aura::new(AuraKind::Buff, 4.0, 1.2, 1.0);
        a.tick(1.1); // fires
        let fired_again = a.tick(1.0); // second interval
        assert!(fired_again);
    }
}
