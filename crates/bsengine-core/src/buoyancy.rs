use bevy_ecs::prelude::Component;

/// Makes a rigid body float on a water surface.
/// The physics system applies an upward force proportional to the submerged
/// volume each frame, opposing gravity.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Buoyancy {
    /// Density of the fluid (kg/m³). Water ≈ 1000, oil ≈ 900.
    pub fluid_density: f32,
    /// Volume of the object that can be submerged (m³).
    /// The effective displaced volume is clamped to this value.
    pub volume: f32,
    /// Drag coefficient applied to velocity while submerged.
    /// 0 = no drag, higher values slow the object down faster.
    pub linear_drag: f32,
    /// Angular drag applied while submerged.
    pub angular_drag: f32,
    /// Y coordinate (world space) of the water surface.
    pub surface_y: f32,
}

impl Buoyancy {
    pub fn new(volume: f32) -> Self {
        Self {
            fluid_density: 1000.0,
            volume: volume.max(0.0),
            linear_drag: 1.0,
            angular_drag: 0.5,
            surface_y: 0.0,
        }
    }

    pub fn with_fluid_density(mut self, density: f32) -> Self {
        self.fluid_density = density.max(0.0);
        self
    }

    pub fn with_linear_drag(mut self, drag: f32) -> Self {
        self.linear_drag = drag.max(0.0);
        self
    }

    pub fn with_angular_drag(mut self, drag: f32) -> Self {
        self.angular_drag = drag.max(0.0);
        self
    }

    pub fn with_surface_y(mut self, y: f32) -> Self {
        self.surface_y = y;
        self
    }

    /// Maximum upward buoyant force: `fluid_density × volume × gravity (9.81 m/s²)`.
    pub fn max_buoyant_force(&self) -> f32 {
        self.fluid_density * self.volume * 9.81
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buoyancy_defaults() {
        let b = Buoyancy::new(1.0);
        assert!((b.fluid_density - 1000.0).abs() < 0.001);
        assert!((b.volume - 1.0).abs() < 0.001);
        assert!((b.linear_drag - 1.0).abs() < 0.001);
        assert_eq!(b.surface_y, 0.0);
    }

    #[test]
    fn volume_clamped() {
        let b = Buoyancy::new(-5.0);
        assert_eq!(b.volume, 0.0);
    }

    #[test]
    fn fluid_density_clamped() {
        let b = Buoyancy::new(1.0).with_fluid_density(-100.0);
        assert_eq!(b.fluid_density, 0.0);
    }

    #[test]
    fn max_buoyant_force() {
        let b = Buoyancy::new(1.0); // 1000 kg/m³ * 1 m³ * 9.81 m/s²
        assert!((b.max_buoyant_force() - 9810.0).abs() < 1.0);
    }

    #[test]
    fn surface_y_stored() {
        let b = Buoyancy::new(2.0).with_surface_y(5.0);
        assert!((b.surface_y - 5.0).abs() < 0.001);
    }
}
