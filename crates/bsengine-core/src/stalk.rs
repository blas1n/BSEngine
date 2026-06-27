use bevy_ecs::prelude::Component;

/// First-strike hunt bonus: while in stalking mode, the entity's next
/// outgoing attack deals amplified damage. The damage system calls
/// `consume()` when an attack lands — it returns `damage_multiplier`
/// and ends the stalk in one atomic operation. Subsequent attacks deal
/// normal damage until `begin()` is called again.
///
/// `begin()` enters stalking mode and sets `just_began`. `consume()`
/// returns the multiplier and sets `just_consumed`; it returns `1.0`
/// (no bonus) when not stalking or disabled. `tick()` clears one-frame
/// flags.
///
/// Distinct from `Stealth` (full visibility suppression), `Ambush` (area
/// burst on approach), and `Charge` (wind-up movement that empowers the
/// next attack over time): Stalk is a **declarative first-strike state**
/// — the entity opts in to the bonus, then the bonus fires and resets on
/// the very next hit without any timer or distance constraint.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Stalk {
    pub active: bool,
    /// Outgoing damage multiplier on the first hit while stalking. Clamped ≥ 1.0.
    pub damage_multiplier: f32,
    pub just_began: bool,
    pub just_consumed: bool,
    pub enabled: bool,
}

impl Stalk {
    pub fn new(damage_multiplier: f32) -> Self {
        Self {
            active: false,
            damage_multiplier: damage_multiplier.max(1.0),
            just_began: false,
            just_consumed: false,
            enabled: true,
        }
    }

    /// Enter stalking mode. Sets `just_began`. No-op when already stalking,
    /// disabled, or called redundantly while active.
    pub fn begin(&mut self) {
        if !self.enabled || self.active {
            return;
        }
        self.active = true;
        self.just_began = true;
    }

    /// Called by the damage system on the next outgoing hit. Returns
    /// `damage_multiplier` and ends the stalk, setting `just_consumed`.
    /// Returns `1.0` when not stalking or disabled (no bonus, no state change).
    pub fn consume(&mut self) -> f32 {
        if !self.enabled || !self.active {
            return 1.0;
        }
        self.active = false;
        self.just_consumed = true;
        self.damage_multiplier
    }

    /// Clear one-frame flags. Call once per game tick.
    pub fn tick(&mut self) {
        self.just_began = false;
        self.just_consumed = false;
    }

    pub fn is_stalking(&self) -> bool {
        self.active
    }
}

impl Default for Stalk {
    fn default() -> Self {
        Self::new(2.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_enters_stalk() {
        let mut s = Stalk::new(2.0);
        s.begin();
        assert!(s.is_stalking());
        assert!(s.just_began);
    }

    #[test]
    fn begin_no_op_when_already_stalking() {
        let mut s = Stalk::new(2.0);
        s.begin();
        s.tick();
        s.begin();
        assert!(!s.just_began);
    }

    #[test]
    fn consume_returns_multiplier_and_ends_stalk() {
        let mut s = Stalk::new(3.0);
        s.begin();
        let m = s.consume();
        assert!((m - 3.0).abs() < 1e-5);
        assert!(!s.is_stalking());
        assert!(s.just_consumed);
    }

    #[test]
    fn consume_returns_one_when_not_stalking() {
        let mut s = Stalk::new(2.0);
        let m = s.consume();
        assert!((m - 1.0).abs() < 1e-5);
        assert!(!s.just_consumed);
    }

    #[test]
    fn second_consume_returns_one() {
        let mut s = Stalk::new(2.0);
        s.begin();
        s.consume(); // first hit: consumes the stalk
        let m = s.consume(); // second hit: no stalk active
        assert!((m - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_began() {
        let mut s = Stalk::new(2.0);
        s.begin();
        s.tick();
        assert!(!s.just_began);
    }

    #[test]
    fn tick_clears_just_consumed() {
        let mut s = Stalk::new(2.0);
        s.begin();
        s.consume();
        s.tick();
        assert!(!s.just_consumed);
    }

    #[test]
    fn can_stalk_again_after_consuming() {
        let mut s = Stalk::new(2.0);
        s.begin();
        s.consume();
        s.tick();
        s.begin();
        assert!(s.is_stalking());
        assert!(s.just_began);
    }

    #[test]
    fn damage_multiplier_clamped_at_one() {
        let s = Stalk::new(0.5);
        assert!((s.damage_multiplier - 1.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_begin_no_op() {
        let mut s = Stalk::new(2.0);
        s.enabled = false;
        s.begin();
        assert!(!s.is_stalking());
        assert!(!s.just_began);
    }

    #[test]
    fn disabled_consume_returns_one() {
        let mut s = Stalk::new(2.0);
        s.begin();
        s.enabled = false;
        let m = s.consume();
        assert!((m - 1.0).abs() < 1e-5);
        assert!(s.is_stalking()); // still active — disabled did not consume
    }

    #[test]
    fn default_multiplier_two() {
        let s = Stalk::default();
        assert!((s.damage_multiplier - 2.0).abs() < 1e-5);
    }
}
