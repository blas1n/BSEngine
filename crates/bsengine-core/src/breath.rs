use bevy_ecs::prelude::Component;

/// Submersion breathing state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreathState {
    /// Above water; air meter is recovering.
    Normal,
    /// Submerged; air is depleting but not yet empty.
    HoldingBreath,
    /// Submerged with no air; HP damage is accumulating.
    Drowning,
}

/// Underwater air / breath meter component.
///
/// The water system sets `submerged` each frame. While submerged, `air`
/// depletes at `depletion_rate`/s. While surfaced, it recovers at
/// `recovery_rate`/s. When `air` reaches 0, the entity starts Drowning and
/// `tick(dt)` returns `damage_rate * dt` HP each frame until they surface.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Breath {
    pub state: BreathState,
    /// Current air supply (0.0 = empty, 1.0 = full).
    pub air: f32,
    /// Air depleted per second while submerged.
    pub depletion_rate: f32,
    /// Air recovered per second while surfaced.
    pub recovery_rate: f32,
    /// HP damage per second while Drowning.
    pub damage_rate: f32,
    /// Set by the water system each frame; true when the entity is underwater.
    pub submerged: bool,
    /// True on the frame the entity transitions to Drowning.
    pub just_started_drowning: bool,
    /// True on the frame the entity surfaces after Drowning or HoldingBreath.
    pub just_surfaced: bool,
    pub enabled: bool,
}

impl Breath {
    pub fn new(depletion_rate: f32, recovery_rate: f32) -> Self {
        Self {
            state: BreathState::Normal,
            air: 1.0,
            depletion_rate: depletion_rate.max(0.0),
            recovery_rate: recovery_rate.max(0.0),
            damage_rate: 2.0,
            submerged: false,
            just_started_drowning: false,
            just_surfaced: false,
            enabled: true,
        }
    }

    pub fn with_damage_rate(mut self, rate: f32) -> Self {
        self.damage_rate = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance breath simulation. Returns HP damage this frame (0 unless Drowning).
    pub fn tick(&mut self, dt: f32) -> f32 {
        self.just_started_drowning = false;
        self.just_surfaced = false;

        if !self.enabled {
            return 0.0;
        }

        let was_submerged = self.state != BreathState::Normal;

        if self.submerged {
            if self.air > 0.0 {
                self.air = (self.air - self.depletion_rate * dt).max(0.0);
            }
            let new_state = if self.air <= 0.0 {
                BreathState::Drowning
            } else {
                BreathState::HoldingBreath
            };
            if new_state == BreathState::Drowning && self.state != BreathState::Drowning {
                self.just_started_drowning = true;
            }
            self.state = new_state;
        } else {
            self.air = (self.air + self.recovery_rate * dt).min(1.0);
            if was_submerged {
                self.just_surfaced = true;
            }
            self.state = BreathState::Normal;
        }

        if self.state == BreathState::Drowning {
            self.damage_rate * dt
        } else {
            0.0
        }
    }

    pub fn is_drowning(&self) -> bool {
        self.state == BreathState::Drowning
    }

    pub fn is_holding_breath(&self) -> bool {
        self.state == BreathState::HoldingBreath
    }

    /// Air supply as a fraction (0.0 = empty, 1.0 = full).
    pub fn air_fraction(&self) -> f32 {
        self.air
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submerged_depletes_air() {
        let mut b = Breath::new(0.5, 1.0);
        b.submerged = true;
        b.tick(1.0);
        assert!((b.air - 0.5).abs() < 1e-5);
        assert_eq!(b.state, BreathState::HoldingBreath);
    }

    #[test]
    fn empty_air_transitions_to_drowning() {
        let mut b = Breath::new(2.0, 1.0);
        b.submerged = true;
        b.tick(1.0); // 1.0 - 2.0 = 0 → Drowning
        assert!(b.is_drowning());
        assert!(b.just_started_drowning);
    }

    #[test]
    fn drowning_deals_damage() {
        let mut b = Breath::new(2.0, 1.0).with_damage_rate(5.0);
        b.submerged = true;
        b.tick(1.0); // enters Drowning
        let dmg = b.tick(1.0);
        assert!((dmg - 5.0).abs() < 1e-5);
    }

    #[test]
    fn surfacing_recovers_air() {
        let mut b = Breath::new(1.0, 0.5);
        b.submerged = true;
        b.tick(1.0); // air = 0 → Drowning
        b.submerged = false;
        b.tick(1.0); // recover 0.5
        assert!((b.air - 0.5).abs() < 1e-5);
        assert_eq!(b.state, BreathState::Normal);
        assert!(b.just_surfaced);
    }

    #[test]
    fn disabled_ignores_tick() {
        let mut b = Breath::new(1.0, 1.0).disabled();
        b.submerged = true;
        let dmg = b.tick(10.0);
        assert_eq!(b.air, 1.0);
        assert_eq!(dmg, 0.0);
    }

    #[test]
    fn full_air_no_damage_while_not_submerged() {
        let mut b = Breath::new(0.5, 1.0);
        let dmg = b.tick(1.0);
        assert_eq!(b.state, BreathState::Normal);
        assert_eq!(dmg, 0.0);
    }
}
