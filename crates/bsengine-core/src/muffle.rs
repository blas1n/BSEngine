use bevy_ecs::prelude::Component;

/// Noise-suppression buff that reduces how far the entity's sounds (footsteps,
/// attacks, abilities) travel, making it harder for AI and detection systems
/// to sense the entity by hearing.
///
/// `effective_sound_radius(base)` returns `base * sound_radius_fraction` while
/// active. A fraction of 0.0 is complete silence; 1.0 is no change.
///
/// `apply(duration)` uses high-watermark. `tick(dt)` counts down and sets
/// `just_unmuffled` on expiry. `clear()` removes the buff early.
///
/// Distinct from `Stealth` (visibility-based concealment) and `Silence` (CC
/// that blocks spell-casting): Muffle specifically targets sound emission for
/// audio-based detection systems without hiding the entity visually.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Muffle {
    pub duration: f32,
    pub timer: f32,
    /// Fraction [0.0, 1.0] of normal sound radius emitted while muffled.
    /// 0.0 = completely silent; 1.0 = no reduction.
    pub sound_radius_fraction: f32,
    pub just_muffled: bool,
    pub just_unmuffled: bool,
    pub enabled: bool,
}

impl Muffle {
    pub fn new(sound_radius_fraction: f32) -> Self {
        Self {
            duration: 0.0,
            timer: 0.0,
            sound_radius_fraction: sound_radius_fraction.clamp(0.0, 1.0),
            just_muffled: false,
            just_unmuffled: false,
            enabled: true,
        }
    }

    /// Apply or extend the muffle for `duration` seconds. High-watermark:
    /// only replaces the current timer if the new duration is longer.
    pub fn apply(&mut self, duration: f32) {
        if !self.enabled {
            return;
        }

        if duration > self.timer {
            let was_active = self.is_active();
            self.duration = duration;
            self.timer = duration;
            if !was_active {
                self.just_muffled = true;
            }
        }
    }

    /// Remove the muffle immediately.
    pub fn clear(&mut self) {
        if self.is_active() {
            self.timer = 0.0;
            self.duration = 0.0;
            self.just_unmuffled = true;
        }
    }

    /// Advance the timer; sets `just_unmuffled` when the buff expires.
    pub fn tick(&mut self, dt: f32) {
        self.just_muffled = false;
        self.just_unmuffled = false;

        if self.timer > 0.0 {
            self.timer -= dt;
            if self.timer <= 0.0 {
                self.timer = 0.0;
                self.duration = 0.0;
                self.just_unmuffled = true;
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.timer > 0.0
    }

    /// Effective sound emission radius after applying the muffle.
    /// Returns `base * sound_radius_fraction` while active, `base` otherwise.
    pub fn effective_sound_radius(&self, base: f32) -> f32 {
        if self.is_active() {
            base * self.sound_radius_fraction
        } else {
            base
        }
    }

    /// Fraction of the muffle duration remaining [1.0 = just applied, 0.0 = expired].
    pub fn remaining_fraction(&self) -> f32 {
        if self.duration <= 0.0 {
            return 0.0;
        }
        (self.timer / self.duration).clamp(0.0, 1.0)
    }
}

impl Default for Muffle {
    fn default() -> Self {
        Self::new(0.2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_activates_muffle() {
        let mut m = Muffle::new(0.2);
        m.apply(3.0);
        assert!(m.is_active());
        assert!(m.just_muffled);
    }

    #[test]
    fn apply_extends_on_longer_duration() {
        let mut m = Muffle::new(0.2);
        m.apply(2.0);
        m.tick(0.016);
        m.apply(5.0);
        assert!((m.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn apply_no_extend_on_shorter_duration() {
        let mut m = Muffle::new(0.2);
        m.apply(5.0);
        m.apply(2.0);
        assert!((m.timer - 5.0).abs() < 1e-4);
    }

    #[test]
    fn tick_expires_muffle() {
        let mut m = Muffle::new(0.2);
        m.apply(1.0);
        m.tick(1.1);
        assert!(!m.is_active());
        assert!(m.just_unmuffled);
    }

    #[test]
    fn clear_ends_early() {
        let mut m = Muffle::new(0.2);
        m.apply(5.0);
        m.clear();
        assert!(!m.is_active());
        assert!(m.just_unmuffled);
    }

    #[test]
    fn effective_sound_radius_while_active() {
        let mut m = Muffle::new(0.3);
        m.apply(3.0);
        let radius = m.effective_sound_radius(20.0);
        assert!((radius - 6.0).abs() < 1e-3); // 20 * 0.3
    }

    #[test]
    fn effective_sound_radius_when_inactive() {
        let m = Muffle::new(0.3);
        assert!((m.effective_sound_radius(20.0) - 20.0).abs() < 1e-5);
    }

    #[test]
    fn complete_silence_at_zero_fraction() {
        let mut m = Muffle::new(0.0);
        m.apply(3.0);
        assert!((m.effective_sound_radius(20.0) - 0.0).abs() < 1e-5);
    }

    #[test]
    fn remaining_fraction_at_half() {
        let mut m = Muffle::new(0.2);
        m.apply(2.0);
        m.tick(1.0);
        assert!((m.remaining_fraction() - 0.5).abs() < 1e-3);
    }

    #[test]
    fn disabled_apply_no_op() {
        let mut m = Muffle::new(0.2);
        m.enabled = false;
        m.apply(5.0);
        assert!(!m.is_active());
    }

    #[test]
    fn tick_clears_just_muffled() {
        let mut m = Muffle::new(0.2);
        m.apply(3.0);
        m.tick(0.016);
        assert!(!m.just_muffled);
    }
}
