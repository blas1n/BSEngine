use bevy_ecs::prelude::Component;

/// How an entity should be oriented to face the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BillboardMode {
    /// Rotates around both X and Y axes — always faces the camera directly.
    #[default]
    Full,
    /// Rotates only around the Y axis — stays upright (e.g. trees, characters).
    Vertical,
}

/// Marker that tells the render system to orient this entity toward the active camera.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct Billboard {
    pub mode: BillboardMode,
}

impl Billboard {
    pub fn full() -> Self {
        Self {
            mode: BillboardMode::Full,
        }
    }

    pub fn vertical() -> Self {
        Self {
            mode: BillboardMode::Vertical,
        }
    }

    pub fn is_full(&self) -> bool {
        self.mode == BillboardMode::Full
    }

    pub fn is_vertical(&self) -> bool {
        self.mode == BillboardMode::Vertical
    }
}

impl Default for Billboard {
    fn default() -> Self {
        Self::full()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_billboard_default() {
        let b = Billboard::default();
        assert!(b.is_full());
        assert!(!b.is_vertical());
    }

    #[test]
    fn vertical_billboard() {
        let b = Billboard::vertical();
        assert!(b.is_vertical());
        assert!(!b.is_full());
    }

    #[test]
    fn mode_equality() {
        assert_eq!(Billboard::full().mode, BillboardMode::Full);
        assert_eq!(Billboard::vertical().mode, BillboardMode::Vertical);
    }
}
