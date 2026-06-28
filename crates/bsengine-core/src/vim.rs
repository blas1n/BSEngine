use bevy_ecs::prelude::Component;

/// Robust-energy accumulation tracker named after vim, the noun
/// meaning robust energy and enthusiasm — a word that arrived in
/// American English in the mid-nineteenth century with uncertain
/// etymology, possibly from the Latin accusative vim (force,
/// energy, vigour), possibly from an independent formation on the
/// model of vigor, but certainly carrying from its first appearance
/// the sense of a quality that is more physical than intellectual,
/// more animal than refined, more kinetic than deliberative. Vim
/// is the energy of the person who springs out of bed already
/// planning the morning's work, who attacks a project with a
/// directness that bypasses the usual hesitations, who fills a
/// room with a kind of crackling operational readiness that makes
/// other people feel simultaneously energised and slightly exhausted
/// by proximity. It is related to vigour and vitality and vivacity
/// but slightly more physical and less decorous than any of them:
/// vim is what you feel in the limbs before a race, in the hands
/// before a fight, in the chest before a day that is going to
/// require everything you have. It entered English in an era of
/// muscular optimism about human energy and its applications —
/// the mid-Victorian period that gave the world the gymnasium
/// and the health movement and the conviction that the body was a
/// machine that could be improved by directed effort — and it
/// retained a slightly athletic flavour through the twentieth
/// century, appearing most comfortably in sporting contexts and in
/// descriptions of energetic older people whose continued vigour
/// is a source of mild social wonder. In game mechanics, vim is
/// the cleanest model for a general drive stat that fills with
/// rest, food, motivation, and inspiration and depletes under
/// physical exertion, stress, injury, and despair. `drive` builds
/// via `invigorate(amount)` and accumulates passively at
/// `vigor_rate` per second in `tick(dt)` or is drained via
/// `sap(amount)`.
///
/// Models robust-energy fill levels, drive-saturation bars,
/// enthusiasm-accumulation trackers, athletic-readiness gauges,
/// physical-vitality fill levels, animal-energy saturation
/// indicators, operational-readiness accumulation bars, morning-
/// energy meters, motivational-force fill levels, or any mechanic
/// where a character, faction, or creature must maintain the
/// reserve of physical and psychological energy that makes decisive
/// action possible — building up during periods of rest and
/// recovery and draining under the cumulative pressure of exertion,
/// stress, and the ordinary erosion of sustained effort.
///
/// `invigorate(amount)` adds drive; fires `just_energized` when
/// first reaching `max_drive`. No-op when disabled.
///
/// `sap(amount)` reduces drive immediately; fires `just_sapped`
/// when reaching 0. No-op when disabled or already sapped.
///
/// `tick(dt)` clears both flags, then increases drive by
/// `vigor_rate * dt` (capped at `max_drive`). Fires `just_energized`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_energized()` returns `drive >= max_drive && enabled`.
///
/// `is_sapped()` returns `drive == 0.0` (not gated by `enabled`).
///
/// `drive_fraction()` returns `(drive / max_drive).clamp(0, 1)`.
///
/// `effective_vim(scale)` returns `scale * drive_fraction()`
/// when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — invigorates at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Vim {
    pub drive: f32,
    pub max_drive: f32,
    pub vigor_rate: f32,
    pub just_energized: bool,
    pub just_sapped: bool,
    pub enabled: bool,
}

impl Vim {
    pub fn new(max_drive: f32, vigor_rate: f32) -> Self {
        Self {
            drive: 0.0,
            max_drive: max_drive.max(0.1),
            vigor_rate: vigor_rate.max(0.0),
            just_energized: false,
            just_sapped: false,
            enabled: true,
        }
    }

    /// Add drive; fires `just_energized` when first reaching max.
    /// No-op when disabled.
    pub fn invigorate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.drive < self.max_drive;
        self.drive = (self.drive + amount).min(self.max_drive);
        if was_below && self.drive >= self.max_drive {
            self.just_energized = true;
        }
    }

    /// Reduce drive; fires `just_sapped` when reaching 0.
    /// No-op when disabled or already sapped.
    pub fn sap(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.drive <= 0.0 {
            return;
        }
        self.drive = (self.drive - amount).max(0.0);
        if self.drive <= 0.0 {
            self.just_sapped = true;
        }
    }

    /// Clear flags, then increase drive by `vigor_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_energized = false;
        self.just_sapped = false;
        if self.enabled && self.vigor_rate > 0.0 && self.drive < self.max_drive {
            let was_below = self.drive < self.max_drive;
            self.drive = (self.drive + self.vigor_rate * dt).min(self.max_drive);
            if was_below && self.drive >= self.max_drive {
                self.just_energized = true;
            }
        }
    }

    /// `true` when drive is at maximum and component is enabled.
    pub fn is_energized(&self) -> bool {
        self.drive >= self.max_drive && self.enabled
    }

    /// `true` when drive is 0 (not gated by `enabled`).
    pub fn is_sapped(&self) -> bool {
        self.drive == 0.0
    }

    /// Fraction of maximum drive [0.0, 1.0].
    pub fn drive_fraction(&self) -> f32 {
        (self.drive / self.max_drive).clamp(0.0, 1.0)
    }

    /// Returns `scale * drive_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vim(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.drive_fraction()
    }
}

