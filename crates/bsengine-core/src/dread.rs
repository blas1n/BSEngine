use bevy_ecs::prelude::Component;

/// Timed periodic fear broadcaster: entity's presence pulses fear stacks
/// onto nearby enemies at a regular interval.
///
/// `tick(dt)` advances the internal `pulse_timer`. When it reaches
/// `pulse_interval`, `just_pulsed` is set and the timer resets. The combat
/// system should iterate over entities with a `Fear` (or equivalent) component
/// that are within `radius` world units and call their stack/buildup methods
/// when `just_pulsed` is `true`. `tick` is a no-op when disabled, so the
/// timer does not advance.
///
/// `in_range(distance)` returns `true` when the component is enabled and
/// the given distance falls within `radius`. `should_apply_to(distance)`
/// combines `just_pulsed` and `in_range(distance)` for a single call-site
/// guard.
///
/// Distinct from `Demoralize` (instant morale penalty on a hit),
/// `Fear` (the fear state held on the target entity),
/// `Ploy` (deliberate self-weakening ruse), and `Aura` (generic
/// radius stat modifier): Dread is a **timed radial fear broadcaster** —
/// terror accumulates passively over time just from proximity, pulsing at
/// fixed intervals regardless of whether any attack lands.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Dread {
    /// Aura radius in world units. Clamped ≥ 0.0.
    pub radius: f32,
    /// Seconds between fear pulses. Clamped ≥ 0.0 (0 means instant/every frame).
    pub pulse_interval: f32,
    /// Internal timer counting toward the next pulse [0, pulse_interval].
    pub pulse_timer: f32,
    /// Fear stacks applied per pulse. Clamped ≥ 1.
    pub buildup_per_pulse: u32,
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Dread {
    pub fn new(radius: f32, pulse_interval: f32, buildup_per_pulse: u32) -> Self {
        Self {
            radius: radius.max(0.0),
            pulse_interval: pulse_interval.max(0.0),
            pulse_timer: 0.0,
            buildup_per_pulse: buildup_per_pulse.max(1),
            just_pulsed: false,
            enabled: true,
        }
    }

    /// Advance the pulse timer by `dt` seconds. Clears `just_pulsed` at the
    /// start of each tick. When the timer reaches `pulse_interval`, fires
    /// `just_pulsed` and resets. No-op when disabled.
    ///
    /// When `pulse_interval == 0`, `just_pulsed` fires every tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_pulsed = false;
        if !self.enabled {
            return;
        }
        if self.pulse_interval <= 0.0 {
            self.just_pulsed = true;
            return;
        }
        self.pulse_timer += dt;
        if self.pulse_timer >= self.pulse_interval {
            self.pulse_timer = 0.0;
            self.just_pulsed = true;
        }
    }

    /// `true` when enabled and `distance` is within the aura radius.
    pub fn in_range(&self, distance: f32) -> bool {
        self.enabled && distance <= self.radius
    }

    /// `true` when a pulse fired this tick AND `distance` is within range.
    /// Use this as a single guard in the combat system loop.
    pub fn should_apply_to(&self, distance: f32) -> bool {
        self.just_pulsed && self.in_range(distance)
    }

    /// Timer progress toward next pulse as a fraction [0.0, 1.0].
    /// Returns 0.0 when `pulse_interval == 0` (instant pulses).
    pub fn pulse_fraction(&self) -> f32 {
        if self.pulse_interval <= 0.0 {
            return 0.0;
        }
        (self.pulse_timer / self.pulse_interval).clamp(0.0, 1.0)
    }
}

