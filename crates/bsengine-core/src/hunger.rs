use bevy_ecs::prelude::Component;

/// Coarse-grained hunger/nutrition state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HungerState {
    /// Well-fed; no penalties.
    Satiated,
    /// Below `hungry_threshold`; designer-defined penalties apply.
    Hungry,
    /// Below `starving_threshold`; `damage_rate` HP/s is dealt.
    Starving,
}

/// Survival hunger/needs component.
///
/// `hunger` ranges 0.0 (full) to 1.0 (starving). It rises at `depletion_rate`
/// per second via `tick(dt)`. When it crosses `hungry_threshold` the state
/// becomes `Hungry`; at `starving_threshold` it becomes `Starving`.
///
/// `tick(dt)` returns the HP penalty for this frame (`damage_rate * dt` while
/// `Starving`, otherwise 0.0).
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hunger {
    pub state: HungerState,
    /// Current hunger level (0.0 = full, 1.0 = maximal hunger).
    pub hunger: f32,
    /// Rate at which hunger increases per second.
    pub depletion_rate: f32,
    /// Hunger level at which the entity becomes Hungry.
    pub hungry_threshold: f32,
    /// Hunger level at which the entity becomes Starving.
    pub starving_threshold: f32,
    /// HP damage per second while Starving.
    pub damage_rate: f32,
    pub enabled: bool,
}

impl Hunger {
    pub fn new(depletion_rate: f32, hungry_threshold: f32, starving_threshold: f32) -> Self {
        Self {
            state: HungerState::Satiated,
            hunger: 0.0,
            depletion_rate: depletion_rate.max(0.0),
            hungry_threshold: hungry_threshold.clamp(0.0, 1.0),
            starving_threshold: starving_threshold.clamp(0.0, 1.0),
            damage_rate: 1.0,
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

    /// Reduce hunger by `amount`. Returns the actual amount restored.
    pub fn eat(&mut self, amount: f32) -> f32 {
        let prev = self.hunger;
        self.hunger = (self.hunger - amount.max(0.0)).max(0.0);
        self.update_state();
        prev - self.hunger
    }

    /// Advance hunger. Returns HP damage dealt this frame (0 unless Starving).
    pub fn tick(&mut self, dt: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        self.hunger = (self.hunger + self.depletion_rate * dt).min(1.0);
        self.update_state();
        if self.state == HungerState::Starving {
            self.damage_rate * dt
        } else {
            0.0
        }
    }

    fn update_state(&mut self) {
        self.state = if self.hunger >= self.starving_threshold {
            HungerState::Starving
        } else if self.hunger >= self.hungry_threshold {
            HungerState::Hungry
        } else {
            HungerState::Satiated
        };
    }

    pub fn is_hungry(&self) -> bool {
        self.state == HungerState::Hungry || self.state == HungerState::Starving
    }

    pub fn is_starving(&self) -> bool {
        self.state == HungerState::Starving
    }

    /// Fraction of hunger (0.0 = full, 1.0 = starving).
    pub fn fraction(&self) -> f32 {
        self.hunger
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_increases_hunger() {
        let mut h = Hunger::new(0.1, 0.4, 0.8);
        h.tick(1.0);
        assert!((h.hunger - 0.1).abs() < 1e-5);
    }

    #[test]
    fn transitions_to_hungry() {
        let mut h = Hunger::new(0.5, 0.4, 0.8);
        h.tick(1.0); // hunger = 0.5 > 0.4 → Hungry
        assert_eq!(h.state, HungerState::Hungry);
    }

    #[test]
    fn transitions_to_starving_and_deals_damage() {
        let mut h = Hunger::new(1.0, 0.4, 0.8).with_damage_rate(5.0);
        let dmg = h.tick(1.0); // hunger = 1.0 >= 0.8 → Starving
        assert!(h.is_starving());
        assert!((dmg - 5.0).abs() < 1e-5);
    }

    #[test]
    fn eat_reduces_hunger() {
        let mut h = Hunger::new(1.0, 0.4, 0.8);
        h.tick(1.0); // hunger = 1.0
        let restored = h.eat(0.5);
        assert!((h.hunger - 0.5).abs() < 1e-5);
        assert!((restored - 0.5).abs() < 1e-5);
    }

    #[test]
    fn eat_transitions_back_to_satiated() {
        let mut h = Hunger::new(1.0, 0.4, 0.8);
        h.tick(1.0);
        h.eat(1.0);
        assert_eq!(h.state, HungerState::Satiated);
    }

    #[test]
    fn disabled_ignores_tick() {
        let mut h = Hunger::new(1.0, 0.4, 0.8).disabled();
        let dmg = h.tick(10.0);
        assert_eq!(h.hunger, 0.0);
        assert_eq!(dmg, 0.0);
    }
}
