use bevy_ecs::prelude::Component;

/// Leg-injury debuff that reduces movement speed and optionally blocks dashes
/// and sprints by physically impairing the entity's locomotion.
///
/// While hobbled, `effective_move_speed(base)` returns `base * speed_fraction`.
/// When `prevents_dash` is true, the dash and sprint systems should check
/// `is_active()` and skip activation.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_recovered` on expiry. `clear()` removes the debuff early.
///
/// Distinct from `Slow` (generic velocity multiplier with no physical flavour)
/// and `Snare` (full root that stops all movement): Hobble models physical leg
/// impairment — the entity can still move but at reduced capacity and cannot
/// perform explosive movements like dashes or sprints.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Hobble {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of base move speed available while hobbled.
    pub speed_fraction: f32,
    /// If true, dash and sprint systems must skip activation while hobbled.
    pub prevents_dash: bool,
    pub just_hobbled: bool,
    pub just_recovered: bool,
    pub enabled: bool,
}

impl Hobble {
    pub fn new(speed_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            speed_fraction: speed_fraction.clamp(0.0, 1.0),
            prevents_dash: false,
            just_hobbled: false,
            just_recovered: false,
            enabled: true,
        }
    }

    pub fn with_prevent_dash(mut self, prevents: bool) -> Self {
        self.prevents_dash = prevents;
        self
    }

    /// Apply or extend the hobble for `duration` seconds. High-watermark: only
    /// replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_hobbled = true;
            }
        }
    }

    /// Remove the hobble immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_recovered = true;
        }
    }

    /// Advance the timer; sets `just_recovered` when the debuff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_hobbled = false;
        self.just_recovered = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_recovered = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective move speed after applying the hobble.
    /// Returns `base * speed_fraction` while active, `base` otherwise.
    pub fn effective_move_speed(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.speed_fraction
        } else {
            base
        }
    }

    /// Fraction of the hobble duration remaining [1.0 = just applied, 0.0 = recovered].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Hobble {
    fn default() -> Self {
        Self::new(0.5).with_prevent_dash(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_hobble() {
        let mut h = Hobble::new(0.5);
        h.apply(3.0);
        assert!(h.is_active());
        assert!(h.just_hobbled);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut h = Hobble::new(0.5);
        h.apply(2.0);
        h.tick(0.016);
        h.apply(5.0);
        assert!((h.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut h = Hobble::new(0.5);
        h.apply(5.0);
        h.apply(2.0);
        assert!((h.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_hobble() {
        let mut h = Hobble::new(0.5);
        h.apply(1.0);
        h.tick(1.1);
        assert!(!h.is_active());
        assert!(h.just_recovered);
    }

    #[test]
    fn clear_ends_early() {
        let mut h = Hobble::new(0.5);
        h.apply(5.0);
        h.clear();
        assert!(!h.is_active());
        assert!(h.just_recovered);
    }

    #[test]
    fn effective_move_speed_while_active() {
        let mut h = Hobble::new(0.4);
        h.apply(3.0);
        let speed = h.effective_move_speed(10.0);
        assert!((speed - 4.0).abs() < 1e-4); // 10 * 0.4
    }

    #[test]
    fn effective_move_speed_when_inactive() {
        let h = Hobble::new(0.4);
        assert!((h.effective_move_speed(10.0) - 10.0).abs() < 1e-5);
    }

    #[test]
    fn prevents_dash_set_by_builder() {
        let h = Hobble::new(0.5).with_prevent_dash(true);
        assert!(h.prevents_dash);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut h = Hobble::new(0.5);
        h.apply(2.0);
        h.tick(1.0);
        assert!((h.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut h = Hobble::new(0.5);
        h.enabled = false;
        h.apply(5.0);
        assert!(!h.is_active());
    }

    #[test]
    fn tick_clears_just_hobbled() {
        let mut h = Hobble::new(0.5);
        h.apply(3.0);
        h.tick(0.016);
        assert!(!h.just_hobbled);
    }
}
