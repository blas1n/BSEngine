use bevy_ecs::prelude::Component;

/// Leap-to-target attack component that launches the entity through the air
/// toward a chosen position.
///
/// Call `leap(duration)` to begin the pounce; the physics system should move
/// the entity along its trajectory until `tick(dt)` returns `true` or `land()`
/// is called manually (e.g., on collision with the ground). On landing, the
/// combat system reads `damage` and `knockdown_duration` to apply to any entity
/// at the impact point.
///
/// `tick(dt)` counts down the flight timer and sets `just_landed` when it
/// expires (auto-landing). `land()` can be called early (e.g., on collision).
///
/// Distinct from `Dash` (horizontal ground movement with no vertical arc),
/// `Charge` (forward rush while staying grounded), and `Knockback` (outward
/// push applied to a target): Pounce is an airborne leap — the attacking entity
/// goes airborne and lands on the target, dealing damage and knocking them down.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pounce {
    pub duration: f32,
    pub timer: f32,
    /// Damage dealt at the landing impact point. Clamped ≥ 0.0.
    pub damage: f32,
    /// Duration the hit target is knocked down after impact. Clamped ≥ 0.0.
    pub knockdown_duration: f32,
    /// Minimum range at which the pounce can target a destination. Clamped ≥ 0.0.
    pub min_range: f32,
    /// Maximum range at which the pounce can target a destination. Clamped ≥ 0.0.
    pub max_range: f32,
    pub just_leaped: bool,
    pub just_landed: bool,
    pub enabled: bool,
}

impl Pounce {
    pub fn new(damage: f32, knockdown_duration: f32, min_range: f32, max_range: f32) -> Self {
        let min_r = min_range.max(0.0);
        let max_r = max_range.max(min_r);
        Self {
            duration: 0.0,
            timer: 0.0,
            damage: damage.max(0.0),
            knockdown_duration: knockdown_duration.max(0.0),
            min_range: min_r,
            max_range: max_r,
            just_leaped: false,
            just_landed: false,
            enabled: true,
        }
    }

    /// Begin the pounce for `duration` seconds. No-op if already airborne or
    /// disabled.
    pub fn leap(&mut self, duration: f32) {
        if !self.enabled || self.is_airborne() {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        self.just_leaped = true;
    }

    /// Manually complete the pounce (e.g., on ground collision). No-op if not
    /// airborne.
    pub fn land(&mut self) {
        if self.is_airborne() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_landed = true;
        }
    }

    /// Advance the timer. Returns `true` and sets `just_landed` when the flight
    /// duration expires.
    pub fn tick(&mut self, dt: f32) -> bool {
        self.just_leaped = false;
        self.just_landed = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_landed = true;
                return true;
            }
        }
        false
    }

    pub fn is_airborne(&self) -> bool {
        self.timer > 0.0
    }

    /// Whether a target at `distance` world units away is within pounce range.
    pub fn in_range(&self, distance: f32) -> bool {
        distance >= self.min_range && distance <= self.max_range
    }

    /// Fraction of flight time remaining [1.0 = just leaped, 0.0 = landed].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Pounce {
    fn default() -> Self {
        Self::new(25.0, 1.0, 2.0, 8.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leap_starts_pounce() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(0.5);
        assert!(p.is_airborne());
        assert!(p.just_leaped);
    }

    #[test]
    fn leap_no_op_when_already_airborne() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(0.5);
        p.tick(0.016);
        let before = p.timer;
        p.leap(2.0); // should not reset
        assert!((p.timer - before).abs() < 1e-4);
    }

    #[test]
    fn land_ends_pounce() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(0.5);
        p.land();
        assert!(!p.is_airborne());
        assert!(p.just_landed);
    }

    #[test]
    fn tick_expires_pounce() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(0.4);
        let landed = p.tick(0.5);
        assert!(landed);
        assert!(!p.is_airborne());
        assert!(p.just_landed);
    }

    #[test]
    fn tick_returns_false_mid_flight() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(1.0);
        let landed = p.tick(0.1);
        assert!(!landed);
        assert!(p.is_airborne());
    }

    #[test]
    fn tick_clears_just_leaped() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(1.0);
        p.tick(0.016);
        assert!(!p.just_leaped);
    }

    #[test]
    fn land_no_op_when_not_airborne() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.land(); // should not set just_landed
        assert!(!p.just_landed);
    }

    #[test]
    fn in_range_true_within_bounds() {
        let p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        assert!(p.in_range(5.0));
        assert!(p.in_range(2.0));
        assert!(p.in_range(8.0));
    }

    #[test]
    fn in_range_false_outside_bounds() {
        let p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        assert!(!p.in_range(1.0)); // too close
        assert!(!p.in_range(9.0)); // too far
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(1.0);
        p.tick(0.5);
        assert!((p.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_leap_no_op() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.enabled = false;
        p.leap(0.5);
        assert!(!p.is_airborne());
    }

    #[test]
    fn max_range_clamped_to_min_range() {
        let p = Pounce::new(25.0, 1.0, 5.0, 2.0); // max < min → clamped to min
        assert!(p.max_range >= p.min_range);
    }

    #[test]
    fn can_leap_again_after_landing() {
        let mut p = Pounce::new(25.0, 1.0, 2.0, 8.0);
        p.leap(0.4);
        p.tick(0.5); // lands
        p.tick(0.016); // clear flags
        p.leap(0.4);
        assert!(p.is_airborne());
    }
}
