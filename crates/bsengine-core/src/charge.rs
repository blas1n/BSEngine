use bevy_ecs::prelude::Component;

/// Phase of a charge-up ability cycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChargePhase {
    /// Input not held; no charge accumulating.
    Idle,
    /// Input held; charge is building toward `max_charge`.
    Charging,
    /// Charge held at max waiting for release.
    Overcharge,
    /// Released; charge is being consumed / fired.
    Releasing,
}

/// Hold-to-charge ability or weapon component (charged shot, power swing, channelled cast).
///
/// The ability system calls:
///   - `begin()` when the player presses the charge button.
///   - `tick(dt)` every frame while input is held.
///   - `release()` when the player releases — triggers the Releasing phase.
///   - `tick(dt)` continues in Releasing until `release_timer` drains, then back to Idle.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Charge {
    pub phase: ChargePhase,
    /// Current accumulated charge [0, max_charge].
    pub current: f32,
    /// Maximum charge level (fully charged = 1× power multiplier when normalised).
    pub max_charge: f32,
    /// Rate at which charge builds per second.
    pub charge_rate: f32,
    /// Minimum charge required before releasing is considered valid.
    pub min_charge: f32,
    /// How long the release animation / effect lasts (seconds).
    pub release_duration: f32,
    /// Timer counting down during the Releasing phase.
    pub release_timer: f32,
    /// Whether the player is currently holding the charge button.
    pub wants_charge: bool,
    pub enabled: bool,
}

impl Charge {
    pub fn new(max_charge: f32, charge_rate: f32) -> Self {
        Self {
            phase: ChargePhase::Idle,
            current: 0.0,
            max_charge: max_charge.max(0.01),
            charge_rate: charge_rate.max(0.0),
            min_charge: 0.0,
            release_duration: 0.1,
            wants_charge: false,
            release_timer: 0.0,
            enabled: true,
        }
    }

    pub fn with_min_charge(mut self, min: f32) -> Self {
        self.min_charge = min.clamp(0.0, self.max_charge);
        self
    }

    pub fn with_release_duration(mut self, secs: f32) -> Self {
        self.release_duration = secs.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Start charging. No-op if not Idle.
    pub fn begin(&mut self) {
        if self.enabled && self.phase == ChargePhase::Idle {
            self.phase = ChargePhase::Charging;
            self.current = 0.0;
        }
    }

    /// Release. If charge is below `min_charge` the charge is cancelled without firing.
    /// Returns `true` if the release was valid (fires the ability).
    pub fn release(&mut self) -> bool {
        match self.phase {
            ChargePhase::Charging | ChargePhase::Overcharge => {
                if self.current >= self.min_charge {
                    self.phase = ChargePhase::Releasing;
                    self.release_timer = self.release_duration;
                    true
                } else {
                    self.cancel();
                    false
                }
            }
            _ => false,
        }
    }

    /// Abandon the charge without firing.
    pub fn cancel(&mut self) {
        self.phase = ChargePhase::Idle;
        self.current = 0.0;
        self.release_timer = 0.0;
    }

    /// Advance timers. Call every frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        match self.phase {
            ChargePhase::Charging => {
                self.current = (self.current + self.charge_rate * dt).min(self.max_charge);
                if self.current >= self.max_charge {
                    self.phase = ChargePhase::Overcharge;
                }
            }
            ChargePhase::Releasing => {
                self.release_timer -= dt;
                if self.release_timer <= 0.0 {
                    self.phase = ChargePhase::Idle;
                    self.current = 0.0;
                }
            }
            _ => {}
        }
    }

    /// Normalised charge [0, 1].
    pub fn fraction(&self) -> f32 {
        if self.max_charge > 0.0 {
            self.current / self.max_charge
        } else {
            0.0
        }
    }

    pub fn is_fully_charged(&self) -> bool {
        self.current >= self.max_charge
    }

    pub fn is_charging(&self) -> bool {
        matches!(self.phase, ChargePhase::Charging | ChargePhase::Overcharge)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn begin_enters_charging() {
        let mut c = Charge::new(1.0, 2.0);
        c.begin();
        assert_eq!(c.phase, ChargePhase::Charging);
    }

    #[test]
    fn tick_builds_charge() {
        let mut c = Charge::new(1.0, 2.0);
        c.begin();
        c.tick(0.25); // 2.0 * 0.25 = 0.5
        assert!((c.current - 0.5).abs() < 1e-6);
    }

    #[test]
    fn full_charge_transitions_to_overcharge() {
        let mut c = Charge::new(1.0, 10.0);
        c.begin();
        c.tick(1.0);
        assert_eq!(c.phase, ChargePhase::Overcharge);
        assert!(c.is_fully_charged());
    }

    #[test]
    fn release_valid_enters_releasing() {
        let mut c = Charge::new(1.0, 10.0).with_release_duration(0.2);
        c.begin();
        c.tick(1.0); // fully charged
        let fired = c.release();
        assert!(fired);
        assert_eq!(c.phase, ChargePhase::Releasing);
    }

    #[test]
    fn release_below_min_cancels() {
        let mut c = Charge::new(1.0, 2.0).with_min_charge(0.5);
        c.begin();
        c.tick(0.1); // 0.2 charge, below min 0.5
        let fired = c.release();
        assert!(!fired);
        assert_eq!(c.phase, ChargePhase::Idle);
    }

    #[test]
    fn releasing_returns_to_idle_after_duration() {
        let mut c = Charge::new(1.0, 10.0).with_release_duration(0.1);
        c.begin();
        c.tick(1.0);
        c.release();
        c.tick(0.2);
        assert_eq!(c.phase, ChargePhase::Idle);
        assert!((c.current).abs() < 1e-6);
    }

    #[test]
    fn cancel_resets_to_idle() {
        let mut c = Charge::new(1.0, 2.0);
        c.begin();
        c.tick(0.3);
        c.cancel();
        assert_eq!(c.phase, ChargePhase::Idle);
        assert!((c.current).abs() < 1e-6);
    }
}
