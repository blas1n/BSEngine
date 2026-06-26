use bevy_ecs::prelude::Component;
use glam::Vec2;

/// Named anchor presets for common 2-D layout positions.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum AnchorPreset {
    /// (0.5, 0.5) — dead center.
    #[default]
    Center,
    TopLeft,
    TopCenter,
    TopRight,
    MiddleLeft,
    MiddleRight,
    BottomLeft,
    BottomCenter,
    BottomRight,
    /// Fully custom normalized position.
    Custom(Vec2),
}

impl AnchorPreset {
    /// Returns the normalized [0,1] position corresponding to this preset.
    /// Origin is bottom-left; (1,1) is top-right.
    pub fn normalized(&self) -> Vec2 {
        match self {
            Self::Center => Vec2::new(0.5, 0.5),
            Self::TopLeft => Vec2::new(0.0, 1.0),
            Self::TopCenter => Vec2::new(0.5, 1.0),
            Self::TopRight => Vec2::new(1.0, 1.0),
            Self::MiddleLeft => Vec2::new(0.0, 0.5),
            Self::MiddleRight => Vec2::new(1.0, 0.5),
            Self::BottomLeft => Vec2::new(0.0, 0.0),
            Self::BottomCenter => Vec2::new(0.5, 0.0),
            Self::BottomRight => Vec2::new(1.0, 0.0),
            Self::Custom(v) => *v,
        }
    }
}

/// Sets the pivot/anchor point for a 2-D or UI entity.
/// The layout system positions the entity so that its `anchor` point lands on
/// the parent's `anchor` normalized coordinate.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub preset: AnchorPreset,
    /// Pixel offset applied after the anchor position is resolved.
    pub offset: Vec2,
}

impl Anchor {
    pub fn new(preset: AnchorPreset) -> Self {
        Self {
            preset,
            offset: Vec2::ZERO,
        }
    }

    pub fn with_offset(mut self, offset: Vec2) -> Self {
        self.offset = offset;
        self
    }

    /// Normalized [0,1] anchor position (before offset).
    pub fn normalized(&self) -> Vec2 {
        self.preset.normalized()
    }
}

impl Default for Anchor {
    fn default() -> Self {
        Self::new(AnchorPreset::Center)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_center_default() {
        let a = Anchor::default();
        let n = a.normalized();
        assert!((n.x - 0.5).abs() < 0.001);
        assert!((n.y - 0.5).abs() < 0.001);
    }

    #[test]
    fn anchor_top_left() {
        let a = Anchor::new(AnchorPreset::TopLeft);
        let n = a.normalized();
        assert_eq!(n, Vec2::new(0.0, 1.0));
    }

    #[test]
    fn anchor_bottom_right() {
        let a = Anchor::new(AnchorPreset::BottomRight);
        let n = a.normalized();
        assert_eq!(n, Vec2::new(1.0, 0.0));
    }

    #[test]
    fn custom_anchor() {
        let pos = Vec2::new(0.25, 0.75);
        let a = Anchor::new(AnchorPreset::Custom(pos));
        assert_eq!(a.normalized(), pos);
    }

    #[test]
    fn offset_stored() {
        let a = Anchor::default().with_offset(Vec2::new(10.0, -5.0));
        assert_eq!(a.offset, Vec2::new(10.0, -5.0));
    }
}
