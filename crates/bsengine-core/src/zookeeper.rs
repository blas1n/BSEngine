use bevy_ecs::prelude::Component;

/// Animal-care accumulation tracker named after the zookeeper, the
/// person responsible for the daily care and welfare of animals held
/// in a zoological garden. The role is older than the word: keepers
/// were employed at the Tower of London's royal menagerie from the
/// thirteenth century onward, and the Aztec emperor Moctezuma II
/// maintained what visitors reported as the most elaborate animal
/// collection in the world, staffed by hundreds of attendants
/// responsible for feeding, cleaning, and medicating its residents.
/// The modern professional zookeeper combines husbandry skill — the
/// ability to read an animal's health, appetite, and social behaviour
/// at a glance — with behavioural enrichment expertise, population-
/// management record-keeping, and often a degree in zoology or animal
/// science. The job demands consistency: animals in captivity develop
/// strong expectations around feeding times, exhibit routines, and
/// keeper personalities, and disruption of those rhythms produces
/// measurable stress responses — elevated cortisol, stereotypic
/// behaviour, reproductive failure. A keeper who builds trust with
/// a large carnivore over years has created something irreplaceable:
/// the animal will approach voluntarily for health checks that would
/// otherwise require dangerous chemical immobilisation. `care` builds
/// via `tend(amount)` and accumulates passively at `tend_rate` per
/// second in `tick(dt)` or declines via `neglect(amount)`.
///
/// Models animal-care fill levels, zoo-welfare saturation bars,
/// keeper-trust accumulation trackers, husbandry-quality gauges,
/// enrichment-programme fill levels, captive-population-health
/// saturation indicators, daily-routine adherence accumulation bars,
/// veterinary-compliance fill levels, keeper-bond formation meters,
/// or any mechanic where patient daily attendance slowly builds an
/// animal's trust, health, and willingness to cooperate until every
/// creature in the collection is thriving — and where neglect,
/// keeper turnover, or insufficient staffing causes that hard-won
/// condition to collapse back into the suspicion and distress of a
/// wild animal that has never learned to trust a human.
///
/// `tend(amount)` adds care; fires `just_thriving` when first
/// reaching `max_care`. No-op when disabled.
///
/// `neglect(amount)` reduces care immediately; fires `just_neglected`
/// when reaching 0. No-op when disabled or already neglected.
///
/// `tick(dt)` clears both flags, then increases care by
/// `tend_rate * dt` (capped at `max_care`). Fires `just_thriving`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_thriving()` returns `care >= max_care && enabled`.
///
/// `is_neglected()` returns `care == 0.0` (not gated by `enabled`).
///
/// `care_fraction()` returns `(care / max_care).clamp(0, 1)`.
///
/// `effective_husbandry(scale)` returns `scale * care_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — tends at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zookeeper {
    pub care: f32,
    pub max_care: f32,
    pub tend_rate: f32,
    pub just_thriving: bool,
    pub just_neglected: bool,
    pub enabled: bool,
}

impl Zookeeper {
    pub fn new(max_care: f32, tend_rate: f32) -> Self {
        Self {
            care: 0.0,
            max_care: max_care.max(0.1),
            tend_rate: tend_rate.max(0.0),
            just_thriving: false,
            just_neglected: false,
            enabled: true,
        }
    }

    /// Add care; fires `just_thriving` when first reaching max.
    /// No-op when disabled.
    pub fn tend(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.care < self.max_care;
        self.care = (self.care + amount).min(self.max_care);
        if was_below && self.care >= self.max_care {
            self.just_thriving = true;
        }
    }

    /// Reduce care; fires `just_neglected` when reaching 0.
    /// No-op when disabled or already neglected.
    pub fn neglect(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.care <= 0.0 {
            return;
        }
        self.care = (self.care - amount).max(0.0);
        if self.care <= 0.0 {
            self.just_neglected = true;
        }
    }

    /// Clear flags, then increase care by `tend_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_thriving = false;
        self.just_neglected = false;
        if self.enabled && self.tend_rate > 0.0 && self.care < self.max_care {
            let was_below = self.care < self.max_care;
            self.care = (self.care + self.tend_rate * dt).min(self.max_care);
            if was_below && self.care >= self.max_care {
                self.just_thriving = true;
            }
        }
    }

    /// `true` when care is at maximum and component is enabled.
    pub fn is_thriving(&self) -> bool {
        self.care >= self.max_care && self.enabled
    }

    /// `true` when care is 0 (not gated by `enabled`).
    pub fn is_neglected(&self) -> bool {
        self.care == 0.0
    }

    /// Fraction of maximum care [0.0, 1.0].
    pub fn care_fraction(&self) -> f32 {
        (self.care / self.max_care).clamp(0.0, 1.0)
    }

    /// Returns `scale * care_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_husbandry(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.care_fraction()
    }
}

