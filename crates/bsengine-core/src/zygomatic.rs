use bevy_ecs::prelude::Component;

/// Facial-bone articulation tracker named after "zygomatic", the
/// adjective describing structures of or relating to the zygomatic
/// bone — the malar bone whose four articular surfaces lock it into
/// the surrounding facial skeleton at the frontozygomatic suture
/// (where it meets the frontal bone above the orbit), the
/// zygomaticomaxillary suture (where it meets the maxilla below the
/// orbit), the zygomaticotemporal suture (where the temporal process
/// of the zygomatic meets the zygomatic process of the temporal bone
/// to complete the zygomatic arch), and the zygomaticosphenoidal
/// suture (where it meets the greater wing of the sphenoid at the
/// lateral orbital wall). These four sutures are the anchors that
/// make the cheekbone a structural keystone of the middle third of
/// the face: they distribute the compressive forces of mastication,
/// protect the orbit laterally, and define the width of the face as
/// seen from the front. In clinical terms, a zygomatic fracture is
/// really a "tetrapod" fracture — it must separate at three or four
/// of those sutures simultaneously for the bone to displace. As a
/// living structure the suture surfaces remodel continuously in
/// response to mechanical load; `articulation` builds via
/// `reinforce(amount)` and accumulates passively at `fuse_rate` per
/// second in `tick(dt)` or weakens via `separate(amount)`.
///
/// Models facial-suture integrity fill levels, mid-face structural-
/// anchor saturation bars, orbital-wall articulation fill levels,
/// zygomatic-tetrapod fracture-resistance trackers, cranio-facial
/// integration gauges, prosthetic-implant osseointegration meters,
/// suture-remodelling accumulation bars, Le Fort-zone structural-
/// continuity saturation trackers, maxillofacial reconstruction
/// completion indicators, or any mechanic where four separate
/// articulation points must all be reinforced simultaneously before
/// the central bone becomes fully integrated into a load-bearing
/// facial architecture.
///
/// `reinforce(amount)` adds articulation; fires `just_fused` when
/// first reaching `max_articulation`. No-op when disabled.
///
/// `separate(amount)` reduces articulation immediately; fires
/// `just_separated` when reaching 0. No-op when disabled or already
/// separated.
///
/// `tick(dt)` clears both flags, then increases articulation by
/// `fuse_rate * dt` (capped at `max_articulation`). Fires
/// `just_fused` when first reaching max. No-op when disabled or
/// rate is 0.
///
/// `is_fused()` returns `articulation >= max_articulation && enabled`.
///
/// `is_separated()` returns `articulation == 0.0` (not gated by
/// `enabled`).
///
/// `articulation_fraction()` returns
/// `(articulation / max_articulation).clamp(0, 1)`.
///
/// `effective_anchorage(scale)` returns
/// `scale * articulation_fraction()` when enabled; `0.0` when
/// disabled.
///
/// Default: `new(100.0, 1.5)` — fuses at 1.5 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zygomatic {
    pub articulation: f32,
    pub max_articulation: f32,
    pub fuse_rate: f32,
    pub just_fused: bool,
    pub just_separated: bool,
    pub enabled: bool,
}

impl Zygomatic {
    pub fn new(max_articulation: f32, fuse_rate: f32) -> Self {
        Self {
            articulation: 0.0,
            max_articulation: max_articulation.max(0.1),
            fuse_rate: fuse_rate.max(0.0),
            just_fused: false,
            just_separated: false,
            enabled: true,
        }
    }

    /// Add articulation; fires `just_fused` when first reaching max.
    /// No-op when disabled.
    pub fn reinforce(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.articulation < self.max_articulation;
        self.articulation = (self.articulation + amount).min(self.max_articulation);
        if was_below && self.articulation >= self.max_articulation {
            self.just_fused = true;
        }
    }

    /// Reduce articulation; fires `just_separated` when reaching 0.
    /// No-op when disabled or already separated.
    pub fn separate(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.articulation <= 0.0 {
            return;
        }
        self.articulation = (self.articulation - amount).max(0.0);
        if self.articulation <= 0.0 {
            self.just_separated = true;
        }
    }

    /// Clear flags, then increase articulation by `fuse_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_fused = false;
        self.just_separated = false;
        if self.enabled && self.fuse_rate > 0.0 && self.articulation < self.max_articulation {
            let was_below = self.articulation < self.max_articulation;
            self.articulation =
                (self.articulation + self.fuse_rate * dt).min(self.max_articulation);
            if was_below && self.articulation >= self.max_articulation {
                self.just_fused = true;
            }
        }
    }

    /// `true` when articulation is at maximum and component is enabled.
    pub fn is_fused(&self) -> bool {
        self.articulation >= self.max_articulation && self.enabled
    }

    /// `true` when articulation is 0 (not gated by `enabled`).
    pub fn is_separated(&self) -> bool {
        self.articulation == 0.0
    }

    /// Fraction of maximum articulation [0.0, 1.0].
    pub fn articulation_fraction(&self) -> f32 {
        (self.articulation / self.max_articulation).clamp(0.0, 1.0)
    }

    /// Returns `scale * articulation_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_anchorage(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.articulation_fraction()
    }
}

