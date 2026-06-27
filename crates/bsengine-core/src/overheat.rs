use bevy_ecs::prelude::Component;

/// Thermal state of an overheating entity (weapon, engine, reactor, etc.).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverheatState {
    /// Operating within normal temperature range.
    Normal,
    /// Heat is elevated but still functional (above `warn_threshold`).
    Warning,
    /// Overheated — entity is disabled / jammed until sufficiently cooled.
    Overheated,
    /// Actively venting / cooling down after an overheat (below `cool_threshold`).
    Cooling,
}

/// Weapon / engine / reactor overheating component.
///
/// Separate from `Heat` (which models environmental temperature of a character).
/// `Overheat` models discrete warm-up → jam → forced-cooldown cycles for
/// mechanical entities such as rapid-fire weapons, engines, and generators.
///
/// Call `add_heat(amount)` each time the entity fires / operates. `tick(dt)`
/// drives the passive cool-down and transitions. Query `is_jammed()` to gate
/// operation.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Overheat {
    pub state: OverheatState,
    /// Current heat level [0, max_heat].
    pub heat: f32,
    /// Maximum heat before the entity jams.
    pub max_heat: f32,
    /// Heat fraction at which the Warning state begins.
    pub warn_threshold: f32,
    /// Heat fraction below which the Cooling state ends and Normal resumes.
    pub cool_threshold: f32,
    /// Passive heat dissipation per second (always active).
    pub cool_rate: f32,
    /// Additional cooldown rate applied only while in the Overheated / Cooling state.
    pub forced_cool_rate: f32,
    /// True on the frame heat reaches max_heat and jamming begins.
    pub just_overheated: bool,
    /// True on the frame cooling completes (Cooling → Normal).
    pub just_cooled: bool,
    pub enabled: bool,
}

impl Overheat {
    pub fn new(max_heat: f32, cool_rate: f32) -> Self {
        Self {
            state: OverheatState::Normal,
            heat: 0.0,
            max_heat: max_heat.max(1.0),
            warn_threshold: 0.7,
            cool_threshold: 0.3,
            cool_rate: cool_rate.max(0.0),
            forced_cool_rate: cool_rate,
            just_overheated: false,
            just_cooled: false,
            enabled: true,
        }
    }

    pub fn with_warn_threshold(mut self, fraction: f32) -> Self {
        self.warn_threshold = fraction.clamp(0.0, 1.0);
        self
    }

    pub fn with_cool_threshold(mut self, fraction: f32) -> Self {
        self.cool_threshold = fraction.clamp(0.0, 1.0);
        self
    }

    pub fn with_forced_cool_rate(mut self, rate: f32) -> Self {
        self.forced_cool_rate = rate.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Add heat from a single operation (e.g. one weapon shot).
    /// Returns false if the entity is currently jammed.
    pub fn add_heat(&mut self, amount: f32) -> bool {
        if !self.enabled || self.is_jammed() {
            return false;
        }
        self.heat = (self.heat + amount.max(0.0)).min(self.max_heat);
        true
    }

    /// Advance heat dissipation and state transitions. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_overheated = false;
        self.just_cooled = false;

        // 1. Detect overheat trigger before cooling so reaching max_heat is never missed.
        if self.heat >= self.max_heat && !self.is_jammed() {
            self.state = OverheatState::Overheated;
            self.just_overheated = true;
        }

        // 2. Apply cooling (passive always; forced during Overheated / Cooling).
        let cool = self.cool_rate * dt;
        let forced = match self.state {
            OverheatState::Overheated | OverheatState::Cooling => self.forced_cool_rate * dt,
            _ => 0.0,
        };
        self.heat = (self.heat - cool - forced).max(0.0);

        // 3. State transitions after cooling is applied.
        let fraction = self.heat_fraction();
        self.state = match self.state {
            OverheatState::Overheated => {
                if fraction < 1.0 {
                    OverheatState::Cooling
                } else {
                    OverheatState::Overheated
                }
            }
            OverheatState::Cooling => {
                if fraction <= self.cool_threshold {
                    self.just_cooled = true;
                    OverheatState::Normal
                } else {
                    OverheatState::Cooling
                }
            }
            _ => {
                if fraction >= self.warn_threshold {
                    OverheatState::Warning
                } else {
                    OverheatState::Normal
                }
            }
        };
    }

    pub fn heat_fraction(&self) -> f32 {
        if self.max_heat > 0.0 {
            self.heat / self.max_heat
        } else {
            0.0
        }
    }

    /// Returns true when the entity cannot fire due to overheat.
    pub fn is_jammed(&self) -> bool {
        matches!(
            self.state,
            OverheatState::Overheated | OverheatState::Cooling
        )
    }

    pub fn is_in_warning(&self) -> bool {
        self.state == OverheatState::Warning
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_heat_increments_correctly() {
        let mut o = Overheat::new(100.0, 0.0);
        o.add_heat(30.0);
        o.add_heat(20.0);
        assert!((o.heat - 50.0).abs() < 1e-5);
    }

    #[test]
    fn overheat_jams_on_max_heat() {
        let mut o = Overheat::new(100.0, 0.0);
        o.add_heat(100.0);
        o.tick(0.001);
        assert!(o.is_jammed());
        assert!(o.just_overheated);
    }

    #[test]
    fn jammed_blocks_add_heat() {
        let mut o = Overheat::new(100.0, 0.0);
        o.add_heat(100.0);
        o.tick(0.001);
        assert!(!o.add_heat(10.0));
    }

    #[test]
    fn cooling_resumes_after_overheat() {
        // cool_rate = 50/s, forced = 100/s → 150/s total while cooling
        let mut o = Overheat::new(100.0, 50.0)
            .with_forced_cool_rate(100.0)
            .with_cool_threshold(0.0);
        o.add_heat(100.0);
        o.tick(0.001); // trigger overheat
        o.tick(1.0); // heat should drop to 0
        assert!(!o.is_jammed());
        assert!(o.just_cooled);
    }

    #[test]
    fn warning_state_triggers_at_threshold() {
        let mut o = Overheat::new(100.0, 0.0).with_warn_threshold(0.7);
        o.add_heat(75.0);
        o.tick(0.0);
        assert_eq!(o.state, OverheatState::Warning);
    }

    #[test]
    fn disabled_blocks_add_heat() {
        let mut o = Overheat::new(100.0, 0.0).disabled();
        assert!(!o.add_heat(50.0));
        assert_eq!(o.heat, 0.0);
    }
}
