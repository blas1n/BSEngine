use bevy_ecs::prelude::Component;

/// Larval-molt tracker. `molt` builds via `grow(amount)` and advances
/// passively at `grow_rate` per second in `tick(dt)` or is shed
/// immediately via `shed(amount)`.
///
/// Models crustacean larval-stage meters, metamorphosis progress bars,
/// chrysalis-formation fill levels, caterpillar-to-butterfly trackers,
/// tadpole-to-frog gauges, exoskeleton-growth accumulators, insect-instar
/// progression indicators, or any mechanic where a creature builds toward
/// a transformative threshold before shedding its old form.
///
/// `grow(amount)` adds molt; fires `just_metamorphosed` when first
/// reaching `max_molt`. No-op when disabled.
///
/// `shed(amount)` reduces molt immediately; fires `just_shed` when
/// reaching 0. No-op when disabled or already shed.
///
/// `tick(dt)` clears both flags, then increases molt by
/// `grow_rate * dt` (capped at `max_molt`). Fires `just_metamorphosed`
/// when first reaching max. No-op when disabled or rate is 0.
///
/// `is_metamorphosed()` returns `molt >= max_molt && enabled`.
///
/// `is_shed()` returns `molt == 0.0` (not gated by `enabled`).
///
/// `molt_fraction()` returns `(molt / max_molt).clamp(0, 1)`.
///
/// `effective_vigor(scale)` returns `scale * molt_fraction()` when
/// enabled; `0.0` when disabled.
///
/// Default: `new(100.0, 4.0)` — grows at 4 units/sec.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Zoea {
    pub molt: f32,
    pub max_molt: f32,
    pub grow_rate: f32,
    pub just_metamorphosed: bool,
    pub just_shed: bool,
    pub enabled: bool,
}

impl Zoea {
    pub fn new(max_molt: f32, grow_rate: f32) -> Self {
        Self {
            molt: 0.0,
            max_molt: max_molt.max(0.1),
            grow_rate: grow_rate.max(0.0),
            just_metamorphosed: false,
            just_shed: false,
            enabled: true,
        }
    }

    /// Add molt; fires `just_metamorphosed` when first reaching max.
    /// No-op when disabled.
    pub fn grow(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 {
            return;
        }
        let was_below = self.molt < self.max_molt;
        self.molt = (self.molt + amount).min(self.max_molt);
        if was_below && self.molt >= self.max_molt {
            self.just_metamorphosed = true;
        }
    }

    /// Reduce molt; fires `just_shed` when reaching 0.
    /// No-op when disabled or already shed.
    pub fn shed(&mut self, amount: f32) {
        if !self.enabled || amount <= 0.0 || self.molt <= 0.0 {
            return;
        }
        self.molt = (self.molt - amount).max(0.0);
        if self.molt <= 0.0 {
            self.just_shed = true;
        }
    }

    /// Clear flags, then increase molt by `grow_rate * dt`.
    pub fn tick(&mut self, dt: f32) {
        self.just_metamorphosed = false;
        self.just_shed = false;
        if self.enabled && self.grow_rate > 0.0 && self.molt < self.max_molt {
            let was_below = self.molt < self.max_molt;
            self.molt = (self.molt + self.grow_rate * dt).min(self.max_molt);
            if was_below && self.molt >= self.max_molt {
                self.just_metamorphosed = true;
            }
        }
    }

    /// `true` when molt is at maximum and component is enabled.
    pub fn is_metamorphosed(&self) -> bool {
        self.molt >= self.max_molt && self.enabled
    }

    /// `true` when molt is 0 (not gated by `enabled`).
    pub fn is_shed(&self) -> bool {
        self.molt == 0.0
    }

    /// Fraction of maximum molt [0.0, 1.0].
    pub fn molt_fraction(&self) -> f32 {
        (self.molt / self.max_molt).clamp(0.0, 1.0)
    }

    /// Returns `scale * molt_fraction()` when enabled; `0.0` when disabled.
    pub fn effective_vigor(&self, scale: f32) -> f32 {
        if !self.enabled {
            return 0.0;
        }
        scale * self.molt_fraction()
    }
}

