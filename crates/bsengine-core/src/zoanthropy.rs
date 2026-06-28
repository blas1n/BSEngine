use bevy_ecs::prelude::Component;

/// Animal-delusion accumulation tracker named after zoanthropy, the
/// psychiatric condition in which a person believes that they have
/// been transformed into an animal or are temporarily assuming an
/// animal's form. The word descends from Greek zoion (animal) and
/// anthropos (human being), and the condition appears in some of the
/// oldest written records of mental illness: the Book of Daniel
/// describes Nebuchadnezzar reduced to grazing in the fields "like
/// an ox," a passage that medieval physicians cited as clinical
/// authority for the diagnosis; Herodotus recorded that the Scythians
/// turned periodically into wolves; the werewolf traditions of
/// northern Europe and the nagual traditions of Mesoamerica both
/// encode the same psychological phenomenon in mythological garb.
/// Modern psychiatry classifies zoanthropic delusions under the
/// broader category of clinical lycanthropy, though reported cases
/// include patients who believed themselves to be frogs, snakes,
/// horses, bees, and birds as well as the canonical lupine form.
/// The delusion is typically episodic, waxing and waning with
/// psychotic breaks, which makes it well-suited to a resource-bar
/// model: `delusion` climbs via `inhabit(amount)` and accumulates
/// passively at `inhabit_rate` per second in `tick(dt)` or retreats
/// via `dispel(amount)`.
///
/// Models zoanthropic delusion-intensity gauges, lycanthropy-phase
/// saturation bars, animal-transformation belief accumulation
/// trackers, were-creature shift meters, nagual-manifestation fill
/// levels, beast-possession saturation indicators, feral-cognition
/// accumulation bars, shapeshifter-instinct overflow meters, ritual-
/// regression delusion fill levels, or any mechanic where a character
/// slides incrementally toward believing themselves to be a creature
/// of the wild — losing language, ignoring tools, crouching to
/// drink at streams — until the final threshold is crossed and the
/// human mind disappears entirely into the animal body it insists
/// it inhabits.
///
/// `inhabit(amount)` adds delusion; fires `just_feral` when first
/// reaching `max_delusion`. No-op when disabled.
///
/// `dispel(amount)` reduces delusion immediately; fires `just_lucid`
/// when reaching 0. No-op when disabled or already lucid.
///
/// `tick(dt)` clears both flags, then increases delusion by
/// `inhabit_rate * dt` (capped at `max_delusion`). Fires `just_feral`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_feral()` returns `delusion >= max_delusion && enabled`.
///
/// `is_lucid()` returns `delusion == 0.0` (not gated by `enabled`).
///
/// `delusion_fraction()` returns
/// `(delusion / max_delusion).clamp(0, 1)`.
///
/// `effective_animism(scale)` returns `scale * delusion_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — inhabits at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoanthropy {
    pub delusion: f32,
    pub max_delusion: f32,
    pub inhabit_rate: f32,
    pub just_feral: bool,
    pub just_lucid: bool,
    pub enabled: bool,
}

impl Zoanthropy {
    pub fn new(max_delusion: f32, inhabit_rate: f32) -> Self {
        Self {
            delusion: 0.0,
            max_delusion: max_delusion.max(0.1),
            inhabit_rate: inhabit_rate.max(0.0),
            just_feral: false,
            just_lucid: false,
            enabled: true,
        }
    }

    /// Add delusion; fires `just_feral` when first reaching max.
    /// No-op when disabled.
    pub fn inhabit(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.delusion < self.max_delusion;
        self.delusion = (self.delusion + amount).min(self.max_delusion);
        if was_below && self.delusion >= self.max_delusion {
            self.just_feral = true;
        }
    }

    /// Reduce delusion; fires `just_lucid` when reaching 0.
    /// No-op when disabled or already lucid.
    pub fn dispel(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.delusion <= 0.0 {
            return;
        }
        self.delusion = (self.delusion - amount).max(0.0);
        if self.delusion <= 0.0 {
            self.just_lucid = true;
        }
    }

    /// Clear flags, then increase delusion by `inhabit_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_feral = false;
        self.just_lucid = false;
        if self.enabled && self.inhabit_rate > 0.0 && self.delusion < self.max_delusion {
            let was_below = self.delusion < self.max_delusion;
            self.delusion = (self.delusion + self.inhabit_rate * dt).min(self.max_delusion);
            if was_below && self.delusion >= self.max_delusion {
                self.just_feral = true;
            }
        }
    }

    /// `true` when delusion is at maximum and component is enabled.
    pub fn is_feral(&self) -> bool {
        self.delusion >= self.max_delusion && self.enabled
    }

    /// `true` when delusion is 0 (not gated by `enabled`).
    pub fn is_lucid(&self) -> bool {
        self.delusion == 0.0
    }

    /// Fraction of maximum delusion [0.0, 1.0].
    pub fn delusion_fraction(&self) -> f32 {
        (self.delusion / self.max_delusion).clamp(0.0, 1.0)
    }

    /// Returns `scale * delusion_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_animism(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.delusion_fraction()
    }
}

