use bevy_ecs::prelude::Component;
use glam::Vec3;

/// Weapon recoil — positional kick and angular kick applied to a camera or weapon socket.
///
/// On fire: call `kick(force, angular)`. Each frame the movement system calls `tick(dt)`
/// to recover toward zero and reads `position_offset` / `angular_offset` for the camera.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Recoil {
    /// Current positional displacement from rest (metres, local camera space).
    pub position_offset: Vec3,
    /// Current angular displacement from rest (radians; x=pitch, y=yaw, z=roll).
    pub angular_offset: Vec3,
    /// Direction the next kick will push the position (normalised by the weapon system).
    pub kick_direction: Vec3,
    /// Magnitude of the next positional kick (metres).
    pub kick_force: f32,
    /// Magnitude of the next angular kick (radians).
    pub angular_kick: f32,
    /// Speed at which offsets recover toward zero (units per second).
    pub recovery_speed: f32,
    /// Fraction of the angular kick applied as yaw (left-right spread); remainder is pitch.
    pub yaw_fraction: f32,
    /// Maximum allowed positional offset (metres).
    pub max_position_offset: f32,
    /// Maximum allowed angular offset (radians).
    pub max_angular_offset: f32,
    pub enabled: bool,
}

impl Recoil {
    pub fn new(kick_force: f32, angular_kick: f32, recovery_speed: f32) -> Self {
        Self {
            position_offset: Vec3::ZERO,
            angular_offset: Vec3::ZERO,
            kick_direction: -Vec3::Z, // default: kick camera backwards
            kick_force: kick_force.max(0.0),
            angular_kick: angular_kick.max(0.0),
            recovery_speed: recovery_speed.max(0.0),
            yaw_fraction: 0.15,
            max_position_offset: 0.3,
            max_angular_offset: 0.5,
            enabled: true,
        }
    }

    pub fn with_yaw_fraction(mut self, f: f32) -> Self {
        self.yaw_fraction = f.clamp(0.0, 1.0);
        self
    }

    pub fn with_limits(mut self, max_pos: f32, max_ang: f32) -> Self {
        self.max_position_offset = max_pos.max(0.0);
        self.max_angular_offset = max_ang.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Apply a single recoil event (e.g. on weapon fire).
    pub fn kick(&mut self) {
        if !self.enabled {
            return;
        }
        let pos_kick = self.kick_direction.normalize_or_zero() * self.kick_force;
        self.position_offset = (self.position_offset + pos_kick).clamp(
            Vec3::splat(-self.max_position_offset),
            Vec3::splat(self.max_position_offset),
        );

        let pitch = self.angular_kick * (1.0 - self.yaw_fraction);
        let yaw = self.angular_kick * self.yaw_fraction;
        let ang_kick = Vec3::new(pitch, yaw, 0.0);
        self.angular_offset = (self.angular_offset + ang_kick).clamp(
            Vec3::splat(-self.max_angular_offset),
            Vec3::splat(self.max_angular_offset),
        );
    }

    /// Recover offsets toward zero. Call every frame.
    pub fn tick(&mut self, dt: f32) {
        if !self.enabled {
            return;
        }
        let step = self.recovery_speed * dt;
        self.position_offset = move_toward_zero(self.position_offset, step);
        self.angular_offset = move_toward_zero(self.angular_offset, step);
    }

    pub fn is_at_rest(&self) -> bool {
        self.position_offset == Vec3::ZERO && self.angular_offset == Vec3::ZERO
    }

    /// Immediately snap all offsets back to rest.
    pub fn reset(&mut self) {
        self.position_offset = Vec3::ZERO;
        self.angular_offset = Vec3::ZERO;
    }
}

fn move_toward_zero(v: Vec3, step: f32) -> Vec3 {
    let len = v.length();
    if len <= step {
        Vec3::ZERO
    } else {
        v * ((len - step) / len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kick_adds_offset() {
        let mut r = Recoil::new(0.1, 0.05, 5.0);
        r.kick_direction = -Vec3::Z;
        r.kick();
        assert!(r.position_offset.length() > 0.0);
        assert!(r.angular_offset.length() > 0.0);
    }

    #[test]
    fn tick_recovers_toward_zero() {
        let mut r = Recoil::new(0.1, 0.05, 10.0);
        r.kick();
        let before = r.position_offset.length();
        r.tick(0.1);
        assert!(r.position_offset.length() < before);
    }

    #[test]
    fn fully_recovers_after_enough_time() {
        let mut r = Recoil::new(0.1, 0.05, 5.0);
        r.kick();
        for _ in 0..100 {
            r.tick(0.1);
        }
        assert!(r.is_at_rest());
    }

    #[test]
    fn offset_clamped_by_limits() {
        let mut r = Recoil::new(10.0, 5.0, 1.0).with_limits(0.1, 0.2);
        r.kick();
        assert!(r.position_offset.length() <= 0.1 + 1e-5);
        assert!(r.angular_offset.length() <= 0.35); // sqrt(0.2^2 + 0.03^2) approx
    }

    #[test]
    fn reset_clears_offsets() {
        let mut r = Recoil::new(0.1, 0.05, 1.0);
        r.kick();
        r.reset();
        assert!(r.is_at_rest());
    }

    #[test]
    fn disabled_ignores_kick() {
        let mut r = Recoil::new(0.1, 0.05, 1.0).disabled();
        r.kick();
        assert!(r.is_at_rest());
    }
}
