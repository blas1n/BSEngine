use bevy_ecs::prelude::Component;

/// Whether the magnet pulls entities toward itself or pushes them away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MagnetMode {
    Attract,
    Repel,
}

/// Area-of-effect force field that pulls or pushes entities within its radius.
///
/// The magnet system queries all entities within `radius` and applies
/// `force_at(distance)` each frame. The owning entity supplies the origin.
///
/// Force law: `strength / distance.powf(falloff)` (default falloff = 2.0,
/// inverse-square). Set `falloff = 1.0` for linear, `0.0` for uniform force.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Magnet {
    pub mode: MagnetMode,
    /// Influence radius (metres). Entities beyond this receive no force.
    pub radius: f32,
    /// Base force strength at unit distance.
    pub strength: f32,
    /// Distance falloff exponent (2.0 = inverse-square, 1.0 = linear).
    pub falloff: f32,
    /// Whether the magnet affects projectiles.
    pub affects_projectiles: bool,
    /// Whether the magnet affects non-projectile entities.
    pub affects_entities: bool,
    pub enabled: bool,
}

impl Magnet {
    pub fn attract(radius: f32, strength: f32) -> Self {
        Self::new(MagnetMode::Attract, radius, strength)
    }

    pub fn repel(radius: f32, strength: f32) -> Self {
        Self::new(MagnetMode::Repel, radius, strength)
    }

    pub fn new(mode: MagnetMode, radius: f32, strength: f32) -> Self {
        Self {
            mode,
            radius: radius.max(0.0),
            strength: strength.max(0.0),
            falloff: 2.0,
            affects_projectiles: false,
            affects_entities: true,
            enabled: true,
        }
    }

    pub fn with_falloff(mut self, exponent: f32) -> Self {
        self.falloff = exponent.max(0.0);
        self
    }

    pub fn with_affects_projectiles(mut self, v: bool) -> Self {
        self.affects_projectiles = v;
        self
    }

    pub fn with_affects_entities(mut self, v: bool) -> Self {
        self.affects_entities = v;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Unsigned force magnitude at `distance` metres. Returns 0 if outside radius.
    pub fn force_at(&self, distance: f32) -> f32 {
        if !self.enabled || distance > self.radius || distance <= 0.0 {
            return 0.0;
        }
        if self.falloff == 0.0 {
            return self.strength;
        }
        self.strength / distance.powf(self.falloff)
    }

    /// Signed force magnitude: positive = away, negative = toward.
    /// Positive for Repel, negative for Attract.
    pub fn signed_force_at(&self, distance: f32) -> f32 {
        let f = self.force_at(distance);
        match self.mode {
            MagnetMode::Attract => -f,
            MagnetMode::Repel => f,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn force_decreases_with_distance() {
        let m = Magnet::attract(10.0, 100.0);
        let near = m.force_at(1.0);
        let far = m.force_at(5.0);
        assert!(near > far);
    }

    #[test]
    fn force_zero_outside_radius() {
        let m = Magnet::attract(5.0, 100.0);
        assert_eq!(m.force_at(10.0), 0.0);
    }

    #[test]
    fn repel_signed_force_positive() {
        let m = Magnet::repel(10.0, 100.0);
        assert!(m.signed_force_at(2.0) > 0.0);
    }

    #[test]
    fn attract_signed_force_negative() {
        let m = Magnet::attract(10.0, 100.0);
        assert!(m.signed_force_at(2.0) < 0.0);
    }

    #[test]
    fn uniform_falloff_constant_force() {
        let m = Magnet::attract(10.0, 50.0).with_falloff(0.0);
        assert!((m.force_at(1.0) - 50.0).abs() < 1e-5);
        assert!((m.force_at(9.0) - 50.0).abs() < 1e-5);
    }

    #[test]
    fn disabled_returns_zero() {
        let m = Magnet::attract(10.0, 100.0).disabled();
        assert_eq!(m.force_at(1.0), 0.0);
    }
}