impl Default for Zookeeper {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zookeeper {
        Zookeeper::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_neglected() {
        let z = z();
        assert_eq!(z.care, 0.0);
        assert!(z.is_neglected());
        assert!(!z.is_thriving());
    }

    #[test]
    fn new_clamps_max_care() {
        let z = Zookeeper::new(-5.0, 1.5);
        assert!((z.max_care - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_tend_rate() {
        let z = Zookeeper::new(100.0, -1.5);
        assert_eq!(z.tend_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zookeeper::default();
        assert!((z.max_care - 100.0).abs() < 1e-5);
        assert!((z.tend_rate - 1.5).abs() < 1e-5);
    }

    // --- tend ---

    #[test]
    fn tend_adds_care() {
        let mut z = z();
        z.tend(40.0);
        assert!((z.care - 40.0).abs() < 1e-3);
    }

    #[test]
    fn tend_clamps_at_max() {
        let mut z = z();
        z.tend(200.0);
        assert!((z.care - 100.0).abs() < 1e-3);
    }

    #[test]
    fn tend_fires_just_thriving_at_max() {
        let mut z = z();
        z.tend(100.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn tend_no_just_thriving_when_already_at_max() {
        let mut z = z();
        z.care = 100.0;
        z.tend(10.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tend_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tend(50.0);
        assert_eq!(z.care, 0.0);
    }

    #[test]
    fn tend_no_op_when_amount_zero() {
        let mut z = z();
        z.tend(0.0);
        assert_eq!(z.care, 0.0);
    }

    // --- neglect ---

    #[test]
    fn neglect_reduces_care() {
        let mut z = z();
        z.care = 60.0;
        z.neglect(20.0);
        assert!((z.care - 40.0).abs() < 1e-3);
    }

    #[test]
    fn neglect_clamps_at_zero() {
        let mut z = z();
        z.care = 30.0;
        z.neglect(200.0);
        assert_eq!(z.care, 0.0);
    }

    #[test]
    fn neglect_fires_just_neglected_at_zero() {
        let mut z = z();
        z.care = 30.0;
        z.neglect(30.0);
        assert!(z.just_neglected);
    }

    #[test]
    fn neglect_no_op_when_already_neglected() {
        let mut z = z();
        z.neglect(10.0);
        assert!(!z.just_neglected);
    }

    #[test]
    fn neglect_no_op_when_disabled() {
        let mut z = z();
        z.care = 50.0;
        z.enabled = false;
        z.neglect(50.0);
        assert!((z.care - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_tends_care() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.care - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_thriving_on_tend_to_max() {
        let mut z = Zookeeper::new(100.0, 200.0);
        z.care = 95.0;
        z.tick(1.0);
        assert!(z.just_thriving);
        assert!(z.is_thriving());
    }

    #[test]
    fn tick_no_tend_when_already_thriving() {
        let mut z = z();
        z.care = 100.0;
        z.tick(1.0);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_no_tend_when_rate_zero() {
        let mut z = Zookeeper::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.care, 0.0);
    }

    #[test]
    fn tick_no_tend_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.care, 0.0);
    }

    #[test]
    fn tick_clears_just_thriving() {
        let mut z = Zookeeper::new(100.0, 200.0);
        z.care = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_thriving);
    }

    #[test]
    fn tick_clears_just_neglected() {
        let mut z = z();
        z.care = 10.0;
        z.neglect(10.0);
        z.tick(0.016);
        assert!(!z.just_neglected);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.care - 9.0).abs() < 1e-3);
    }

    // --- is_thriving / is_neglected ---

    #[test]
    fn is_thriving_false_when_disabled() {
        let mut z = z();
        z.care = 100.0;
        z.enabled = false;
        assert!(!z.is_thriving());
    }

    #[test]
    fn is_neglected_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_neglected());
    }

    // --- care_fraction / effective_husbandry ---

    #[test]
    fn care_fraction_zero_when_neglected() {
        assert_eq!(z().care_fraction(), 0.0);
    }

    #[test]
    fn care_fraction_half_at_midpoint() {
        let mut z = z();
        z.care = 50.0;
        assert!((z.care_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_husbandry_zero_when_neglected() {
        assert_eq!(z().effective_husbandry(100.0), 0.0);
    }

    #[test]
    fn effective_husbandry_scales_with_care() {
        let mut z = z();
        z.care = 75.0;
        assert!((z.effective_husbandry(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_husbandry_zero_when_disabled() {
        let mut z = z();
        z.care = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_husbandry(100.0), 0.0);
    }
}
