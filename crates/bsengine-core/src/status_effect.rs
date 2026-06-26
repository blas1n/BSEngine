use bevy_ecs::prelude::Component;

/// Category of modifier a status effect applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectKind {
    /// Multiplies the target stat by `value` (e.g. 1.5 = +50% speed).
    StatMultiplier,
    /// Adds `value` to the target stat each second (poison, regen).
    DamageOverTime,
    /// Prevents the entity from moving when `value` > 0.
    Immobilize,
    /// Prevents the entity from using abilities when `value` > 0.
    Silence,
    /// Custom game-defined effect type.
    Custom(u32),
}

/// A single timed status effect on an entity (buff, debuff, DoT, CC).
/// Multiple effects can coexist — attach one `StatusEffect` per active condition.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct StatusEffect {
    /// Human-readable identifier (e.g. `"poison"`, `"haste"`).
    pub id: String,
    pub kind: EffectKind,
    /// Numeric strength of the effect. Interpretation depends on `kind`.
    pub value: f32,
    /// Remaining duration in seconds. `f32::INFINITY` = permanent until removed.
    pub duration: f32,
    /// Whether this effect ticks on physics frames (true) or only on status ticks (false).
    pub ticks_every_frame: bool,
    pub enabled: bool,
}

impl StatusEffect {
    pub fn new(id: impl Into<String>, kind: EffectKind, value: f32, duration: f32) -> Self {
        Self {
            id: id.into(),
            kind,
            value,
            duration: duration.max(0.0),
            ticks_every_frame: false,
            enabled: true,
        }
    }

    pub fn poison(damage_per_second: f32, duration: f32) -> Self {
        Self::new(
            "poison",
            EffectKind::DamageOverTime,
            damage_per_second,
            duration,
        )
        .every_frame()
    }

    pub fn haste(speed_multiplier: f32, duration: f32) -> Self {
        Self::new(
            "haste",
            EffectKind::StatMultiplier,
            speed_multiplier,
            duration,
        )
    }

    pub fn every_frame(mut self) -> Self {
        self.ticks_every_frame = true;
        self
    }

    pub fn permanent(mut self) -> Self {
        self.duration = f32::INFINITY;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` when the effect has run its course.
    pub fn is_expired(&self) -> bool {
        self.duration <= 0.0
    }

    /// Advance remaining duration by `dt`. Returns `true` if the effect just expired.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled || self.duration.is_infinite() {
            return false;
        }
        self.duration = (self.duration - dt).max(0.0);
        self.duration == 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_effect_defaults() {
        let e = StatusEffect::haste(1.5, 5.0);
        assert_eq!(e.id, "haste");
        assert_eq!(e.kind, EffectKind::StatMultiplier);
        assert!((e.value - 1.5).abs() < 0.001);
        assert!(e.enabled);
    }

    #[test]
    fn tick_expires() {
        let mut e = StatusEffect::poison(5.0, 2.0);
        let expired = e.tick(2.0);
        assert!(expired);
        assert!(e.is_expired());
    }

    #[test]
    fn permanent_never_expires() {
        let mut e = StatusEffect::haste(2.0, 1.0).permanent();
        e.tick(1000.0);
        assert!(!e.is_expired());
    }

    #[test]
    fn duration_clamped() {
        let e = StatusEffect::new("slow", EffectKind::StatMultiplier, 0.5, -1.0);
        assert_eq!(e.duration, 0.0);
    }

    #[test]
    fn disabled_does_not_tick() {
        let mut e = StatusEffect::poison(5.0, 2.0).disabled();
        e.tick(2.0);
        assert!(!e.is_expired());
    }
}
