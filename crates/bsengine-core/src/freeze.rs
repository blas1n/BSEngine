use bevy_ecs::prelude::Component;

/// Cold / ice buildup severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FreezeState {
    /// No cold buildup; entity operates at full speed.
    Normal,
    /// Partial buildup; movement slowed by `chill_slow`.
    Chilled,
    /// Maximum buildup; entity is fully immobile until `frozen_timer` expires.
    Frozen,
}

/// Ice/cold status-effect component — symmetric counterpart to `Burn`.
///
/// The cold-damage system calls `apply_cold(amount)` on each cold hit.
/// `cold_buildup` accumulates until it crosses `chill_threshold` (→ Chilled)
/// or `freeze_threshold` (→ Frozen). Between hits, `cold_buildup` decays at
/// `cold_decay_rate`/s via `tick(dt)`. While Frozen, the entity cannot move;
/// `tick()` counts down `frozen_timer` before returning to Normal.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Freeze {
    pub state: FreezeState,
    /// Current cold buildup level.
    pub cold_buildup: f32,
    /// Buildup at which Chilled activates.
    pub chill_threshold: f32,
    /// Buildup at which Frozen activates.
    pub freeze_threshold: f32,
    /// Decay rate of `cold_buildup` per second while not Frozen.
    pub cold_decay_rate: f32,
    /// Speed multiplier while Chilled (0.0–1.0). Frozen always implies 0.
    pub chill_slow: f32,
    /// How long (seconds) the Frozen state lasts before returning to Normal.
    pub frozen_duration: f32,
    /// Time remaining in the Frozen state.
    pub frozen_timer: f32,
    /// True on the frame the entity transitions to Chilled or Frozen.
    pub just_frozen: bool,
    /// True on the frame the entity thaws back to Normal.
    pub just_thawed: bool,
    /// If true, cold has no effect on this entity.
    pub immune: bool,
    pub enabled: bool,
}

impl Freeze {
    pub fn new(chill_threshold: f32, freeze_threshold: f32, frozen_duration: f32) -> Self {
        Self {
            state: FreezeState::Normal,
            cold_buildup: 0.0,
            chill_threshold: chill_threshold.max(0.0),
            freeze_threshold: freeze_threshold.max(0.0),
            cold_decay_rate: 5.0,
            chill_slow: 0.5,
            frozen_duration: frozen_duration.max(0.0),
            frozen_timer: 0.0,
            just_frozen: false,
            just_thawed: false,
            immune: false,
            enabled: true,
        }
    }

    pub fn with_chill_slow(mut self, factor: f32) -> Self {
        self.chill_slow = factor.clamp(0.0, 1.0);
        self
    }

    pub fn with_decay_rate(mut self, rate: f32) -> Self {
        self.cold_decay_rate = rate.max(0.0);
        self
    }

    pub fn immune(mut self) -> Self {
        self.immune = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply `amount` of cold to the entity. Returns false if immune or disabled.
    pub fn apply_cold(&mut self, amount: f32) -> bool {
        if !self.enabled || self.immune || self.state == FreezeState::Frozen {
            return false;
        }
        self.cold_buildup += amount.max(0.0);
        self.update_state();
        true
    }

    /// Immediately thaw the entity to Normal.
    pub fn thaw(&mut self) {
        self.cold_buildup = 0.0;
        self.frozen_timer = 0.0;
        if self.state != FreezeState::Normal {
            self.just_thawed = true;
        }
        self.state = FreezeState::Normal;
    }

    /// Advance state. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_frozen = false;
        self.just_thawed = false;

        if !self.enabled {
            return;
        }

        match self.state {
            FreezeState::Frozen => {
                self.frozen_timer -= dt;
                if self.frozen_timer <= 0.0 {
                    self.state = FreezeState::Normal;
                    self.cold_buildup = 0.0;
                    self.just_thawed = true;
                }
            }
            FreezeState::Chilled | FreezeState::Normal => {
                self.cold_buildup = (self.cold_buildup - self.cold_decay_rate * dt).max(0.0);
                self.update_state();
            }
        }
    }

    fn update_state(&mut self) {
        let new_state = if self.cold_buildup >= self.freeze_threshold {
            FreezeState::Frozen
        } else if self.cold_buildup >= self.chill_threshold {
            FreezeState::Chilled
        } else {
            FreezeState::Normal
        };

        if new_state == FreezeState::Frozen && self.state != FreezeState::Frozen {
            self.frozen_timer = self.frozen_duration;
            self.just_frozen = true;
        }
        self.state = new_state;
    }

    /// Speed multiplier to apply to movement this frame.
    pub fn speed_multiplier(&self) -> f32 {
        match self.state {
            FreezeState::Normal => 1.0,
            FreezeState::Chilled => self.chill_slow,
            FreezeState::Frozen => 0.0,
        }
    }

    pub fn is_frozen(&self) -> bool {
        self.state == FreezeState::Frozen
    }

    pub fn is_chilled(&self) -> bool {
        self.state == FreezeState::Chilled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_cold_transitions_to_chilled() {
        let mut f = Freeze::new(10.0, 30.0, 2.0);
        f.apply_cold(15.0);
        assert_eq!(f.state, FreezeState::Chilled);
    }

    #[test]
    fn apply_cold_transitions_to_frozen() {
        let mut f = Freeze::new(10.0, 30.0, 2.0);
        f.apply_cold(35.0);
        assert!(f.is_frozen());
        assert!(f.just_frozen);
    }

    #[test]
    fn speed_multiplier_correct_per_state() {
        let mut f = Freeze::new(10.0, 30.0, 2.0).with_chill_slow(0.4);
        assert_eq!(f.speed_multiplier(), 1.0);
        f.apply_cold(15.0);
        assert_eq!(f.speed_multiplier(), 0.4);
        f.apply_cold(20.0);
        assert_eq!(f.speed_multiplier(), 0.0);
    }

    #[test]
    fn tick_decays_cold_buildup() {
        let mut f = Freeze::new(10.0, 30.0, 2.0).with_decay_rate(20.0);
        f.apply_cold(15.0);
        f.tick(1.0); // 15 - 20 → 0
        assert_eq!(f.state, FreezeState::Normal);
    }

    #[test]
    fn frozen_timer_expires_to_normal() {
        let mut f = Freeze::new(10.0, 30.0, 1.0);
        f.apply_cold(50.0);
        f.tick(1.5);
        assert_eq!(f.state, FreezeState::Normal);
        assert!(f.just_thawed);
    }

    #[test]
    fn immune_rejects_cold() {
        let mut f = Freeze::new(10.0, 30.0, 2.0).immune();
        let ok = f.apply_cold(50.0);
        assert!(!ok);
        assert_eq!(f.state, FreezeState::Normal);
    }
}
