use bevy_ecs::prelude::Component;

/// Phase of the rage cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RagePhase {
    /// Rage meter is empty or below the activation threshold.
    Calm,
    /// Rage is building toward the activation threshold.
    Building,
    /// Character is in full rage — buffs active, timer running.
    Raging,
    /// Rage has expired; cooling down before the meter can fill again.
    Cooling,
}

/// Rage / berserk state component — attack buff that builds from taking damage.
///
/// Attach to characters that have a rage mechanic (berserkers, monsters, etc.).
/// Call `on_hit(amount)` whenever the character takes damage to build rage.
/// `tick(dt)` advances the rage timer and handles transitions.
/// While Raging, multiply outgoing damage by `damage_multiplier` and incoming
/// defense by `defense_multiplier`.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Rage {
    pub phase: RagePhase,
    /// Current rage [0, max_rage].
    pub rage: f32,
    pub max_rage: f32,
    /// Rage gained per unit of damage taken.
    pub rage_per_damage: f32,
    /// Fraction of max_rage required to trigger Raging.
    pub activation_threshold: f32,
    /// Passive decay rate (rage/s) while Calm (not in combat).
    pub decay_rate: f32,
    /// How long Raging lasts (seconds, 0 = until rage depletes).
    pub rage_duration: f32,
    pub rage_timer: f32,
    /// Duration of the Cooling phase before the meter can build again.
    pub cooldown_duration: f32,
    pub cooldown_timer: f32,
    /// Outgoing damage multiplier while Raging.
    pub damage_multiplier: f32,
    /// Incoming damage multiplier while Raging (< 1.0 = more fragile).
    pub defense_multiplier: f32,
    /// True on the frame Raging begins.
    pub just_entered_rage: bool,
    /// True on the frame Raging ends (transitions to Cooling).
    pub just_left_rage: bool,
    pub enabled: bool,
}

impl Rage {
    pub fn new(max_rage: f32, rage_per_damage: f32) -> Self {
        Self {
            phase: RagePhase::Calm,
            rage: 0.0,
            max_rage: max_rage.max(1.0),
            rage_per_damage: rage_per_damage.max(0.0),
            activation_threshold: 0.8,
            decay_rate: 5.0,
            rage_duration: 5.0,
            rage_timer: 0.0,
            cooldown_duration: 3.0,
            cooldown_timer: 0.0,
            damage_multiplier: 2.0,
            defense_multiplier: 1.5,
            just_entered_rage: false,
            just_left_rage: false,
            enabled: true,
        }
    }

    pub fn with_activation_threshold(mut self, fraction: f32) -> Self {
        self.activation_threshold = fraction.clamp(0.0, 1.0);
        self
    }

    pub fn with_multipliers(mut self, damage: f32, defense: f32) -> Self {
        self.damage_multiplier = damage.max(1.0);
        self.defense_multiplier = defense.max(1.0);
        self
    }

    pub fn with_rage_duration(mut self, secs: f32) -> Self {
        self.rage_duration = secs.max(0.0);
        self
    }

    pub fn with_cooldown(mut self, secs: f32) -> Self {
        self.cooldown_duration = secs.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Call when the character takes `damage` to accumulate rage.
    /// Returns the actual rage added.
    pub fn on_hit(&mut self, damage: f32) -> f32 {
        if !self.enabled || self.phase == RagePhase::Cooling {
            return 0.0;
        }
        let gained = (damage * self.rage_per_damage).max(0.0);
        self.rage = (self.rage + gained).min(self.max_rage);
        if self.phase == RagePhase::Calm || self.phase == RagePhase::Building {
            self.phase = RagePhase::Building;
        }
        gained
    }

    /// Advance timers and state. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_entered_rage = false;
        self.just_left_rage = false;

        match self.phase {
            RagePhase::Calm => {
                // Passive decay.
                self.rage = (self.rage - self.decay_rate * dt).max(0.0);
            }
            RagePhase::Building => {
                // Check activation.
                if self.rage >= self.max_rage * self.activation_threshold {
                    self.phase = RagePhase::Raging;
                    self.rage_timer = self.rage_duration;
                    self.just_entered_rage = true;
                } else {
                    // Decay if not actively building.
                    self.rage = (self.rage - self.decay_rate * dt * 0.5).max(0.0);
                    if self.rage <= 0.0 {
                        self.phase = RagePhase::Calm;
                    }
                }
            }
            RagePhase::Raging => {
                if self.rage_duration > 0.0 {
                    self.rage_timer = (self.rage_timer - dt).max(0.0);
                    if self.rage_timer <= 0.0 {
                        self.phase = RagePhase::Cooling;
                        self.cooldown_timer = self.cooldown_duration;
                        self.rage = 0.0;
                        self.just_left_rage = true;
                    }
                }
            }
            RagePhase::Cooling => {
                self.cooldown_timer = (self.cooldown_timer - dt).max(0.0);
                if self.cooldown_timer <= 0.0 {
                    self.phase = RagePhase::Calm;
                }
            }
        }
    }

    pub fn is_raging(&self) -> bool {
        self.phase == RagePhase::Raging
    }

    /// Effective outgoing damage multiplier for the current phase.
    pub fn current_damage_multiplier(&self) -> f32 {
        if self.is_raging() {
            self.damage_multiplier
        } else {
            1.0
        }
    }

    /// Effective incoming damage multiplier for the current phase.
    pub fn current_defense_multiplier(&self) -> f32 {
        if self.is_raging() {
            self.defense_multiplier
        } else {
            1.0
        }
    }

    pub fn rage_fraction(&self) -> f32 {
        self.rage / self.max_rage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rage() -> Rage {
        Rage::new(100.0, 0.5)
            .with_activation_threshold(0.8)
            .with_rage_duration(3.0)
            .with_cooldown(1.0)
            .with_multipliers(2.0, 1.5)
    }

    #[test]
    fn on_hit_builds_rage() {
        let mut r = rage();
        r.on_hit(50.0); // 50 * 0.5 = 25
        assert!((r.rage - 25.0).abs() < 1e-5);
        assert_eq!(r.phase, RagePhase::Building);
    }

    #[test]
    fn activation_threshold_triggers_raging() {
        let mut r = rage();
        r.on_hit(200.0); // fills rage to 100
        r.tick(0.0);
        assert_eq!(r.phase, RagePhase::Raging);
        assert!(r.just_entered_rage);
    }

    #[test]
    fn multipliers_active_while_raging() {
        let mut r = rage();
        r.on_hit(200.0);
        r.tick(0.0);
        assert!((r.current_damage_multiplier() - 2.0).abs() < 1e-5);
        assert!((r.current_defense_multiplier() - 1.5).abs() < 1e-5);
    }

    #[test]
    fn rage_expires_and_enters_cooldown() {
        let mut r = rage();
        r.on_hit(200.0);
        r.tick(0.0); // enter rage
        r.tick(3.0); // exhaust rage_duration
        assert_eq!(r.phase, RagePhase::Cooling);
        assert!(r.just_left_rage);
    }

    #[test]
    fn cooldown_blocks_rage_build() {
        let mut r = rage();
        r.on_hit(200.0);
        r.tick(0.0);
        r.tick(3.0); // expire
        let added = r.on_hit(100.0);
        assert_eq!(added, 0.0); // blocked during cooldown
    }

    #[test]
    fn disabled_ignores_on_hit() {
        let mut r = rage().disabled();
        r.on_hit(200.0);
        assert_eq!(r.rage, 0.0);
    }
}
