use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Controls whether an entity is drawn by the render system.
/// Entities without this component are treated as visible.
#[derive(Component, Debug, Clone, PartialEq, Eq, Reflect)]
#[reflect(Component, Default)]
pub struct Visible {
    /// Whether the entity is currently drawn.
    pub is_visible: bool,
}

impl Default for Visible {
    fn default() -> Self {
        Self { is_visible: true }
    }
}

impl Visible {
    /// Creates a `Visible` component that hides the entity.
    pub fn hidden() -> Self {
        Self { is_visible: false }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_visible() {
        assert!(Visible::default().is_visible);
    }

    #[test]
    fn hidden_is_not_visible() {
        assert!(!Visible::hidden().is_visible);
    }

    #[test]
    fn visible_equality() {
        assert_eq!(Visible::default(), Visible { is_visible: true });
        assert_ne!(Visible::default(), Visible::hidden());
    }
}