impl Default for Zoea {
    fn default() -> Self {
        Self::new(100.0, 4.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn z() -> Zoea {
        Zoea::new(100.0, 4.0)
    }

    // --- construction ---

    #[test]
    fn new_starts_shed() {
        let z = z();
        assert_eq!(z.molt, 0.0);
        assert!(z.is_shed());
        assert!(!z.is_metamorphosed());
    }

    #[test]
    fn new_clamps_max_molt() {
        let z = Zoea::new(-5.0, 4.0);
        assert!((z.max_molt - 0.1).abs() < 1e-5);
    }

    #[test]
    fn new_clamps_grow_rate() {
        let z = Zoea::new(100.0, -3.0);
        assert_eq!(z.grow_rate, 0.0);
    }

    #[test]
    fn default_values() {
        let z = Zoea::default();
        assert!((z.max_molt - 100.0).abs() < 1e-5);
        assert!((z.grow_rate - 4.0).abs() < 1e-5);
    }

    // --- grow ---

    #[test]
    fn grow_adds_molt() {
        let mut z = z();
        z.grow(40.0);
        assert!((z.molt - 40.0).abs() < 1e-3);
    }

    #[test]
    fn grow_clamps_at_max() {
        let mut z = z();
        z.grow(200.0);
        assert!((z.molt - 100.0).abs() < 1e-3);
    }

    #[test]
    fn grow_fires_just_metamorphosed_at_max() {
        let mut z = z();
        z.grow(100.0);
        assert!(z.just_metamorphosed);
        assert!(z.is_metamorphosed());
    }

    #[test]
    fn grow_no_just_metamorphosed_when_already_at_max() {
        let mut z = z();
        z.molt = 100.0;
        z.grow(10.0);
        assert!(!z.just_metamorphosed);
    }

    #[test]
    fn grow_no_op_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.grow(50.0);
        assert_eq!(z.molt, 0.0);
    }

    #[test]
    fn grow_no_op_when_amount_zero() {
        let mut z = z();
        z.grow(0.0);
        assert_eq!(z.molt, 0.0);
    }

    // --- shed ---

    #[test]
    fn shed_reduces_molt() {
        let mut z = z();
        z.molt = 60.0;
        z.shed(20.0);
        assert!((z.molt - 40.0).abs() < 1e-3);
    }

    #[test]
    fn shed_clamps_at_zero() {
        let mut z = z();
        z.molt = 30.0;
        z.shed(200.0);
        assert_eq!(z.molt, 0.0);
    }

    #[test]
    fn shed_fires_just_shed_at_zero() {
        let mut z = z();
        z.molt = 30.0;
        z.shed(30.0);
        assert!(z.just_shed);
    }

    #[test]
    fn shed_no_op_when_already_shed() {
        let mut z = z();
        z.shed(10.0);
        assert!(!z.just_shed);
    }

    #[test]
    fn shed_no_op_when_disabled() {
        let mut z = z();
        z.molt = 50.0;
        z.enabled = false;
        z.shed(50.0);
        assert!((z.molt - 50.0).abs() < 1e-3);
    }

    // --- tick ---

    #[test]
    fn tick_grows_molt() {
        let mut z = z(); // rate=4
        z.tick(1.0); // 0 + 4 = 4
        assert!((z.molt - 4.0).abs() < 1e-3);
    }

    #[test]
    fn tick_fires_just_metamorphosed_on_grow_to_max() {
        let mut z = Zoea::new(100.0, 200.0);
        z.molt = 95.0;
        z.tick(1.0);
        assert!(z.just_metamorphosed);
        assert!(z.is_metamorphosed());
    }

    #[test]
    fn tick_no_grow_when_already_metamorphosed() {
        let mut z = z();
        z.molt = 100.0;
        z.tick(1.0);
        assert!(!z.just_metamorphosed);
    }

    #[test]
    fn tick_no_grow_when_rate_zero() {
        let mut z = Zoea::new(100.0, 0.0);
        z.tick(100.0);
        assert_eq!(z.molt, 0.0);
    }

    #[test]
    fn tick_no_grow_when_disabled() {
        let mut z = z();
        z.enabled = false;
        z.tick(1.0);
        assert_eq!(z.molt, 0.0);
    }

    #[test]
    fn tick_clears_just_metamorphosed() {
        let mut z = Zoea::new(100.0, 200.0);
        z.molt = 95.0;
        z.tick(1.0);
        z.tick(0.016);
        assert!(!z.just_metamorphosed);
    }

    #[test]
    fn tick_clears_just_shed() {
        let mut z = z();
        z.molt = 10.0;
        z.shed(10.0);
        z.tick(0.016);
        assert!(!z.just_shed);
    }

    #[test]
    fn tick_scales_with_dt() {
        let mut z = z(); // rate=4
        z.tick(3.0); // 4*3 = 12
        assert!((z.molt - 12.0).abs() < 1e-3);
    }

    // --- is_metamorphosed / is_shed ---

    #[test]
    fn is_metamorphosed_false_when_disabled() {
        let mut z = z();
        z.molt = 100.0;
        z.enabled = false;
        assert!(!z.is_metamorphosed());
    }

    #[test]
    fn is_shed_not_gated_by_enabled() {
        let mut z = z();
        z.enabled = false;
        assert!(z.is_shed());
    }

    // --- molt_fraction / effective_vigor ---

    #[test]
    fn molt_fraction_zero_when_shed() {
        assert_eq!(z().molt_fraction(), 0.0);
    }

    #[test]
    fn molt_fraction_half_at_midpoint() {
        let mut z = z();
        z.molt = 50.0;
        assert!((z.molt_fraction() - 0.5).abs() < 1e-4);
    }

    #[test]
    fn effective_vigor_zero_when_shed() {
        assert_eq!(z().effective_vigor(100.0), 0.0);
    }

    #[test]
    fn effective_vigor_scales_with_molt() {
        let mut z = z();
        z.molt = 65.0;
        assert!((z.effective_vigor(100.0) - 65.0).abs() < 1e-3);
    }

    #[test]
    fn effective_vigor_zero_when_disabled() {
        let mut z = z();
        z.molt = 50.0;
        z.enabled = false;
        assert_eq!(z.effective_vigor(100.0), 0.0);
    }
}
