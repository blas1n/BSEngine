use bevy_ecs::prelude::Component;

/// Point-of-interest broadcaster for AI navigation and HUD systems.
///
/// When lit, the beacon signals its location and `priority` to any agent or
/// system within `broadcast_radius` world units. Call `light(duration)` with
/// `duration = 0.0` for a permanent beacon (stays lit until `extinguish()` is
/// called), or with a positive duration for a timed beacon that self-extinguishes
/// when the timer runs out.
///
/// `tick(dt)` advances the timer and sets `just_extinguished` on expiry.
/// Calling `light(duration)` while already lit replaces the current timer.
///
/// Distinct from `Alarm` (post-detection alert state), `Radar` (active sweep),
/// and `Notice` (passive acknowledgement): Beacon is a persistent point-of-
/// interest marker — it passively signals "come here" or "observe this" to
/// nearby agents rather than reacting to a detected threat.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Beacon {
    /// Relative importance; higher value = higher priority for competing agents.
    pub priority: u32,
    /// World-unit radius within which the beacon broadcasts its presence.
    pub broadcast_radius: f32,
    /// Duration in seconds; `0.0` means permanent (requires manual extinguish).
    pub duration: f32,
    pub timer: f32,
    pub lit: bool,
    pub just_lit: bool,
    pub just_extinguished: bool,
    pub enabled: bool,
}

impl Beacon {
    pub fn new(priority: u32, broadcast_radius: f32) -> Self {
        Self {
            priority,
            broadcast_radius: broadcast_radius.max(0.0),
            duration: 0.0,
            timer: 0.0,
            lit: false,
            just_lit: false,
            just_extinguished: false,
            enabled: true,
        }
    }

    /// Light the beacon. `duration = 0.0` means permanent; positive values set
    /// a countdown that extinguishes the beacon automatically.  Replaces any
    /// current timer. No-op when disabled.
    pub fn light(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }
        self.duration = duration.max(0.0);
        self.timer = self.duration;
        let was_lit = self.lit;
        self.lit = true;
        if !was_lit {
            self.just_lit = true;
        }
    }

    /// Extinguish the beacon immediately.
    pub fn extinguish(&mut self) {
        if self.lit {
            self.lit = false;
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_extinguished = true;
        }
    }

    /// Advance the timer; sets `just_extinguished` when a timed beacon expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_lit = false;
        self.just_extinguished = false;

        if self.lit && self.duration > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.lit = false;
                self.just_extinguished = true;
            }
        }
    }

    pub fn is_lit(&self) -> bool {
        self.lit
    }

    /// Whether the beacon is lit indefinitely (duration == 0.0 while lit).
    pub fn is_permanent(&self) -> bool {
        self.lit && self.duration == 0.0
    }

    /// Fraction of the timed duration remaining. Returns `1.0` for permanent
    /// beacons that are lit, `0.0` when unlit or at expiry.
    pub fn remaining_fraction(&self) -> f32 {
        if !self.lit {
            return 0.0;
        }
        if self.duration <= 0.0 {
            return 1.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Beacon {
    fn default() -> Self {
        Self::new(1, 20.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_permanent_activates_beacon() {
        let mut b = Beacon::new(1, 20.0);
        b.light(0.0);
        assert!(b.is_lit());
        assert!(b.is_permanent());
        assert!(b.just_lit);
    }

    #[test]
    fn light_timed_activates_beacon() {
        let mut b = Beacon::new(1, 20.0);
        b.light(5.0);
        assert!(b.is_lit());
        assert!(!b.is_permanent());
        assert!((b.timer - 5.0).abs() < 1e-5);
    }

    #[test]
    fn light_replaces_timer_when_already_lit() {
        let mut b = Beacon::new(1, 20.0);
        b.light(3.0);
        b.tick(1.0);
        b.light(10.0);
        assert!((b.timer - 10.0).abs() < 1e-4);
    }

    #[test]
    fn light_does_not_set_just_lit_when_already_lit() {
        let mut b = Beacon::new(1, 20.0);
        b.light(3.0);
        b.tick(0.016);
        b.light(5.0); // retrigger while lit
        assert!(!b.just_lit);
    }

    #[test]
    fn extinguish_ends_beacon() {
        let mut b = Beacon::new(1, 20.0);
        b.light(0.0);
        b.extinguish();
        assert!(!b.is_lit());
        assert!(b.just_extinguished);
    }

    #[test]
    fn tick_expires_timed_beacon() {
        let mut b = Beacon::new(1, 20.0);
        b.light(2.0);
        b.tick(2.1);
        assert!(!b.is_lit());
        assert!(b.just_extinguished);
    }

    #[test]
    fn tick_does_not_expire_permanent_beacon() {
        let mut b = Beacon::new(1, 20.0);
        b.light(0.0);
        b.tick(100.0);
        assert!(b.is_lit());
        assert!(!b.just_extinguished);
    }

    #[test]
    fn tick_clears_just_lit() {
        let mut b = Beacon::new(1, 20.0);
        b.light(5.0);
        b.tick(0.016);
        assert!(!b.just_lit);
    }

    #[test]
    fn remaining_fraction_permanent() {
        let mut b = Beacon::new(1, 20.0);
        b.light(0.0);
        assert!((b.remaining_fraction() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_timed_at_half() {
        let mut b = Beacon::new(1, 20.0);
        b.light(4.0);
        b.tick(2.0);
        assert!((b.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn remaining_fraction_when_unlit() {
        let b = Beacon::new(1, 20.0);
        assert!((b.remaining_fraction() - 0.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_light_no_op() {
        let mut b = Beacon::new(1, 20.0);
        b.enabled = false;
        b.light(5.0);
        assert!(!b.is_lit());
    }

    #[test]
    fn extinguish_no_op_when_unlit() {
        let mut b = Beacon::new(1, 20.0);
        b.extinguish(); // should not set just_extinguished
        assert!(!b.just_extinguished);
    }

    #[test]
    fn broadcast_radius_clamped() {
        let b = Beacon::new(1, -5.0);
        assert!((b.broadcast_radius - 0.0).abs() < 1e-5);
    }
}
