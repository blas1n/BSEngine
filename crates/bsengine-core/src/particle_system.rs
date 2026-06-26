use bevy_ecs::prelude::Component;
use glam::Vec3;

/// How new particles are distributed at birth.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum EmissionShape {
    /// All particles spawn at the entity's world position.
    #[default]
    Point,
    /// Particles spawn on the surface of a sphere of the given radius.
    Sphere { radius: f32 },
    /// Particles spawn inside an axis-aligned box of the given half-extents.
    Box { half_extents: Vec3 },
    /// Particles spawn on a cone, directed along +Y. `half_angle` is in radians.
    Cone { half_angle: f32, radius: f32 },
}

/// CPU-side particle emitter. Drives a GPU particle system by supplying spawn parameters.
/// The render system reads this component to determine how many particles to emit per frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct ParticleSystem {
    /// Texture path used for individual particle billboards.
    pub texture: String,
    /// Particles emitted per second. Fractional rates are accumulated across frames.
    pub emission_rate: f32,
    /// Range of particle lifetimes in seconds `[min, max]`.
    pub lifetime: [f32; 2],
    /// Range of initial particle speeds `[min, max]` (m/s).
    pub speed: [f32; 2],
    /// Range of particle sizes `[min, max]` (world-space diameter).
    pub size: [f32; 2],
    /// Starting color RGBA.
    pub color_start: [f32; 4],
    /// Ending color RGBA — particles lerp from start to end over their lifetime.
    pub color_end: [f32; 4],
    pub shape: EmissionShape,
    /// When true, particles are simulated in world space and persist after the emitter moves.
    pub world_space: bool,
    /// Gravity multiplier applied to each particle's velocity each second.
    pub gravity_scale: f32,
    pub enabled: bool,
}

impl ParticleSystem {
    pub fn new(texture: impl Into<String>) -> Self {
        Self {
            texture: texture.into(),
            ..Self::default()
        }
    }

    pub fn with_emission_rate(mut self, rate: f32) -> Self {
        self.emission_rate = rate.max(0.0);
        self
    }

    pub fn with_lifetime(mut self, min: f32, max: f32) -> Self {
        self.lifetime = [min.max(0.0), max.max(min.max(0.0))];
        self
    }

    pub fn with_speed(mut self, min: f32, max: f32) -> Self {
        self.speed = [min.max(0.0), max.max(min.max(0.0))];
        self
    }

    pub fn with_size(mut self, min: f32, max: f32) -> Self {
        self.size = [min.max(0.0), max.max(min.max(0.0))];
        self
    }

    pub fn with_shape(mut self, shape: EmissionShape) -> Self {
        self.shape = shape;
        self
    }

    pub fn with_gravity_scale(mut self, scale: f32) -> Self {
        self.gravity_scale = scale;
        self
    }

    pub fn world_space(mut self) -> Self {
        self.world_space = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for ParticleSystem {
    fn default() -> Self {
        Self {
            texture: String::new(),
            emission_rate: 20.0,
            lifetime: [0.5, 1.5],
            speed: [1.0, 3.0],
            size: [0.1, 0.3],
            color_start: [1.0, 1.0, 1.0, 1.0],
            color_end: [1.0, 1.0, 1.0, 0.0],
            shape: EmissionShape::Point,
            world_space: false,
            gravity_scale: 0.0,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particle_system_defaults() {
        let ps = ParticleSystem::default();
        assert!((ps.emission_rate - 20.0).abs() < 0.001);
        assert_eq!(ps.lifetime, [0.5, 1.5]);
        assert_eq!(ps.shape, EmissionShape::Point);
        assert!(ps.enabled);
    }

    #[test]
    fn emission_rate_clamped() {
        let ps = ParticleSystem::new("smoke").with_emission_rate(-5.0);
        assert_eq!(ps.emission_rate, 0.0);
    }

    #[test]
    fn lifetime_range_ordered() {
        let ps = ParticleSystem::new("sparks").with_lifetime(1.0, 0.5);
        assert!(ps.lifetime[0] <= ps.lifetime[1]);
    }

    #[test]
    fn sphere_shape() {
        let ps = ParticleSystem::new("fire").with_shape(EmissionShape::Sphere { radius: 2.0 });
        assert_eq!(ps.shape, EmissionShape::Sphere { radius: 2.0 });
    }

    #[test]
    fn world_space_flag() {
        let ps = ParticleSystem::new("dust").world_space();
        assert!(ps.world_space);
    }
}