impl Default for Vim {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v() -> Vim {
        Vim::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_sapped() {
        let v = v();
        assert_eq!(v.drive, 0.0);
        assert!(v.is_sapped());
        assert!(!v.is_energized());
    }

    #[test]
    fn new_clamps_max_drive() {
        let v = Vim::new(-5.0, 1.5);
        assert!((v.max_drive - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_vigor_rate() {
        let v = Vim::new(100.0, -1.5);
        assert_eq!(v.vigor_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let v = Vim::default();
        assert!((v.max_drive - 100.0).abs() < 1e-5);
        assert!((v.vigor_rate - 1.5).abs() < 1e-5);
    }

    // --- invigorate ---

    #[test]
    fn invigorate_adds_drive() {
        let mut v = v();
        v.invigorate(40.0);
        assert!((v.drive - 40.0).abs() < 1e-3);
    }

    #[test]
    fn invigorate_clamps_at_max() {
        let mut v = v();
        v.invigorate(200.0);
        assert!((v.drive - 100.0).abs() < 1e-3);
    }

    #[test]
    fn invigorate_fires_just_energized_at_max() {
        let mut v = v();
        v.invigorate(100.0);
        assert!(v.just_energized);
        assert!(v.is_energized());
    }

    #[test]
    fn invigorate_no_just_energized_when_already_at_max() {
        let mut v = v();
        v.drive = 100.0;
        v.invigorate(10.0);
        assert!(!v.just_energized);
    }

    #[test]
    fn invigorate_no_op_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.invigorate(50.0);
        assert_eq!(v.drive, 0.0);
    }

    #[test]
    fn invigorate_no_op_when_amount_zero() {
        let mut v = v();
        v.invigorate(0.0);
        assert_eq!(v.drive, 0.0);
    }

    // --- sap ---

    #[test]
    fn sap_reduces_drive() {
        let mut v = v();
        v.drive = 60.0;
        v.sap(20.0);
        assert!((v.drive - 40.0).abs() < 1e-3);
    }

    #[test]
    fn sap_clamps_at_zero() {
        let mut v = v();
        v.drive = 30.0;
        v.sap(200.0);
        assert_eq!(v.drive, 0.0);
    }

    #[test]
    fn sap_fires_just_sapped_at_zero() {
        let mut v = v();
        v.drive = 30.0;
        v.sap(30.0);
        assert!(v.just_sapped);
    }

    #[test]
    fn sap_no_op_when_already_sapped() {
        let mut v = v();
        v.sap(10.0);
        assert!(!v.just_sapped);
    }

    #[test]
    fn sap_no_op_when_disabled() {
        let mut v = v();
        v.drive = 50.0;
        v.enabled = false;
        v.sap(50.0);
        assert!((v.drive - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_builds_drive() {
        let mut v = v(); // rate=1.5
        v.tick(4.0); // 0 + 1.5*4 = 6
        assert!((v.drive - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_energized_on_drive_to_max() {
        let mut v = Vim::new(100.0, 200.0);
        v.drive = 95.0;
        v.tick(1.0);
        assert!(v.just_energized);
        assert!(v.is_energized());
    }

    #[test]
    fn tick_no_build_when_already_energized() {
        let mut v = v();
        v.drive = 100.0;
        v.tick(1.0);
        assert!(!v.just_energized);
    }

    #[test]
    fn tick_no_build_when_rate_zero() {
        let mut v = Vim::new(100.0, 0.0);
        v.tick(100.0);
        assert_eq!(v.drive, 0.0);
    }

    #[test]
    fn tick_no_build_when_disabled() {
        let mut v = v();
        v.enabled = false;
        v.tick(1.0);
        assert_eq!(v.drive, 0.0);
    }

    #[test]
    fn tick_clears_just_energized() {
        let mut v = Vim::new(100.0, 200.0);
        v.drive = 95.0;
        v.tick(1.0);
        v.tick(0.016);
        assert!(!v.just_energized);
    }

    #[test]
    fn tick_clears_just_sapped() {
        let mut v = v();
        v.drive = 10.0;
        v.sap(10.0);
        v.tick(0.016);
        assert!(!v.just_sapped);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut v = v(); // rate=1.5
        v.tick(6.0); // 1.5*6 = 9
        assert!((v.drive - 9.0).abs() < 1e-3);
    }

    // --- is_energized / is_sapped ---

    #[test]
    fn is_energized_false_when_disabled() {
        let mut v = v();
        v.drive = 100.0;
        v.enabled = false;
        assert!(!v.is_energized());
    }

    #[test]
    fn is_sapped_not_gated_by_enabled() {
        let mut v = v();
        v.enabled = false;
        assert!(v.is_sapped());
    }

    // --- drive_fraction / effective_vim ---

    #[test]
    fn drive_fraction_zero_when_sapped() {
        assert_eq!(v().drive_fraction(), 0.0);
    }

    #[test]
    fn drive_fraction_half_at_midpoint() {
        let mut v = v();
        v.drive = 50.0;
        assert!((v.drive_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vim_zero_when_sapped() {
        assert_eq!(v().effective_vim(100.0), 0.0);
    }

    #[test]
    fn effective_vim_scales_with_drive() {
        let mut v = v();
        v.drive = 75.0;
        assert!((v.effective_vim(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vim_zero_when_disabled() {
        let mut v = v();
        v.drive = 50.0;
        v.enabled = false;
        assert_eq!(v.effective_vim(100.0), 0.0);
    }
}
