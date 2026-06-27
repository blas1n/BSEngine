use bevy_ecs::prelude::Component;

/// Life-state machine for DBNO / revival mechanics (Apex-style).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviveState {
    /// Normal — no special revival state active.
    Alive,
    /// Down-but-not-out — entity is incapacitated and bleeding out.
    Downed,
    /// An ally is actively reviving this entity.
    Reviving,
    /// Bleed-out completed with no revival; entity is fully dead.
    Dead,
}

/// Down-but-not-out (DBNO) and revival component.
///
/// Models the classic "second-chance" mechanic:
///
/// 1. `take_down()` — Alive → Downed; bleed-out clock starts.
/// 2. `begin_revive()` — Downed → Reviving; an ally spends `revive_duration`
///    ticking `tick(dt)` to fill `revive_progress`.
/// 3. When `revive_progress >= revive_duration` the state transitions back to
///    Alive, `revives_remaining` decrements, and `just_revived` fires.
/// 4. If the bleed-out timer reaches 0 while Downed or Reviving, the entity
///    enters Dead and `just_died` fires.
/// 5. When `revives_remaining == 0` revival is blocked — `take_down()` goes
///    straight to Dead.
///
/// `cancel_revive()` interrupts an in-progress revival (Reviving → Downed)
/// while preserving `revive_progress` so it can be resumed.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Revive {
    pub state: ReviveState,
    /// Total seconds before a downed entity bleeds out.
    pub down_duration: f32,
    /// Remaining bleed-out time (counts down while Downed or Reviving).
    pub down_timer: f32,
    /// Total seconds required to complete a revival.
    pub revive_duration: f32,
    /// Elapsed revival time (0.0 → revive_duration).
    pub revive_progress: f32,
    /// How many more times this entity can be revived. `u32::MAX` = unlimited.
    pub revives_remaining: u32,
    /// Fires on the first frame the entity is downed.
    pub just_downed: bool,
    /// Fires on the first frame a revival completes.
    pub just_revived: bool,
    /// Fires on the first frame the entity dies.
    pub just_died: bool,
    pub enabled: bool,
}

impl Revive {
    pub fn new(down_duration: f32, revive_duration: f32) -> Self {
        Self {
            state: ReviveState::Alive,
            down_duration: down_duration.max(0.0),
            down_timer: 0.0,
            revive_duration: revive_duration.max(0.0),
            revive_progress: 0.0,
            revives_remaining: u32::MAX,
            just_downed: false,
            just_revived: false,
            just_died: false,
            enabled: true,
        }
    }

    pub fn with_revives(mut self, n: u32) -> Self {
        self.revives_remaining = n;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Transition Alive → Downed (or directly to Dead if no revivals remain).
    pub fn take_down(&mut self) {
        if !self.enabled || self.state != ReviveState::Alive {
            return;
        }
        if self.revives_remaining == 0 {
            self.state = ReviveState::Dead;
            self.just_died = true;
        } else {
            self.state = ReviveState::Downed;
            self.down_timer = self.down_duration;
            self.just_downed = true;
        }
    }

    /// Start a revival: Downed → Reviving. Resets progress to 0.
    pub fn begin_revive(&mut self) {
        if self.state == ReviveState::Downed {
            self.state = ReviveState::Reviving;
            self.revive_progress = 0.0;
        }
    }

    /// Interrupt revival: Reviving → Downed. Progress is preserved.
    pub fn cancel_revive(&mut self) {
        if self.state == ReviveState::Reviving {
            self.state = ReviveState::Downed;
        }
    }

    /// Advance timers by `dt`. Call once per frame.
    ///
    /// While Downed or Reviving the bleed-out clock ticks down.
    /// While Reviving `revive_progress` counts up toward `revive_duration`.
    pub fn tick(&mut self, dt: f32) {
        self.just_downed = false;
        self.just_revived = false;
        self.just_died = false;

        if !self.enabled {
            return;
        }

        match self.state {
            ReviveState::Downed | ReviveState::Reviving => {
                self.down_timer = (self.down_timer - dt).max(0.0);

                if self.state == ReviveState::Reviving {
                    self.revive_progress = (self.revive_progress + dt).min(self.revive_duration);

                    if self.revive_progress >= self.revive_duration {
                        self.state = ReviveState::Alive;
                        self.revive_progress = 0.0;
                        if self.revives_remaining != u32::MAX {
                            self.revives_remaining -= 1;
                        }
                        self.just_revived = true;
                        return;
                    }
                }

                if self.down_timer <= 0.0 {
                    self.state = ReviveState::Dead;
                    self.just_died = true;
                }
            }
            _ => {}
        }
    }

    pub fn is_incapacitated(&self) -> bool {
        matches!(self.state, ReviveState::Downed | ReviveState::Reviving)
    }

    /// 0.0 = fully bled out, 1.0 = just downed.
    pub fn down_fraction(&self) -> f32 {
        if self.down_duration <= 0.0 {
            return 0.0;
        }
        self.down_timer / self.down_duration
    }

    /// 0.0 = no progress, 1.0 = revival complete.
    pub fn revive_fraction(&self) -> f32 {
        if self.revive_duration <= 0.0 {
            return 0.0;
        }
        self.revive_progress / self.revive_duration
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn take_down_starts_downed() {
        let mut r = Revive::new(5.0, 3.0);
        r.take_down();
        assert_eq!(r.state, ReviveState::Downed);
        assert!(r.just_downed);
    }

    #[test]
    fn bleed_out_causes_death() {
        let mut r = Revive::new(1.0, 3.0);
        r.take_down();
        r.tick(0.0);
        r.tick(1.0);
        assert_eq!(r.state, ReviveState::Dead);
        assert!(r.just_died);
    }

    #[test]
    fn begin_revive_fills_progress() {
        let mut r = Revive::new(10.0, 2.0);
        r.take_down();
        r.tick(0.0);
        r.begin_revive();
        r.tick(2.0);
        assert_eq!(r.state, ReviveState::Alive);
        assert!(r.just_revived);
    }

    #[test]
    fn cancel_revive_returns_to_downed() {
        let mut r = Revive::new(10.0, 4.0);
        r.take_down();
        r.tick(0.0);
        r.begin_revive();
        r.tick(1.0); // 1s of 4s progress
        r.cancel_revive();
        assert_eq!(r.state, ReviveState::Downed);
        assert!((r.revive_progress - 1.0).abs() < 1e-5);
    }

    #[test]
    fn no_revives_remaining_goes_straight_to_dead() {
        let mut r = Revive::new(5.0, 3.0).with_revives(0);
        r.take_down();
        assert_eq!(r.state, ReviveState::Dead);
        assert!(r.just_died);
    }

    #[test]
    fn limited_revives_decrements() {
        let mut r = Revive::new(10.0, 1.0).with_revives(2);
        r.take_down();
        r.tick(0.0);
        r.begin_revive();
        r.tick(1.0);
        assert_eq!(r.revives_remaining, 1);
    }

    #[test]
    fn disabled_blocks_take_down() {
        let mut r = Revive::new(5.0, 3.0).disabled();
        r.take_down();
        assert_eq!(r.state, ReviveState::Alive);
    }
}
