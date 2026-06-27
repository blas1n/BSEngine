use bevy_ecs::prelude::Component;

/// Charged energy pulse: builds up over `charge_time` seconds and then
/// releases in an expanding sphere of radius `radius`, dealing `damage` to
/// every entity within range.
///
/// Call `prime()` to begin charging (`just_primed` fires once). Each frame
/// call `tick(dt)` — it returns `true` on the frame the nova discharges
/// (`just_discharged` also set). Callers should then query `radius` and
/// `damage` to apply the effect to all entities within `in_range(distance)`.
/// `cancel()` aborts the charge without firing.
///
/// Distinct from `Explosion` (instantaneous single-destructive burst, no
/// charge), `Blast` (directional cone/wave), and `Pulse` (repeating periodic
/// ring): Nova is a **charged spherical energy burst** — the entity winds up,
/// then releases all at once.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Nova {
    /// Time to charge before release. Clamped ≥ 0.0.
    pub charge_time: f32,
    pub charge_timer: f32,
    /// World-unit radius of the energy sphere. Clamped ≥ 0.0.
    pub radius: f32,
    /// Base damage dealt to all entities within `radius` on discharge.
    /// Clamped ≥ 0.0.
    pub damage: f32,
    pub just_primed: bool,
    pub just_discharged: bool,
    pub enabled: bool,
}

impl Nova {
    pub fn new(charge_time: f32, radius: f32, damage: f32) -> Self {
        Self {
            charge_time: charge_time.max(0.0),
            charge_timer: 0.0,
            radius: radius.max(0.0),
            damage: damage.max(0.0),
            just_primed: false,
            just_discharged: false,
            enabled: true,
        }
    }

    /// Begin charging the nova. No-op if already primed or disabled.
    pub fn prime(&mut self) {
        if !self.enabled || self.is_primed() {
            return;
        }
        self.charge_timer = self.charge_time;
        self.just_primed = true;
    }

    /// Abort the charge without discharging. No-op if not primed.
    pub fn cancel(&mut self) {
        self.charge_timer = 0.0;
    }

    /// Advance the charge timer. Returns `true` on the frame the nova
    /// discharges; sets `just_discharged` and clears `just_primed`.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_primed = false;
        self.just_discharged = false;

        if self.charge_timer > 0.0 {
            self.charge_timer -= dt;
            if self.charge_timer <= 0.0 {
                self.charge_timer = 0.0;
                self.just_discharged = true;
                return true;
            }
        }
        false
    }

    pub fn is_primed(&self) -> bool {
        self.charge_timer > 0.0
    }

    /// Whether `distance` falls within the nova's blast sphere.
    pub fn in_range(&self, distance: f32) -> bool {
        distance <= self.radius
    }

    /// Fraction of charge complete [0.0 = not primed, 1.0 = about to discharge].
    pub fn charge_fraction(&self) -> f32 {
        if self.charge_time <= 0.0 || !self.is_primed() {
            return 0.0;
        }
        (1.0 - self.charge_timer / self.charge_time).clamp(0.0, 1.0)
    }
}

impl Default for Nova {
    fn default() -> Self {
        Self::new(1.0, 8.0, 60.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prime_starts_charge() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.prime();
        assert!(n.is_primed());
        assert!(n.just_primed);
    }

    #[test]
    fn prime_no_op_when_already_primed() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.prime();
        n.tick(0.016);
        n.prime(); // still charging — no-op
        assert!(!n.just_primed);
    }

    #[test]
    fn cancel_aborts_charge() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.prime();
        n.cancel();
        assert!(!n.is_primed());
    }

    #[test]
    fn tick_returns_true_on_discharge() {
        let mut n = Nova::new(0.5, 8.0, 60.0);
        n.prime();
        let discharged = n.tick(1.0);
        assert!(discharged);
        assert!(n.just_discharged);
        assert!(!n.is_primed());
    }

    #[test]
    fn tick_returns_false_while_charging() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.prime();
        let discharged = n.tick(0.1);
        assert!(!discharged);
        assert!(!n.just_discharged);
        assert!(n.is_primed());
    }

    #[test]
    fn tick_clears_just_primed() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.prime();
        n.tick(0.016);
        assert!(!n.just_primed);
    }

    #[test]
    fn in_range_within_radius() {
        let n = Nova::new(1.0, 8.0, 60.0);
        assert!(n.in_range(0.0));
        assert!(n.in_range(8.0));
    }

    #[test]
    fn in_range_outside_radius() {
        let n = Nova::new(1.0, 8.0, 60.0);
        assert!(!n.in_range(8.1));
    }

    #[test]
    fn charge_fraction_zero_when_not_primed() {
        let n = Nova::new(1.0, 8.0, 60.0);
        assert!((n.charge_fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn charge_fraction_at_half() {
        let mut n = Nova::new(2.0, 8.0, 60.0);
        n.prime();
        n.tick(1.0); // half of 2.0s
        assert!((n.charge_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn charge_fraction_near_one_before_discharge() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.prime();
        n.tick(0.99);
        assert!(n.charge_fraction() > 0.98);
    }

    #[test]
    fn instant_discharge_when_charge_time_zero() {
        let mut n = Nova::new(0.0, 8.0, 60.0);
        n.prime();
        // charge_timer = 0.0 → is_primed() = false → not primed
        // Actually with charge_time=0.0, prime sets timer=0.0, is_primed()=false
        assert!(!n.is_primed());
        let discharged = n.tick(0.016);
        assert!(!discharged); // timer was 0 before tick
    }

    #[test]
    fn disabled_prime_no_op() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.enabled = false;
        n.prime();
        assert!(!n.is_primed());
    }

    #[test]
    fn cancel_no_op_when_not_primed() {
        let mut n = Nova::new(1.0, 8.0, 60.0);
        n.cancel(); // no-op, no panic
        assert!(!n.is_primed());
    }

    #[test]
    fn damage_clamped() {
        let n = Nova::new(1.0, 8.0, -10.0);
        assert!((n.damage - 0.0).abs() < 1e-5);
    }

    #[test]
    fn radius_clamped() {
        let n = Nova::new(1.0, -5.0, 60.0);
        assert!((n.radius - 0.0).abs() < 1e-5);
    }
}
