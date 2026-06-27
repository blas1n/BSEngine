use bevy_ecs::prelude::Component;

/// Anti-magic field that suppresses incoming magical effects for a duration.
///
/// While active, the entity rejects incoming effects according to the
/// `blocks_buffs` / `blocks_debuffs` flags. Unlike `Immune` (which blocks
/// specific damage-type bits), `Nullify` operates at the effect-category
/// level — the ability system checks these flags before applying any buff,
/// debuff, or heal and discards effects that match.
///
/// Examples: spell immunity bubbles, anti-magic shells, black-hole zones.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_expired` when the null field drops.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Nullify {
    pub duration: f32,
    pub timer: f32,
    /// When true, incoming beneficial effects (heals, buffs) are blocked.
    pub blocks_buffs: bool,
    /// When true, incoming detrimental effects (debuffs, CC) are blocked.
    pub blocks_debuffs: bool,
    pub just_activated: bool,
    pub just_expired: bool,
    pub enabled: bool,
}

impl Nullify {
    /// Create a nullify field that blocks both buffs and debuffs.
    pub fn new() -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            blocks_buffs: true,
            blocks_debuffs: true,
            just_activated: false,
            just_expired: false,
            enabled: true,
        }
    }

    /// Block only harmful incoming effects.
    pub fn debuffs_only() -> Self {
        Self {
            blocks_buffs: false,
            ..Self::new()
        }
    }

    /// Block only beneficial incoming effects (e.g. prevent self-healing).
    pub fn buffs_only() -> Self {
        Self {
            blocks_debuffs: false,
            ..Self::new()
        }
    }

    /// Apply or extend a nullify of `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_activated = true;
            }
        }
    }

    /// Remove the nullify field immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_expired = true;
        }
    }

    /// Advance the timer; sets `just_expired` when the effect ends.
    pub fn tick(&mut self, dt: f32) {
        self.just_activated = false;
        self.just_expired = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_expired = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Returns `true` when an incoming effect should be blocked.
    /// `is_buff` = true for heals/buffs, false for debuffs/CC.
    pub fn blocks(&self, is_buff: bool) -> bool {
        if !self.enabled || !self.is_active() {
            return false;
        }
        if is_buff {
            self.blocks_buffs
        } else {
            self.blocks_debuffs
        }
    }

    /// Fraction of the nullify duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Nullify {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_nullify() {
        let mut n = Nullify::new();
        n.apply(3.0);
        assert!(n.is_active());
        assert!(n.just_activated);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut n = Nullify::new();
        n.apply(2.0);
        n.tick(0.016);
        n.apply(5.0);
        assert!((n.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut n = Nullify::new();
        n.apply(5.0);
        n.apply(2.0);
        assert!((n.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_nullify() {
        let mut n = Nullify::new();
        n.apply(1.0);
        n.tick(1.1);
        assert!(!n.is_active());
        assert!(n.just_expired);
    }

    #[test]
    fn clear_cancels_immediately() {
        let mut n = Nullify::new();
        n.apply(5.0);
        n.clear();
        assert!(!n.is_active());
        assert!(n.just_expired);
    }

    #[test]
    fn blocks_buffs_when_active() {
        let mut n = Nullify::new();
        n.apply(3.0);
        assert!(n.blocks(true)); // buff
        assert!(n.blocks(false)); // debuff
    }

    #[test]
    fn does_not_block_when_inactive() {
        let n = Nullify::new(); // timer = 0
        assert!(!n.blocks(true));
        assert!(!n.blocks(false));
    }

    #[test]
    fn debuffs_only_ignores_buffs() {
        let mut n = Nullify::debuffs_only();
        n.apply(3.0);
        assert!(!n.blocks(true)); // buff passes through
        assert!(n.blocks(false)); // debuff blocked
    }

    #[test]
    fn buffs_only_ignores_debuffs() {
        let mut n = Nullify::buffs_only();
        n.apply(3.0);
        assert!(n.blocks(true)); // buff blocked
        assert!(!n.blocks(false)); // debuff passes through
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut n = Nullify::new();
        n.apply(2.0);
        n.tick(1.0);
        assert!((n.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_blocks_nothing() {
        let mut n = Nullify::new();
        n.apply(5.0);
        n.enabled = false;
        assert!(!n.blocks(true));
        assert!(!n.blocks(false));
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut n = Nullify::new();
        n.enabled = false;
        n.apply(5.0);
        assert!(!n.is_active());
    }
}