impl Default for Dread {
    fn default() -> Self {
        Self::new(8.0, 2.0, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_starts_without_pulse() {
        let d = Dread::new(8.0, 2.0, 1);
        assert!(!d.just_pulsed);
        assert_eq!(d.pulse_timer, 0.0);
    }

    #[test]
    fn tick_no_pulse_before_interval() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(1.0);
        assert!(!d.just_pulsed);
        assert!((d.pulse_timer - 1.0).abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_pulsed_at_interval() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.0);
        assert!(d.just_pulsed);
        assert!(d.pulse_timer.abs() < 1e-5);
    }

    #[test]
    fn tick_fires_just_pulsed_when_exceeded() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.5);
        assert!(d.just_pulsed);
        // timer resets; excess time is lost (simple reset to 0)
        assert!(d.pulse_timer.abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_pulsed_next_frame() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.0);
        assert!(d.just_pulsed);
        d.tick(0.016);
        assert!(!d.just_pulsed);
    }

    #[test]
    fn tick_no_op_when_disabled() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.enabled = false;
        d.tick(2.0);
        assert!(!d.just_pulsed);
        assert!(d.pulse_timer.abs() < 1e-5);
    }

    #[test]
    fn tick_clears_just_pulsed_even_when_disabled() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.0); // pulse fires
        d.enabled = false;
        d.tick(0.016); // disabled but flag must clear
        assert!(!d.just_pulsed);
    }

    #[test]
    fn tick_zero_interval_pulses_every_frame() {
        let mut d = Dread::new(8.0, 0.0, 1);
        d.tick(0.016);
        assert!(d.just_pulsed);
        d.tick(0.016);
        assert!(d.just_pulsed);
    }

    #[test]
    fn in_range_true_within_radius() {
        let d = Dread::new(8.0, 2.0, 1);
        assert!(d.in_range(0.0));
        assert!(d.in_range(8.0));
    }

    #[test]
    fn in_range_false_beyond_radius() {
        let d = Dread::new(8.0, 2.0, 1);
        assert!(!d.in_range(8.001));
        assert!(!d.in_range(100.0));
    }

    #[test]
    fn in_range_false_when_disabled() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.enabled = false;
        assert!(!d.in_range(1.0));
    }

    #[test]
    fn should_apply_to_true_after_pulse_in_range() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.0);
        assert!(d.should_apply_to(5.0));
    }

    #[test]
    fn should_apply_to_false_before_pulse() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(1.0);
        assert!(!d.should_apply_to(5.0));
    }

    #[test]
    fn should_apply_to_false_out_of_range() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.0);
        assert!(!d.should_apply_to(9.0));
    }

    #[test]
    fn pulse_fraction_before_interval() {
        let mut d = Dread::new(8.0, 4.0, 1);
        d.tick(1.0);
        assert!((d.pulse_fraction() - 0.25).abs() < 1e-4);
    }

    #[test]
    fn pulse_fraction_zero_after_pulse() {
        let mut d = Dread::new(8.0, 2.0, 1);
        d.tick(2.0);
        assert!(d.pulse_fraction().abs() < 1e-5);
    }

    #[test]
    fn pulse_fraction_zero_when_interval_is_zero() {
        let d = Dread::new(8.0, 0.0, 1);
        assert_eq!(d.pulse_fraction(), 0.0);
    }

    #[test]
    fn radius_clamped_non_negative() {
        let d = Dread::new(-5.0, 2.0, 1);
        assert_eq!(d.radius, 0.0);
    }

    #[test]
    fn pulse_interval_clamped_non_negative() {
        let d = Dread::new(8.0, -1.0, 1);
        assert_eq!(d.pulse_interval, 0.0);
    }

    #[test]
    fn buildup_per_pulse_clamped_to_one() {
        let d = Dread::new(8.0, 2.0, 0);
        assert_eq!(d.buildup_per_pulse, 1);
    }

    #[test]
    fn accumulates_across_multiple_intervals() {
        let mut d = Dread::new(8.0, 1.0, 2);
        let mut pulses = 0u32;
        for _ in 0..3 {
            d.tick(1.0);
            if d.just_pulsed {
                pulses += 1;
            }
        }
        assert_eq!(pulses, 3);
        assert_eq!(d.buildup_per_pulse, 2);
    }
}
