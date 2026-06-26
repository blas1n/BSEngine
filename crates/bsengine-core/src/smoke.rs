use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Visual style of the smoke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SmokeStyle {
    /// Thin wisp (candle, match).
    Wisp,
    /// Dense cloud (explosion aftermath, engine fire).
    Dense,
    /// Coloured chemical smoke (flare, grenade).
    Chemical,
    /// Steam or mist (cooking pot, vent).
    Steam,
}

/// Smoke emitter attached to an entity.
/// The VFX system reads this to drive a particle system or volumetric effect.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Smoke {
    pub style: SmokeStyle,
    /// Emission rate in particles per second.
    pub rate: f32,
    /// Particle colour (RGBA, linear).
    pub color: [f32; 4],
    /// Initial speed of emitted particles (m/s).
    pub particle_speed: f32,
    /// How fast particles expand radius-wise (m/s).
    pub spread_rate: f32,
    /// Particle lifetime in seconds.
    pub particle_lifetime: f32,
    /// World-space offset from the entity's origin.
    pub offset: Vec3,
    /// If `Some(t)`, the emitter turns off after `t` seconds.
    pub burst_duration: Option<f32>,
    /// Elapsed time since emission started.
    pub elapsed: f32,
    pub enabled: bool,
}

impl Smoke {
    pub fn new(style: SmokeStyle) -> Self {
        Self {
            style,
            rate: 20.0,
            color: [0.6, 0.6, 0.6, 0.8],
            particle_speed: 0.5,
            spread_rate: 0.2,
            particle_lifetime: 3.0,
            offset: Vec3::ZERO,
            burst_duration: None,
            elapsed: 0.0,
            enabled: true,
        }
    }

    pub fn wisp() -> Self {
        Self::new(SmokeStyle::Wisp)
            .with_rate(5.0)
            .with_color([0.8, 0.8, 0.8, 0.5])
    }

    pub fn dense() -> Self {
        Self::new(SmokeStyle::Dense)
            .with_rate(60.0)
            .with_color([0.2, 0.2, 0.2, 0.9])
    }

    pub fn with_rate(mut self, rate: f32) -> Self {
        self.rate = rate.max(0.0);
        self
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_offset(mut self, offset: Vec3) -> Self {
        self.offset = offset;
        self
    }

    pub fn with_burst(mut self, duration: f32) -> Self {
        self.burst_duration = Some(duration.max(0.0));
        self
    }

    pub fn with_lifetime(mut self, seconds: f32) -> Self {
        self.particle_lifetime = seconds.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Advance time. Returns `true` when a burst emitter expires.
    pub fn tick(&mut self, dt: f32) -> bool {
        if !self.enabled {
            return false;
        }
        self.elapsed += dt;
        if let Some(dur) = self.burst_duration {
            if self.elapsed >= dur {
                self.enabled = false;
                return true;
            }
        }
        false
    }

    pub fn is_active(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_wisp_low_rate() {
        let s = Smoke::wisp();
        assert!(s.rate < 10.0);
        assert_eq!(s.style, SmokeStyle::Wisp);
    }

    #[test]
    fn smoke_dense_high_rate() {
        let s = Smoke::dense();
        assert!(s.rate >= 50.0);
    }

    #[test]
    fn smoke_burst_expires() {
        let mut s = Smoke::new(SmokeStyle::Chemical).with_burst(1.0);
        assert!(!s.tick(0.5));
        assert!(s.tick(0.6));
        assert!(!s.is_active());
    }

    #[test]
    fn smoke_continuous_never_expires() {
        let mut s = Smoke::new(SmokeStyle::Steam);
        for _ in 0..100 {
            assert!(!s.tick(0.1));
        }
        assert!(s.is_active());
    }

    #[test]
    fn smoke_disabled_tick_no_op() {
        let mut s = Smoke::new(SmokeStyle::Dense).disabled();
        assert!(!s.tick(10.0));
    }
}
