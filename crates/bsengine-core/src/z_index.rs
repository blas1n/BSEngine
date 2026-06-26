use bevy_ecs::prelude::Component;

/// Render draw order for an entity. Higher values are drawn later (on top).
/// Entities without ZIndex are treated as ZIndex(0).
/// The render pipeline sorts draw calls by this value before submission.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ZIndex(pub i32);

impl Default for ZIndex {
    fn default() -> Self {
        Self(0)
    }
}

impl ZIndex {
    pub fn new(order: i32) -> Self {
        Self(order)
    }

    pub fn value(self) -> i32 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_zero() {
        assert_eq!(ZIndex::default().value(), 0);
    }

    #[test]
    fn new_stores_value() {
        assert_eq!(ZIndex::new(5).value(), 5);
    }

    #[test]
    fn negative_z_index() {
        assert_eq!(ZIndex::new(-10).value(), -10);
    }

    #[test]
    fn ordering() {
        assert!(ZIndex::new(1) > ZIndex::new(0));
        assert!(ZIndex::new(-1) < ZIndex::new(0));
    }

    #[test]
    fn equality() {
        assert_eq!(ZIndex::new(3), ZIndex(3));
    }
}
