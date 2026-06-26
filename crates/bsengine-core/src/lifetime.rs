use bevy_ecs::prelude::Component;

/// Automatically despawns an entity when `remaining` reaches zero.
/// Decrement is driven by `LifetimePlugin` using `Res<Time>.delta_seconds`.
#[derive(Component, Debug, Clone)]
pub struct Lifetime {
    pub remaining: f32,
}

impl Lifetime {
    pub fn from_seconds(seconds: f32) -> Self {
        Self {
            remaining: seconds.max(0.0),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.remaining <= 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_seconds_stores_value() {
        let l = Lifetime::from_seconds(2.5);
        assert!((l.remaining - 2.5).abs() < 1e-6);
    }

    #[test]
    fn from_seconds_clamps_negative_to_zero() {
        let l = Lifetime::from_seconds(-1.0);
        assert!((l.remaining - 0.0).abs() < 1e-6);
    }

    #[test]
    fn is_expired_when_zero() {
        let l = Lifetime { remaining: 0.0 };
        assert!(l.is_expired());
    }

    #[test]
    fn is_not_expired_when_positive() {
        let l = Lifetime::from_seconds(0.1);
        assert!(!l.is_expired());
    }
}
