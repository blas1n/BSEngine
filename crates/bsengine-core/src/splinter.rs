use bevy_ecs::prelude::Component;

/// Heavy-hit burst trigger: when a single incoming hit meets or exceeds
/// `threshold` damage and the component is not on cooldown, `check_hit`
/// returns `true` and sets `just_splintered` — signalling damage systems to
/// apply a radial burst of `burst_damage(hit)` to nearby entities within
/// `radius`. A `cooldown` prevents rapid re-triggering.
///
/// Distinct from `Explosion` (fires on death), `Pierce` (projectile
/// pass-through), and `Scatter` (multi-projectile spread): Splinter is a
/// **heavy-hit reactive burst** — it fires mid-combat whenever a single
/// blow exceeds the threshold, then enters cooldown.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Splinter {
    /// Minimum single-hit damage required to trigger. Clamped ≥ 0.0.
    pub threshold: f32,
    /// World-unit radius of the splinter burst. Clamped ≥ 0.0.
    pub radius: f32,
    /// Fraction of the triggering hit's damage dealt as burst damage.
    /// Clamped ≥ 0.0.
    pub damage_fraction: f32,
    /// Minimum seconds between triggers. Clamped ≥ 0.0.
    pub cooldown: f32,
    pub cooldown_timer: f32,
    pub just_splintered: bool,
    pub enabled: bool,
}

impl Splinter {
    pub fn new(threshold: f32, radius: f32, damage_fraction: f32, cooldown: f32) -> Self {
        Self {
            threshold: threshold.max(0.0),
            radius: radius.max(0.0),
            damage_fraction: damage_fraction.max(0.0),
            cooldown: cooldown.max(0.0),
            cooldown_timer: 0.0,
            just_splintered: false,
            enabled: true,
        }
    }

    /// Call when this entity takes `damage` from a single hit. Returns `true`
    /// when `damage >= threshold`, not on cooldown, and enabled — triggering a
    /// splinter burst and starting the cooldown. Returns `false` otherwise.
    pub fn check_hit(&mut self, damage: f32) -> bool {
        if !self.enabled || damage < self.threshold || self.is_on_cooldown() {
            return false;
        }
        self.cooldown_timer = self.cooldown;
        self.just_splintered = true;
        true
    }

    /// Advance the cooldown timer. Clears `just_splintered` at the start of
    /// each tick.
    pub fn tick(&mut self, dt: f32) {
        self.just_splintered = false;

        if self.cooldown_timer > 0.0 {
            self.cooldown_timer -= dt;
            if self.cooldown_timer < 0.0 {
                self.cooldown_timer = 0.0;
            }
        }
    }

    pub fn is_on_cooldown(&self) -> bool {
        self.cooldown_timer > 0.0
    }

    /// Burst damage dealt to nearby entities when `check_hit` returns `true`.
    pub fn burst_damage(&self, triggering_hit: f32) -> f32 {
        triggering_hit * self.damage_fraction
    }

    /// Fraction of the cooldown remaining [1.0 = just triggered, 0.0 = ready].
    pub fn cooldown_fraction(&self) -> f32 {
        if self.cooldown <= 0.0 {
            return 0.0;
        }
        (self.cooldown_timer / self.cooldown).clamp(0.0, 1.0)
    }
}

impl Default for Splinter {
    fn default() -> Self {
        Self::new(50.0, 3.0, 0.5, 5.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_hit_triggers_on_threshold() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        assert!(s.check_hit(50.0));
        assert!(s.just_splintered);
    }

    #[test]
    fn check_hit_triggers_above_threshold() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        assert!(s.check_hit(100.0));
    }

    #[test]
    fn check_hit_no_trigger_below_threshold() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        assert!(!s.check_hit(49.9));
        assert!(!s.just_splintered);
    }

    #[test]
    fn check_hit_starts_cooldown() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        s.check_hit(100.0);
        assert!(s.is_on_cooldown());
        assert!((s.cooldown_timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn check_hit_no_retrigger_on_cooldown() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        s.check_hit(100.0);
        assert!(!s.check_hit(100.0));
    }

    #[test]
    fn tick_counts_down_cooldown() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        s.check_hit(100.0);
        s.tick(2.0);
        assert!((s.cooldown_timer - 3.0).abs() < 1e-4);
        assert!(s.is_on_cooldown());
    }

    #[test]
    fn tick_clears_cooldown() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        s.check_hit(100.0);
        s.tick(5.1);
        assert!(!s.is_on_cooldown());
        assert_eq!(s.cooldown_timer, 0.0);
    }

    #[test]
    fn tick_clears_just_splintered() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        s.check_hit(100.0);
        s.tick(0.016);
        assert!(!s.just_splintered);
    }

    #[test]
    fn retrigger_after_cooldown_expires() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 2.0);
        s.check_hit(100.0);
        s.tick(2.1);
        assert!(s.check_hit(100.0));
    }

    #[test]
    fn burst_damage_is_fraction_of_hit() {
        let s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        assert!((s.burst_damage(100.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn burst_damage_zero_threshold_fraction() {
        let s = Splinter::new(50.0, 3.0, 0.0, 5.0);
        assert!((s.burst_damage(100.0)).abs() < 1e-5);
    }

    #[test]
    fn cooldown_fraction_at_half() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 4.0);
        s.check_hit(100.0);
        s.tick(2.0);
        assert!((s.cooldown_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn cooldown_fraction_zero_when_ready() {
        let s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        assert!((s.cooldown_fraction()).abs() < 1e-5);
    }

    #[test]
    fn disabled_check_hit_no_op() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 5.0);
        s.enabled = false;
        assert!(!s.check_hit(100.0));
        assert!(!s.just_splintered);
    }

    #[test]
    fn zero_cooldown_allows_immediate_retrigger() {
        let mut s = Splinter::new(50.0, 3.0, 0.5, 0.0);
        assert!(s.check_hit(100.0));
        s.tick(0.016);
        assert!(s.check_hit(100.0));
    }
}
