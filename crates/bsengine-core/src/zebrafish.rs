use bevy_ecs::prelude::Component;

/// Optical-transparency accumulation tracker named after the zebrafish
/// (Danio rerio), the small freshwater cyprinid whose horizontal blue
/// and silver stripes give it its common name but whose scientific
/// importance derives almost entirely from a different property: the
/// near-complete optical transparency of its embryo and larva. George
/// Streisinger at the University of Oregon identified in the late
/// 1970s that the zebrafish combined the genetic tractability of
/// Drosophila with vertebrate organ plans, a generation time of three
/// months, the production of hundreds of embryos per spawning pair,
/// and — most valuably for live imaging — a larva through whose clear
/// body wall every organ, blood vessel, and individual neuron can be
/// watched under a compound microscope as it develops in real time.
/// The transparent-skin mutant line casper, carrying homozygous
/// mutations in both the nacre and roy genes, extends that clarity
/// into adulthood, allowing researchers to watch individual melanoma
/// cells seeding distant organs and individual neutrophils crawling
/// toward a wound without sectioning a single tissue. The standard
/// GFP-reporter approach of inserting a fluorescent protein under a
/// tissue-specific promoter works in zebrafish with photons rather
/// than inferences: you can literally watch a gene switch on. By
/// 2010 the zebrafish had displaced the frog as the primary vertebrate
/// model for developmental genetics and small-molecule drug screening,
/// and by 2020 it had become the organism of choice for live in-vivo
/// imaging of neural circuits, tumour biology, and immune-cell
/// dynamics. `transparency` builds via `expose(amount)` and
/// accumulates passively at `develop_rate` per second in `tick(dt)`
/// or clouds via `occlude(amount)`.
///
/// Models tissue-transparency fill levels, optical-clarity saturation
/// bars, live-imaging-readiness accumulation trackers, larval-
/// development progress bars, genetic-reporter signal-strength gauges,
/// in-vivo-visibility fill levels, model-organism fitness saturation
/// bars, fluorescent-label penetration meters, developmental-stage
/// completion trackers, or any mechanic where patient biological
/// development slowly clears a living system to the point where every
/// internal structure is visible from outside — right up until pigment
/// deposits or opacity agents cloud the window and the view goes dark.
///
/// `expose(amount)` adds transparency; fires `just_transparent` when
/// first reaching `max_transparency`. No-op when disabled.
///
/// `occlude(amount)` reduces transparency immediately; fires
/// `just_opaque` when reaching 0. No-op when disabled or already
/// opaque.
///
/// `tick(dt)` clears both flags, then increases transparency by
/// `develop_rate * dt` (capped at `max_transparency`). Fires
/// `just_transparent` when first reaching max. No-op when disabled
/// or rate is 0.
///
/// `is_transparent()` returns
/// `transparency >= max_transparency && enabled`.
///
/// `is_opaque()` returns `transparency == 0.0` (not gated by `enabled`).
///
/// `transparency_fraction()` returns
/// `(transparency / max_transparency).clamp(0, 1)`.
///
/// `effective_visibility(scale)` returns
/// `scale * transparency_fraction()` when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — develops at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zebrafish {
    pub transparency: f32,
    pub max_transparency: f32,
    pub develop_rate: f32,
    pub just_transparent: bool,
    pub just_opaque: bool,
    pub enabled: bool,
}

impl Zebrafish {
    pub fn new(max_transparency: f32, develop_rate: f32) -> Self {
        Self {
            transparency: 0.0,
            max_transparency: max_transparency.max(0.1),
            develop_rate: develop_rate.max(0.0),
            just_transparent: false,
            just_opaque: false,
            enabled: true,
        }
    }

    /// Add transparency; fires `just_transparent` when first reaching max.
    /// No-op when disabled.
    pub fn expose(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.transparency < self.max_transparency;
        self.transparency = (self.transparency + amount).min(self.max_transparency);
        if was_below && self.transparency >= self.max_transparency {
            self.just_transparent = true;
        }
    }

    /// Reduce transparency; fires `just_opaque` when reaching 0.
    /// No-op when disabled or already opaque.
    pub fn occlude(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.transparency <= 0.0 {
            return;
        }
        self.transparency = (self.transparency - amount).max(0.0);
        if self.transparency <= 0.0 {
            self.just_opaque = true;
        }
    }

    /// Clear flags, then increase transparency by `develop_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_transparent = false;
        self.just_opaque = false;
        if self.enabled && self.develop_rate > 0.0 && self.transparency < self.max_transparency {
            let was_below = self.transparency < self.max_transparency;
            self.transparency =
                (self.transparency + self.develop_rate * dt).min(self.max_transparency);
            if was_below && self.transparency >= self.max_transparency {
                self.just_transparent = true;
            }
        }
    }

    /// `true` when transparency is at maximum and component is enabled.
    pub fn is_transparent(&self) -> bool {
        self.transparency >= self.max_transparency && self.enabled
    }

    /// `true` when transparency is 0 (not gated by `enabled`).
    pub fn is_opaque(&self) -> bool {
        self.transparency == 0.0
    }

    /// Fraction of maximum transparency [0.0, 1.0].
    pub fn transparency_fraction(&self) -> f32 {
        (self.transparency / self.max_transparency).clamp(0.0, 1.0)
    }

    /// Returns `scale * transparency_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_visibility(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.transparency_fraction()
    }
}

