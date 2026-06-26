use bevy_ecs::prelude::Component;

/// Kinetic friction coefficient. Clamped to [0.0, 1.0].
///
/// Physics solvers typically combine two bodies' friction via geometric mean:
/// `combined = sqrt(a.coefficient * b.coefficient)`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Friction {
    pub coefficient: f32,
}

impl Friction {
    pub fn new(coefficient: f32) -> Self {
        Self {
            coefficient: coefficient.clamp(0.0, 1.0),
        }
    }

    /// Combine with another body's friction using the geometric mean.
    pub fn combine(&self, other: &Friction) -> f32 {
        (self.coefficient * other.coefficient).sqrt()
    }
}

impl Default for Friction {
    fn default() -> Self {
        Self { coefficient: 0.5 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_clamped_high() {
        assert_eq!(Friction::new(2.0).coefficient, 1.0);
    }

    #[test]
    fn coefficient_clamped_low() {
        assert_eq!(Friction::new(-1.0).coefficient, 0.0);
    }

    #[test]
    fn default_is_half() {
        assert_eq!(Friction::default().coefficient, 0.5);
    }

    #[test]
    fn combine_identical_friction() {
        let f = Friction::new(0.4);
        assert!((f.combine(&f) - 0.4).abs() < 0.001);
    }

    #[test]
    fn combine_zero_friction_gives_zero() {
        let a = Friction::new(0.8);
        let b = Friction::new(0.0);
        assert_eq!(a.combine(&b), 0.0);
    }
}
