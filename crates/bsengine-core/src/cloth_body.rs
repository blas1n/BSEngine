use bevy_ecs::prelude::Component;

/// Makes a skinned or mesh entity simulate cloth physics.
/// The cloth solver iterates over a particle grid each frame, applying
/// wind, gravity, and constraint forces to deform the mesh vertices.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct ClothBody {
    /// Mass of each cloth particle in kg. Higher = heavier, less reactive to forces.
    pub particle_mass: f32,
    /// Structural spring stiffness [0, 1]. 1 = rigid, 0 = no resistance to stretching.
    pub stiffness: f32,
    /// Bending stiffness [0, 1]. Resistance to folding perpendicular to the cloth surface.
    pub bending_stiffness: f32,
    /// Damping applied to cloth velocities each frame [0, 1]. Higher = less oscillation.
    pub damping: f32,
    /// Friction against colliders [0, 1].
    pub friction: f32,
    /// Number of solver iterations per physics step. More = stiffer, more accurate, costlier.
    pub solver_iterations: u32,
    /// When true, the cloth responds to `Wind` components in the scene.
    pub wind_response: bool,
    pub enabled: bool,
}

impl ClothBody {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_particle_mass(mut self, mass: f32) -> Self {
        self.particle_mass = mass.max(0.0);
        self
    }

    pub fn with_stiffness(mut self, stiffness: f32) -> Self {
        self.stiffness = stiffness.clamp(0.0, 1.0);
        self
    }

    pub fn with_bending_stiffness(mut self, stiffness: f32) -> Self {
        self.bending_stiffness = stiffness.clamp(0.0, 1.0);
        self
    }

    pub fn with_damping(mut self, damping: f32) -> Self {
        self.damping = damping.clamp(0.0, 1.0);
        self
    }

    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction.clamp(0.0, 1.0);
        self
    }

    pub fn with_solver_iterations(mut self, iterations: u32) -> Self {
        self.solver_iterations = iterations.max(1);
        self
    }

    pub fn without_wind(mut self) -> Self {
        self.wind_response = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for ClothBody {
    fn default() -> Self {
        Self {
            particle_mass: 0.1,
            stiffness: 0.8,
            bending_stiffness: 0.5,
            damping: 0.1,
            friction: 0.2,
            solver_iterations: 4,
            wind_response: true,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cloth_body_defaults() {
        let cb = ClothBody::default();
        assert!((cb.particle_mass - 0.1).abs() < 0.001);
        assert!((cb.stiffness - 0.8).abs() < 0.001);
        assert_eq!(cb.solver_iterations, 4);
        assert!(cb.wind_response);
        assert!(cb.enabled);
    }

    #[test]
    fn stiffness_clamped() {
        let cb = ClothBody::new().with_stiffness(2.0);
        assert!((cb.stiffness - 1.0).abs() < 0.001);
    }

    #[test]
    fn damping_clamped() {
        let cb = ClothBody::new().with_damping(-1.0);
        assert_eq!(cb.damping, 0.0);
    }

    #[test]
    fn solver_iterations_min_one() {
        let cb = ClothBody::new().with_solver_iterations(0);
        assert_eq!(cb.solver_iterations, 1);
    }

    #[test]
    fn without_wind() {
        let cb = ClothBody::new().without_wind();
        assert!(!cb.wind_response);
    }
}
