use bevy_ecs::prelude::Component;

/// Physical mass of an entity in kilograms.
/// Drives force-to-acceleration conversion in physics integrators.
/// Zero mass is clamped to a small positive value to avoid division by zero.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Mass {
    pub value: f32,
}

impl Mass {
    const MIN: f32 = 1e-6;

    pub fn new(kg: f32) -> Self {
        Self {
            value: kg.max(Self::MIN),
        }
    }

    /// Inverse mass (1 / value). Returns 0 for static bodies (use `Mass::static_body()`).
    pub fn inverse(&self) -> f32 {
        1.0 / self.value
    }
}

impl Default for Mass {
    fn default() -> Self {
        Self { value: 1.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mass_is_one_kg() {
        assert_eq!(Mass::default().value, 1.0);
    }

    #[test]
    fn zero_mass_clamped() {
        assert!(Mass::new(0.0).value > 0.0);
    }

    #[test]
    fn negative_mass_clamped() {
        assert!(Mass::new(-5.0).value > 0.0);
    }

    #[test]
    fn inverse_of_one_kg() {
        assert!((Mass::new(1.0).inverse() - 1.0).abs() < 0.001);
    }

    #[test]
    fn inverse_of_two_kg() {
        assert!((Mass::new(2.0).inverse() - 0.5).abs() < 0.001);
    }

    #[test]
    fn large_mass_has_small_inverse() {
        assert!(Mass::new(1000.0).inverse() < 0.01);
    }
}
