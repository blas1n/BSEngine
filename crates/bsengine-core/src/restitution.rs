use bevy_ecs::prelude::Component;

/// Coefficient of restitution — how much kinetic energy is preserved on collision.
/// Clamped to [0.0, 1.0]: 0.0 = perfectly inelastic, 1.0 = perfectly elastic.
///
/// Physics solvers typically combine via the maximum of the two bodies:
/// `combined = max(a.coefficient, b.coefficient)`.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Restitution {
    pub coefficient: f32,
}

impl Restitution {
    pub fn new(coefficient: f32) -> Self {
        Self {
            coefficient: coefficient.clamp(0.0, 1.0),
        }
    }

    /// Combine with another body's restitution using the maximum value.
    pub fn combine(&self, other: &Restitution) -> f32 {
        self.coefficient.max(other.coefficient)
    }
}

impl Default for Restitution {
    fn default() -> Self {
        Self { coefficient: 0.0 }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coefficient_clamped_high() {
        assert_eq!(Restitution::new(2.0).coefficient, 1.0);
    }

    #[test]
    fn coefficient_clamped_low() {
        assert_eq!(Restitution::new(-1.0).coefficient, 0.0);
    }

    #[test]
    fn default_is_zero() {
        assert_eq!(Restitution::default().coefficient, 0.0);
    }

    #[test]
    fn combine_takes_maximum() {
        let a = Restitution::new(0.3);
        let b = Restitution::new(0.8);
        assert!((a.combine(&b) - 0.8).abs() < 0.001);
    }

    #[test]
    fn combine_identical() {
        let r = Restitution::new(0.6);
        assert!((r.combine(&r) - 0.6).abs() < 0.001);
    }
}
