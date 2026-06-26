use bevy_ecs::prelude::Component;

/// Linear damping applied to `Velocity.linear` each frame.
/// Formula: `velocity *= (1.0 - linear * dt).max(0.0)`.
/// `linear = 0.0` → no damping. `linear = 2.0` → ~50% speed loss per second.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Damping {
    pub linear: f32,
}

impl Default for Damping {
    fn default() -> Self {
        Self { linear: 0.0 }
    }
}

impl Damping {
    pub fn new(linear: f32) -> Self {
        Self { linear }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(Damping::default().linear, 0.0);
    }

    #[test]
    fn new_stores_value() {
        assert_eq!(Damping::new(2.5).linear, 2.5);
    }

    #[test]
    fn negative_damping_allowed() {
        // Negative damping = acceleration; users may want this.
        assert_eq!(Damping::new(-1.0).linear, -1.0);
    }
}
