use bevy_ecs::prelude::Component;

/// Animal-form iconography accumulation tracker named after zoomorphic,
/// the adjective describing art, architecture, and religious symbolism
/// that depicts, represents, or embodies animals or animal forms.
/// Zoomorphic design is among the oldest documented human visual
/// traditions: the cave paintings of Chauvet — the oldest known
/// figurative art, dated to approximately 36,000 years ago — are
/// overwhelmingly zoomorphic, representing cave lions, woolly rhinos,
/// and bears with a fidelity of observation that implies decades of
/// sustained looking at living animals. The tradition persisted into
/// every subsequent culture: ancient Egyptian gods wore the heads of
/// ibises, jackals, and falcons; Scythian metalworkers folded panthers
/// and stags into interlocking zoomorphic knots; Hiberno-Saxon
/// manuscript illuminators filled the Book of Kells with serpentine
/// animals so densely interwoven that they become a kind of abstract
/// pattern before they resolve back into recognisable creatures. In
/// heraldry, zoomorphic charges — lions rampant, eagles displayed,
/// dolphins naiant — encode political and moral claims through the
/// symbolic properties attributed to particular animals. In game
/// design, zoomorphic iconography tracks how thoroughly a figure,
/// totem, or artefact has been imbued with animal-form symbolism:
/// from the first rough sketch of a beast motif scratched into clay,
/// through progressive elaboration of horns, scales, and claws, until
/// every surface bears the dense, interlocking animal imagery of a
/// fully realised zoomorphic composition. `iconography` builds via
/// `engrave(amount)` and accumulates passively at `depict_rate` per
/// second in `tick(dt)` or is eroded via `erode(amount)`.
///
/// Models zoomorphic-art fill levels, animal-motif saturation bars,
/// totem-carving completion trackers, heraldic-beast elaboration
/// gauges, manuscript-illumination progress indicators, cave-painting
/// composition fill bars, deity-iconography saturation meters,
/// knotwork-animal density accumulators, ceremonial-mask carving
/// progress bars, or any mechanic where patient artistic attention
/// slowly builds a surface of dense animal imagery until the figure
/// is entirely consumed by its own zoomorphic identity — and where
/// exposure, water, or fire strips those carved lines back to blank
/// stone.
///
/// `engrave(amount)` adds iconography; fires `just_engraved` when
/// first reaching `max_iconography`. No-op when disabled.
///
/// `erode(amount)` reduces iconography immediately; fires `just_eroded`
/// when reaching 0. No-op when disabled or already eroded.
///
/// `tick(dt)` clears both flags, then increases iconography by
/// `depict_rate * dt` (capped at `max_iconography`). Fires
/// `just_engraved` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_engraved()` returns `iconography >= max_iconography && enabled`.
///
/// `is_eroded()` returns `iconography == 0.0` (not gated by `enabled`).
///
/// `iconography_fraction()` returns
/// `(iconography / max_iconography).clamp(0, 1)`.
///
/// `effective_symbolism(scale)` returns
/// `scale * iconography_fraction()` when enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 1.5)` — depicts at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoomorphic {
    pub iconography: f32,
    pub max_iconography: f32,
    pub depict_rate: f32,
    pub just_engraved: bool,
    pub just_eroded: bool,
    pub enabled: bool,
}

impl Zoomorphic {
    pub fn new(max_iconography: f32, depict_rate: f32) -> Self {
        Self {
            iconography: 0.0,
            max_iconography: max_iconography.max(0.1),
            depict_rate: depict_rate.max(0.0),
            just_engraved: false,
            just_eroded: false,
            enabled: true,
        }
    }

    /// Add iconography; fires `just_engraved` when first reaching max.
    /// No-op when disabled.
    pub fn engrave(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.iconography < self.max_iconography;
        self.iconography = (self.iconography + amount).min(self.max_iconography);
        if was_below && self.iconography >= self.max_iconography {
            self.just_engraved = true;
        }
    }

    /// Reduce iconography; fires `just_eroded` when reaching 0.
    /// No-op when disabled or already eroded.
    pub fn erode(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.iconography <= 0.0 {
            return;
        }
        self.iconography = (self.iconography - amount).max(0.0);
        if self.iconography <= 0.0 {
            self.just_eroded = true;
        }
    }

    /// Clear flags, then increase iconography by `depict_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_engraved = false;
        self.just_eroded = false;
        if self.enabled && self.depict_rate > 0.0 && self.iconography < self.max_iconography {
            let was_below = self.iconography < self.max_iconography;
            self.iconography = (self.iconography + self.depict_rate * dt).min(self.max_iconography);
            if was_below && self.iconography >= self.max_iconography {
                self.just_engraved = true;
            }
        }
    }

    /// `true` when iconography is at maximum and component is enabled.
    pub fn is_engraved(&self) -> bool {
        self.iconography >= self.max_iconography && self.enabled
    }

    /// `true` when iconography is 0 (not gated by `enabled`).
    pub fn is_eroded(&self) -> bool {
        self.iconography == 0.0
    }

    /// Fraction of maximum iconography [0.0, 1.0].
    pub fn iconography_fraction(&self) -> f32 {
        (self.iconography / self.max_iconography).clamp(0.0, 1.0)
    }

    /// Returns `scale * iconography_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_symbolism(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.iconography_fraction()
    }
}