impl Default for Zoanthropy {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoanthropy {
        Zoanthropy::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_lucid() {
        let z = z();
        assert_eq!(z.delusion, 0.0);
        assert!(z.is_lucid());
        assert!(!z.is_feral());
    }

    #[test]
    fn new_clamps_max_delusion() {
        let z = Zoanthropy::new(-5.0, 1.5);
        assert!((z.max_delusion - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_inhabit_rate() {
        let z = Zoanthropy::new(100.0, -1.5);
        assert_eq!(z.inhabit_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoanthropy::default();
        assert!((z.max_delusion - 100.0).abs() < 1e-5);
        assert!((z.inhabit_rate - 1.5).abs() < 1e-5);
    }

    // --- inhabit ---

    #[test]
    fn inhabit_adds_delusion() {
        let mut z = z();
        z.inhabit(40.0);
        assert!((z.delusion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn inhabit_clamps_at_max() {
        let mut z = z();
        z.inhabit(200.0);
        assert!((z.delusion - 100.0).abs() < 1e-3);
    }

    #[test]
    fn inhabit_fires_just_feral_at_max() {
        let mut z = z();
        z.inhabit(100.0);
        assert!(z.just_feral);
        assert!(z.is_feral());
    }

    #[test]
    fn inhabit_no_just_feral_when_already_at_max() {
        let mut z = z();
        z.delusion = 100.0;
        z.inhabit(10.0);
        assert!(!z.just_feral);
    }

    #[test]
    fn inhabit_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.inhabit(50.0);
        assert_eq!(z.delusion, 0.0);
    }

    #[test]
    fn inhabit_no_op_when_amount_zero() {
        let mut z = z();
        z.inhabit(0.0);
        assert_eq!(z.delusion, 0.0);
    }

    // --- dispel ---

    #[test]
    fn dispel_reduces_delusion() {
        let mut z = z();
        z.delusion = 60.0;
        z.dispel(20.0);
        assert!((z.delusion - 40.0).abs() < 1e-3);
    }

    #[test]
    fn dispel_clamps_at_zero() {
        let mut z = z();
        z.delusion = 30.0;
        z.dispel(200.0);
        assert_eq!(z.delusion, 0.0);
    }

    #[test]
    fn dispel_fires_just_lucid_at_zero() {
        let mut z = z();
        z.delusion = 30.0;
        z.dispel(30.0);
        assert!(z.just_lucid);
    }

    #[test]
    fn dispel_no_op_when_already_lucid() {
        let mut z = z();
        z.dispel(10.0);
        assert!(!z.just_lucid);
    }

    #[test]
    fn dispel_no_op_when_disabled() {
        let mut z = z();
        z.delusion = 50.0;
        z.enabled = false;
        z.dispel(50.0);
        assert!((z.delusion - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_inhabits_delusion() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.delusion - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_feral_on_inhabit_to_max() {
        let mut z = Zoanthropy::new(100.0, 200.0);
        z.delusion = 95.0;
        z.tick(1.0);
        assert!(z.just_feral);
        assert!(z.is_feral());
    }

    #[test]
    fn tick_no_inhabit_when_already_feral() {
        let mut z = z();
        z.delusion = 100.0;
        z.tick(1.0);
        assert!(!z.just_feral);
    }

    #[test]
    fn tick_no_inhabit_when_rate_zero() {
        let mut z = Zoanthropy::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.delusion, 0.0);
    }

    #[test]
    fn tick_no_inhabit_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.delusion, 0.0);
    }

    #[test]
    fn tick_clears_just_feral() {
        let mut z = Zoanthropy::new(100.0, 200.0);
        z.delusion = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_feral);
    }

    #[test]
    fn tick_clears_just_lucid() {
        let mut z = z();
        z.delusion = 10.0;
        z.dispel(10.0);
        z.tick(0.016);
        assert!(!z.just_lucid);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.delusion - 9.0).abs() < 1e-3);
    }

    // --- is_feral / is_lucid ---

    #[test]
    fn is_feral_false_when_disabled() {
        let mut z = z();
        z.delusion = 100.0;
        z.enabled = false;
        assert!(!z.is_feral());
    }

    #[test]
    fn is_lucid_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_lucid());
    }

    // --- delusion_fraction / effective_animism ---

    #[test]
    fn delusion_fraction_zero_when_lucid() {
        assert_eq!(z().delusion_fraction(), 0.0);
    }

    #[test]
    fn delusion_fraction_half_at_midpoint() {
        let mut z = z();
        z.delusion = 50.0;
        assert!((z.delusion_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_animism_zero_when_lucid() {
        assert_eq!(z().effective_animism(100.0), 0.0);
    }

    #[test]
    fn effective_animism_scales_with_delusion() {
        let mut z = z();
        z.delusion = 75.0;
        assert!((z.effective_animism(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_animism_zero_when_disabled() {
        let mut z = z();
        z.delusion = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_animism(100.0), 0.0);
    }
}
