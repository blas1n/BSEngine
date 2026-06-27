use bevy_ecs::prelude::Component;

/// The kind of stat penalty a curse applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CurseKind {
    /// Reduces outgoing damage dealt (multiplier < 1.0).
    DamageDown,
    /// Reduces movement speed (multiplier < 1.0).
    SpeedDown,
    /// Reduces armor / damage reduction (multiplier < 1.0).
    ArmorDown,
    /// Increases damage taken (multiplier > 1.0).
    DamageTakenUp,
    /// Caller-defined penalty; `strength` carries the meaning.
    Custom,
}

/// Curse debuff — timed stat penalty applied by an external source.
///
/// Multiple curses can coexist if needed by attaching additional `Curse`
/// components via Bevy's component system; this single component handles
/// one active curse at a time per entity. A longer-lasting curse replaces
/// a shorter one on re-apply.
///
/// The relevant system reads `kind` and `strength` to scale the stat each
/// frame while `is_active()` returns true. `tick(dt)` expires the curse.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Curse {
    pub kind: CurseKind,
    /// Effect strength. Meaning depends on `kind`:
    /// - DamageDown / SpeedDown / ArmorDown: multiplier applied to the stat
    ///   (e.g. 0.7 = 30% reduction). Clamped to [0.0, 1.0] on apply.
    /// - DamageTakenUp: multiplier on incoming damage (e.g. 1.5 = 50% more).
    /// - Custom: caller-defined.
    pub strength: f32,
    pub duration: f32,
    pub timer: f32,
    /// True on the first frame the curse is applied (including re-apply).
    pub just_cursed: bool,
    /// True on the first frame the curse expires.
    pub just_lifted: bool,
    pub enabled: bool,
}

impl Curse {
    pub fn new(kind: CurseKind, strength: f32, duration: f32) -> Self {
        Self {
            kind,
            strength,
            duration: duration.max(0.0),
            timer: 0.0,
            just_cursed: false,
            just_lifted: false,
            enabled: true,
        }
    }

    /// Apply a curse. If the new duration is longer than time remaining,
    /// it replaces the current curse (kind and strength updated too).
    pub fn apply(&mut self, kind: CurseKind, strength: f32, duration: f32) {
        if !self.enabled {
            return;
        }
        let remaining = self.duration - self.timer;
        if duration > remaining || !self.is_active() {
            self.kind = kind;
            self.strength = strength;
            self.duration = duration.max(0.0);
            self.timer = 0.0;
        }
        self.just_cursed = true;
    }

    /// Remove the curse immediately (cleanse).
    pub fn clear(&mut self) {
        self.timer = self.duration;
    }

    /// Advance the timer. Transitions to lifted when duration is reached.
    pub fn tick(&mut self, dt: f32) {
        self.just_cursed = false;
        self.just_lifted = false;

        if !self.is_active() {
            return;
        }

        self.timer += dt;
        if self.timer >= self.duration {
            self.timer = self.duration;
            self.just_lifted = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer < self.duration
    }

    /// Fraction of the curse duration elapsed [0.0, 1.0].
    pub fn elapsed_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }

    /// Remaining duration in seconds.
    pub fn remaining(&self) -> f32 {
        (self.duration - self.timer).max(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_starts_curse() {
        let mut c = Curse::new(CurseKind::DamageDown, 0.7, 0.0);
        c.apply(CurseKind::DamageDown, 0.7, 3.0);
        assert!(c.is_active());
        assert!(c.just_cursed);
    }

    #[test]
    fn longer_apply_replaces_shorter() {
        let mut c = Curse::new(CurseKind::DamageDown, 0.7, 0.0);
        c.apply(CurseKind::DamageDown, 0.7, 2.0);
        c.apply(CurseKind::SpeedDown, 0.5, 5.0);
        assert_eq!(c.kind, CurseKind::SpeedDown);
        assert_eq!(c.duration, 5.0);
    }

    #[test]
    fn shorter_apply_does_not_replace() {
        let mut c = Curse::new(CurseKind::DamageDown, 0.7, 0.0);
        c.apply(CurseKind::DamageDown, 0.7, 5.0);
        c.apply(CurseKind::SpeedDown, 0.5, 1.0);
        assert_eq!(c.kind, CurseKind::DamageDown);
        assert_eq!(c.duration, 5.0);
    }

    #[test]
    fn tick_expires_curse() {
        let mut c = Curse::new(CurseKind::DamageDown, 0.7, 0.0);
        c.apply(CurseKind::DamageDown, 0.7, 2.0);
        c.tick(2.1);
        assert!(!c.is_active());
        assert!(c.just_lifted);
    }

    #[test]
    fn clear_deactivates_immediately() {
        let mut c = Curse::new(CurseKind::DamageDown, 0.7, 0.0);
        c.apply(CurseKind::DamageDown, 0.7, 5.0);
        c.clear();
        assert!(!c.is_active());
    }

    #[test]
    fn disabled_ignores_apply() {
        let mut c = Curse::new(CurseKind::DamageDown, 0.7, 0.0);
        c.enabled = false;
        c.apply(CurseKind::DamageDown, 0.7, 3.0);
        assert!(!c.is_active());
        assert!(!c.just_cursed);
    }

    #[test]
    fn elapsed_fraction_correct() {
        let mut c = Curse::new(CurseKind::ArmorDown, 0.6, 0.0);
        c.apply(CurseKind::ArmorDown, 0.6, 4.0);
        c.tick(1.0);
        let frac = c.elapsed_fraction();
        assert!((frac - 0.25).abs() < 1e-5);
    }

    #[test]
    fn remaining_decreases_with_tick() {
        let mut c = Curse::new(CurseKind::SpeedDown, 0.8, 0.0);
        c.apply(CurseKind::SpeedDown, 0.8, 4.0);
        c.tick(1.0);
        assert!((c.remaining() - 3.0).abs() < 1e-5);
    }
}