impl Default for Zoomorphic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoomorphic {
        Zoomorphic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_eroded() {
        let z = z();
        assert_eq!(z.iconography, 0.0);
        assert!(z.is_eroded());
        assert!(!z.is_engraved());
    }

    #[test]
    fn new_clamps_max_iconography() {
        let z = Zoomorphic::new(-5.0, 1.5);
        assert!((z.max_iconography - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_depict_rate() {
        let z = Zoomorphic::new(100.0, -1.5);
        assert_eq!(z.depict_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoomorphic::default();
        assert!((z.max_iconography - 100.0).abs() < 1e-5);
        assert!((z.depict_rate - 1.5).abs() < 1e-5);
    }

    // --- engrave ---

    #[test]
    fn engrave_adds_iconography() {
        let mut z = z();
        z.engrave(40.0);
        assert!((z.iconography - 40.0).abs() < 1e-3);
    }

    #[test]
    fn engrave_clamps_at_max() {
        let mut z = z();
        z.engrave(200.0);
        assert!((z.iconography - 100.0).abs() < 1e-3);
    }

    #[test]
    fn engrave_fires_just_engraved_at_max() {
        let mut z = z();
        z.engrave(100.0);
        assert!(z.just_engraved);
        assert!(z.is_engraved());
    }

    #[test]
    fn engrave_no_just_engraved_when_already_at_max() {
        let mut z = z();
        z.iconography = 100.0;
        z.engrave(10.0);
        assert!(!z.just_engraved);
    }

    #[test]
    fn engrave_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.engrave(50.0);
        assert_eq!(z.iconography, 0.0);
    }

    #[test]
    fn engrave_no_op_when_amount_zero() {
        let mut z = z();
        z.engrave(0.0);
        assert_eq!(z.iconography, 0.0);
    }

    // --- erode ---

    #[test]
    fn erode_reduces_iconography() {
        let mut z = z();
        z.iconography = 60.0;
        z.erode(20.0);
        assert!((z.iconography - 40.0).abs() < 1e-3);
    }

    #[test]
    fn erode_clamps_at_zero() {
        let mut z = z();
        z.iconography = 30.0;
        z.erode(200.0);
        assert_eq!(z.iconography, 0.0);
    }

    #[test]
    fn erode_fires_just_eroded_at_zero() {
        let mut z = z();
        z.iconography = 30.0;
        z.erode(30.0);
        assert!(z.just_eroded);
    }

    #[test]
    fn erode_no_op_when_already_eroded() {
        let mut z = z();
        z.erode(10.0);
        assert!(!z.just_eroded);
    }

    #[test]
    fn erode_no_op_when_disabled() {
        let mut z = z();
        z.iconography = 50.0;
        z.enabled = false;
        z.erode(50.0);
        assert!((z.iconography - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_depicts_iconography() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.iconography - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_engraved_on_depict_to_max() {
        let mut z = Zoomorphic::new(100.0, 200.0);
        z.iconography = 95.0;
        z.tick(1.0);
        assert!(z.just_engraved);
        assert!(z.is_engraved());
    }

    #[test]
    fn tick_no_depict_when_already_engraved() {
        let mut z = z();
        z.iconography = 100.0;
        z.tick(1.0);
        assert!(!z.just_engraved);
    }

    #[test]
    fn tick_no_depict_when_rate_zero() {
        let mut z = Zoomorphic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.iconography, 0.0);
    }

    #[test]
    fn tick_no_depict_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.iconography, 0.0);
    }

    #[test]
    fn tick_clears_just_engraved() {
        let mut z = Zoomorphic::new(100.0, 200.0);
        z.iconography = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_engraved);
    }

    #[test]
    fn tick_clears_just_eroded() {
        let mut z = z();
        z.iconography = 10.0;
        z.erode(10.0);
        z.tick(0.016);
        assert!(!z.just_eroded);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.iconography - 9.0).abs() < 1e-3);
    }

    // --- is_engraved / is_eroded ---

    #[test]
    fn is_engraved_false_when_disabled() {
        let mut z = z();
        z.iconography = 100.0;
        z.enabled = false;
        assert!(!z.is_engraved());
    }

    #[test]
    fn is_eroded_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_eroded());
    }

    // --- iconography_fraction / effective_symbolism ---

    #[test]
    fn iconography_fraction_zero_when_eroded() {
        assert_eq!(z().iconography_fraction(), 0.0);
    }

    #[test]
    fn iconography_fraction_half_at_midpoint() {
        let mut z = z();
        z.iconography = 50.0;
        assert!((z.iconography_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_symbolism_zero_when_eroded() {
        assert_eq!(z().effective_symbolism(100.0), 0.0);
    }

    #[test]
    fn effective_symbolism_scales_with_iconography() {
        let mut z = z();
        z.iconography = 75.0;
        assert!((z.effective_symbolism(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_symbolism_zero_when_disabled() {
        let mut z = z();
        z.iconography = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_symbolism(100.0), 0.0);
    }
}
