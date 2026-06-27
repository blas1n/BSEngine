use bevy_ecs::prelude::Component;

/// Whether the pulse fires once or repeats on an interval.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseMode {
    /// Fires exactly once, then becomes Inactive.
    Oneshot,
    /// Fires repeatedly at `interval` seconds.
    Repeating,
}

/// Rhythmic outward-wave emitter component.
///
/// Models any effect that radiates outward in a sphere at regular intervals:
/// sonar pings, shield ripples, heartbeat VFX, AoE damage rings, or decoy
/// noise pulses. The component tracks timing; the rendering/effect system
/// reads `just_pulsed` and `radius` each frame to spawn the visual wave.
///
/// Distinct from `Oscillate` (positional back-and-forth), `Lure` (AI
/// attraction radius), and `Explosion` (one-shot detonation).
///
/// Call `activate()` to start. `tick(dt)` drives the interval timer and
/// sets `just_pulsed` for one frame each time the timer fires.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Pulse {
    pub mode: PulseMode,
    pub is_active: bool,
    /// Emission radius of each wave (units).
    pub radius: f32,
    /// Optional maximum radius the wave expands to (0 = instant effect).
    pub max_radius: f32,
    /// Time between pulses in seconds.
    pub interval: f32,
    /// Countdown to next pulse.
    pub timer: f32,
    /// [0, 1] attenuation at the edge of `max_radius`.
    pub falloff: f32,
    /// Number of pulses fired since activation.
    pub pulse_count: u32,
    /// True on the exact frame a pulse fires.
    pub just_pulsed: bool,
    pub enabled: bool,
}

impl Pulse {
    pub fn new(radius: f32, interval: f32) -> Self {
        Self {
            mode: PulseMode::Repeating,
            is_active: false,
            radius,
            max_radius: 0.0,
            interval: interval.max(0.0),
            timer: 0.0,
            falloff: 1.0,
            pulse_count: 0,
            just_pulsed: false,
            enabled: true,
        }
    }

    pub fn oneshot(radius: f32) -> Self {
        let mut p = Self::new(radius, 0.0);
        p.mode = PulseMode::Oneshot;
        p
    }

    pub fn with_max_radius(mut self, max_radius: f32) -> Self {
        self.max_radius = max_radius.max(0.0);
        self
    }

    pub fn with_falloff(mut self, falloff: f32) -> Self {
        self.falloff = falloff.clamp(0.0, 1.0);
        self
    }

    /// Start pulsing. Fires the first pulse immediately on the next tick.
    pub fn activate(&mut self) {
        if !self.enabled {
            return;
        }
        self.is_active = true;
        self.timer = 0.0; // fires on first tick
        self.pulse_count = 0;
    }

    pub fn deactivate(&mut self) {
        self.is_active = false;
    }

    /// Reset counter and deactivate.
    pub fn reset(&mut self) {
        self.is_active = false;
        self.timer = 0.0;
        self.pulse_count = 0;
        self.just_pulsed = false;
    }

    /// Advance the pulse timer. Call once per frame.
    pub fn tick(&mut self, dt: f32) {
        self.just_pulsed = false;

        if !self.enabled || !self.is_active {
            return;
        }

        self.timer = (self.timer - dt).max(0.0);
        if self.timer <= 0.0 {
            self.just_pulsed = true;
            self.pulse_count += 1;

            match self.mode {
                PulseMode::Repeating => {
                    self.timer = self.interval;
                }
                PulseMode::Oneshot => {
                    self.is_active = false;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fires_on_first_tick() {
        let mut p = Pulse::new(5.0, 1.0);
        p.activate();
        p.tick(0.016);
        assert!(p.just_pulsed);
        assert_eq!(p.pulse_count, 1);
    }

    #[test]
    fn repeating_fires_on_interval() {
        let mut p = Pulse::new(5.0, 1.0);
        p.activate();
        p.tick(0.016); // immediate first pulse
        p.tick(1.0); // second pulse after interval
        assert!(p.just_pulsed);
        assert_eq!(p.pulse_count, 2);
    }

    #[test]
    fn oneshot_fires_once_then_stops() {
        let mut p = Pulse::oneshot(5.0);
        p.activate();
        p.tick(0.016);
        assert!(p.just_pulsed);
        assert!(!p.is_active);
        p.tick(0.016);
        assert!(!p.just_pulsed);
        assert_eq!(p.pulse_count, 1);
    }

    #[test]
    fn deactivate_stops_pulses() {
        let mut p = Pulse::new(5.0, 0.5);
        p.activate();
        p.tick(0.016); // first pulse
        p.deactivate();
        p.tick(1.0);
        assert!(!p.just_pulsed);
        assert_eq!(p.pulse_count, 1);
    }

    #[test]
    fn reset_clears_state() {
        let mut p = Pulse::new(5.0, 1.0);
        p.activate();
        p.tick(0.016);
        p.reset();
        assert_eq!(p.pulse_count, 0);
        assert!(!p.is_active);
    }

    #[test]
    fn disabled_blocks_activate() {
        let mut p = Pulse::new(5.0, 1.0);
        p.enabled = false;
        p.activate();
        p.tick(0.016);
        assert!(!p.just_pulsed);
    }
}