impl Default for Zebrafish {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zebrafish {
        Zebrafish::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_opaque() {
        let z = z();
        assert_eq!(z.transparency, 0.0);
        assert!(z.is_opaque());
        assert!(!z.is_transparent());
    }

    #[test]
    fn new_clamps_max_transparency() {
        let z = Zebrafish::new(-5.0, 1.5);
        assert!((z.max_transparency - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_develop_rate() {
        let z = Zebrafish::new(100.0, -1.5);
        assert_eq!(z.develop_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zebrafish::default();
        assert!((z.max_transparency - 100.0).abs() < 1e-5);
        assert!((z.develop_rate - 1.5).abs() < 1e-5);
    }

    // --- expose ---

    #[test]
    fn expose_adds_transparency() {
        let mut z = z();
        z.expose(40.0);
        assert!((z.transparency - 40.0).abs() < 1e-3);
    }

    #[test]
    fn expose_clamps_at_max() {
        let mut z = z();
        z.expose(200.0);
        assert!((z.transparency - 100.0).abs() < 1e-3);
    }

    #[test]
    fn expose_fires_just_transparent_at_max() {
        let mut z = z();
        z.expose(100.0);
        assert!(z.just_transparent);
        assert!(z.is_transparent());
    }

    #[test]
    fn expose_no_just_transparent_when_already_at_max() {
        let mut z = z();
        z.transparency = 100.0;
        z.expose(10.0);
        assert!(!z.just_transparent);
    }

    #[test]
    fn expose_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.expose(50.0);
        assert_eq!(z.transparency, 0.0);
    }

    #[test]
    fn expose_no_op_when_amount_zero() {
        let mut z = z();
        z.expose(0.0);
        assert_eq!(z.transparency, 0.0);
    }

    // --- occlude ---

    #[test]
    fn occlude_reduces_transparency() {
        let mut z = z();
        z.transparency = 60.0;
        z.occlude(20.0);
        assert!((z.transparency - 40.0).abs() < 1e-3);
    }

    #[test]
    fn occlude_clamps_at_zero() {
        let mut z = z();
        z.transparency = 30.0;
        z.occlude(200.0);
        assert_eq!(z.transparency, 0.0);
    }

    #[test]
    fn occlude_fires_just_opaque_at_zero() {
        let mut z = z();
        z.transparency = 30.0;
        z.occlude(30.0);
        assert!(z.just_opaque);
    }

    #[test]
    fn occlude_no_op_when_already_opaque() {
        let mut z = z();
        z.occlude(10.0);
        assert!(!z.just_opaque);
    }

    #[test]
    fn occlude_no_op_when_disabled() {
        let mut z = z();
        z.transparency = 50.0;
        z.enabled = false;
        z.occlude(50.0);
        assert!((z.transparency - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_develops_transparency() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.transparency - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_transparent_on_develop_to_max() {
        let mut z = Zebrafish::new(100.0, 200.0);
        z.transparency = 95.0;
        z.tick(1.0);
        assert!(z.just_transparent);
        assert!(z.is_transparent());
    }

    #[test]
    fn tick_no_develop_when_already_transparent() {
        let mut z = z();
        z.transparency = 100.0;
        z.tick(1.0);
        assert!(!z.just_transparent);
    }

    #[test]
    fn tick_no_develop_when_rate_zero() {
        let mut z = Zebrafish::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.transparency, 0.0);
    }

    #[test]
    fn tick_no_develop_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.transparency, 0.0);
    }

    #[test]
    fn tick_clears_just_transparent() {
        let mut z = Zebrafish::new(100.0, 200.0);
        z.transparency = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_transparent);
    }

    #[test]
    fn tick_clears_just_opaque() {
        let mut z = z();
        z.transparency = 10.0;
        z.occlude(10.0);
        z.tick(0.016);
        assert!(!z.just_opaque);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.transparency - 9.0).abs() < 1e-3);
    }

    // --- is_transparent / is_opaque ---

    #[test]
    fn is_transparent_false_when_disabled() {
        let mut z = z();
        z.transparency = 100.0;
        z.enabled = false;
        assert!(!z.is_transparent());
    }

    #[test]
    fn is_opaque_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_opaque());
    }

    // --- transparency_fraction / effective_visibility ---

    #[test]
    fn transparency_fraction_zero_when_opaque() {
        assert_eq!(z().transparency_fraction(), 0.0);
    }

    #[test]
    fn transparency_fraction_half_at_midpoint() {
        let mut z = z();
        z.transparency = 50.0;
        assert!((z.transparency_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_visibility_zero_when_opaque() {
        assert_eq!(z().effective_visibility(100.0), 0.0);
    }

    #[test]
    fn effective_visibility_scales_with_transparency() {
        let mut z = z();
        z.transparency = 75.0;
        assert!((z.effective_visibility(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_visibility_zero_when_disabled() {
        let mut z = z();
        z.transparency = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_visibility(100.0), 0.0);
    }
}
