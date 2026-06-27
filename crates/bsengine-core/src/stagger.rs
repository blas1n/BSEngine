use bevy_ecs::prelude::Component;

/// Phase of the stagger state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StaggerPhase {
    /// Entity is operating normally.
    None,
    /// Entity was hit hard enough to stagger; movement and actions are interrupted.
    Staggered,
    /// Stagger animation finished; entity is regaining control.
    Recovering,
}

/// Brief interrupt/stun state triggered when incoming force exceeds a threshold.
///
/// The hit system calls `apply(force)` on each damaging hit. If `force >=
/// stagger_threshold` and the entity is not already staggered, the entity
/// enters `Staggered` for `stagger_duration` seconds, then `Recovering` for
/// `recovery_duration` seconds, then returns to `None`.
///
/// `resist` (0.0–1.0) is not a random roll; it is a damage-reduction factor
/// applied before comparing against the threshold, letting designers express
/// partial resistance without RNG.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stagger {
    pub phase: StaggerPhase,
    /// Minimum force required to trigger a stagger.
    pub stagger_threshold: f32,
    /// How long (seconds) the Staggered phase lasts.
    pub stagger_duration: f32,
    /// Time remaining in the Staggered phase.
    pub stagger_timer: f32,
    /// How long (seconds) the Recovering phase lasts.
    pub recovery_duration: f32,
    /// Time remaining in the Recovering phase.
    pub recovery_timer: f32,
    /// Total staggers accumulated (resets on `reset()`).
    pub stagger_count: u32,
    /// Force reduction factor before threshold comparison (0.0 = immune, 1.0 = full).
    pub resist: f32,
    /// True on the frame the entity enters the Staggered phase.
    pub just_staggered: bool,
    pub enabled: bool,
}

impl Stagger {
    pub fn new(stagger_threshold: f32, stagger_duration: f32, recovery_duration: f32) -> Self {
        Self {
            phase: StaggerPhase::None,
            stagger_threshold: stagger_threshold.max(0.0),
            stagger_duration: stagger_duration.max(0.0),
            stagger_timer: 0.0,
            recovery_duration: recovery_duration.max(0.0),
            recovery_timer: 0.0,
            stagger_count: 0,
            resist: 1.0,
            just_staggered: false,
            enabled: true,
        }
    }

    pub fn with_resist(mut self, factor: f32) -> Self {
        self.resist = factor.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply incoming `force`. Returns true if a stagger was triggered.
    pub fn apply(&mut self, force: f32) -> bool {
        if !self.enabled || self.phase != StaggerPhase::None {
            return false;
        }
        let effective = force * self.resist;
        if effective >= self.stagger_threshold {
            self.phase = StaggerPhase::Staggered;
            self.stagger_timer = self.stagger_duration;
            self.just_staggered = true;
            self.stagger_count += 1;
            return true;
        }
        false
    }

    /// Advance the stagger state. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_staggered = false;
        if !self.enabled {
            return;
        }
        match self.phase {
            StaggerPhase::Staggered => {
                self.stagger_timer -= dt;
                if self.stagger_timer <= 0.0 {
                    let remaining = -self.stagger_timer;
                    self.phase = StaggerPhase::Recovering;
                    self.recovery_timer = self.recovery_duration - remaining;
                    if self.recovery_timer <= 0.0 {
                        self.phase = StaggerPhase::None;
                        self.recovery_timer = 0.0;
                    }
                }
            }
            StaggerPhase::Recovering => {
                self.recovery_timer -= dt;
                if self.recovery_timer <= 0.0 {
                    self.phase = StaggerPhase::None;
                    self.recovery_timer = 0.0;
                }
            }
            StaggerPhase::None => {}
        }
    }

    /// Forcibly cancel the stagger and return to None.
    pub fn reset(&mut self) {
        self.phase = StaggerPhase::None;
        self.stagger_timer = 0.0;
        self.recovery_timer = 0.0;
    }

    pub fn is_staggered(&self) -> bool {
        self.phase == StaggerPhase::Staggered
    }

    pub fn is_recovering(&self) -> bool {
        self.phase == StaggerPhase::Recovering
    }

    /// True when the entity can be staggered (not already staggered or recovering).
    pub fn can_stagger(&self) -> bool {
        self.enabled && self.phase == StaggerPhase::None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_above_threshold_staggers() {
        let mut s = Stagger::new(10.0, 0.5, 0.2);
        assert!(s.apply(15.0));
        assert!(s.is_staggered());
        assert!(s.just_staggered);
        assert_eq!(s.stagger_count, 1);
    }

    #[test]
    fn apply_below_threshold_does_not_stagger() {
        let mut s = Stagger::new(10.0, 0.5, 0.2);
        assert!(!s.apply(5.0));
        assert_eq!(s.phase, StaggerPhase::None);
    }

    #[test]
    fn stagger_transitions_to_recovering() {
        let mut s = Stagger::new(10.0, 0.3, 0.2);
        s.apply(15.0);
        s.tick(0.4);
        assert!(s.is_recovering());
    }

    #[test]
    fn recovery_returns_to_none() {
        let mut s = Stagger::new(10.0, 0.3, 0.2);
        s.apply(15.0);
        s.tick(0.6);
        assert_eq!(s.phase, StaggerPhase::None);
    }

    #[test]
    fn resist_reduces_effective_force() {
        let mut s = Stagger::new(10.0, 0.5, 0.2).with_resist(0.5);
        // 15 * 0.5 = 7.5 < 10 → no stagger
        assert!(!s.apply(15.0));
        // 25 * 0.5 = 12.5 >= 10 → stagger
        assert!(s.apply(25.0));
    }

    #[test]
    fn already_staggered_blocks_second_stagger() {
        let mut s = Stagger::new(10.0, 0.5, 0.2);
        s.apply(15.0);
        assert!(!s.apply(15.0));
        assert_eq!(s.stagger_count, 1);
    }
}