impl Default for Zygomatic {
    fn default() -> Self {
        Self::new(100.0, 1.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zygomatic {
        Zygomatic::new(100.0, 1.5)
    }

    // --- construction ---

    #[test]
    fn new_starts_separated() {
        let z = z();
        assert_eq!(z.articulation, 0.0);
        assert!(z.is_separated());
        assert!(!z.is_fused());
    }

    #[test]
    fn new_clamps_max_articulation() {
        let z = Zygomatic::new(-5.0, 1.5);
        assert!((z.max_articulation - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_fuse_rate() {
        let z = Zygomatic::new(100.0, -1.5);
        assert_eq!(z.fuse_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zygomatic::default();
        assert!((z.max_articulation - 100.0).abs() < 1e-5);
        assert!((z.fuse_rate - 1.5).abs() < 1e-5);
    }

    // --- reinforce ---

    #[test]
    fn reinforce_adds_articulation() {
        let mut z = z();
        z.reinforce(40.0);
        assert!((z.articulation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn reinforce_clamps_at_max() {
        let mut z = z();
        z.reinforce(200.0);
        assert!((z.articulation - 100.0).abs() < 1e-3);
    }

    #[test]
    fn reinforce_fires_just_fused_at_max() {
        let mut z = z();
        z.reinforce(100.0);
        assert!(z.just_fused);
        assert!(z.is_fused());
    }

    #[test]
    fn reinforce_no_just_fused_when_already_at_max() {
        let mut z = z();
        z.articulation = 100.0;
        z.reinforce(10.0);
        assert!(!z.just_fused);
    }

    #[test]
    fn reinforce_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.reinforce(50.0);
        assert_eq!(z.articulation, 0.0);
    }

    #[test]
    fn reinforce_no_op_when_amount_zero() {
        let mut z = z();
        z.reinforce(0.0);
        assert_eq!(z.articulation, 0.0);
    }

    // --- separate ---

    #[test]
    fn separate_reduces_articulation() {
        let mut z = z();
        z.articulation = 60.0;
        z.separate(20.0);
        assert!((z.articulation - 40.0).abs() < 1e-3);
    }

    #[test]
    fn separate_clamps_at_zero() {
        let mut z = z();
        z.articulation = 30.0;
        z.separate(200.0);
        assert_eq!(z.articulation, 0.0);
    }

    #[test]
    fn separate_fires_just_separated_at_zero() {
        let mut z = z();
        z.articulation = 30.0;
        z.separate(30.0);
        assert!(z.just_separated);
    }

    #[test]
    fn separate_no_op_when_already_separated() {
        let mut z = z();
        z.separate(10.0);
        assert!(!z.just_separated);
    }

    #[test]
    fn separate_no_op_when_disabled() {
        let mut z = z();
        z.articulation = 50.0;
        z.enabled = false;
        z.separate(50.0);
        assert!((z.articulation - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_fuses_articulation() {
        let mut z = z(); // rate=1.5
        z.tick(4.0); // 0 + 1.5*4 = 6
        assert!((z.articulation - 6.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_fused_on_fuse_to_max() {
        let mut z = Zygomatic::new(100.0, 200.0);
        z.articulation = 95.0;
        z.tick(1.0);
        assert!(z.just_fused);
        assert!(z.is_fused());
    }

    #[test]
    fn tick_no_fuse_when_already_fused() {
        let mut z = z();
        z.articulation = 100.0;
        z.tick(1.0);
        assert!(!z.just_fused);
    }

    #[test]
    fn tick_no_fuse_when_rate_zero() {
        let mut z = Zygomatic::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.articulation, 0.0);
    }

    #[test]
    fn tick_no_fuse_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.articulation, 0.0);
    }

    #[test]
    fn tick_clears_just_fused() {
        let mut z = Zygomatic::new(100.0, 200.0);
        z.articulation = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_fused);
    }

    #[test]
    fn tick_clears_just_separated() {
        let mut z = z();
        z.articulation = 10.0;
        z.separate(10.0);
        z.tick(0.016);
        assert!(!z.just_separated);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=1.5
        z.tick(6.0); // 1.5*6 = 9
        assert!((z.articulation - 9.0).abs() < 1e-3);
    }

    // --- is_fused / is_separated ---

    #[test]
    fn is_fused_false_when_disabled() {
        let mut z = z();
        z.articulation = 100.0;
        z.enabled = false;
        assert!(!z.is_fused());
    }

    #[test]
    fn is_separated_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_separated());
    }

    // --- articulation_fraction / effective_anchorage ---

    #[test]
    fn articulation_fraction_zero_when_separated() {
        assert_eq!(z().articulation_fraction(), 0.0);
    }

    #[test]
    fn articulation_fraction_half_at_midpoint() {
        let mut z = z();
        z.articulation = 50.0;
        assert!((z.articulation_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_anchorage_zero_when_separated() {
        assert_eq!(z().effective_anchorage(100.0), 0.0);
    }

    #[test]
    fn effective_anchorage_scales_with_articulation() {
        let mut z = z();
        z.articulation = 75.0;
        assert!((z.effective_anchorage(100.0) - 75.0).abs() < 1e-3);
    }

    #[test]
    fn effective_anchorage_zero_when_disabled() {
        let mut z = z();
        z.articulation = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_anchorage(100.0), 0.0);
    }
}
