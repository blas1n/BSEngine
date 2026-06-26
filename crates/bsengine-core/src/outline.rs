use bevy_ecs::prelude::Component;

use crate::Color;

/// How the outline is positioned relative to the mesh edge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutlineMode {
    /// Outline extends outward from the mesh boundary.
    #[default]
    Outer,
    /// Outline is drawn inside the mesh boundary.
    Inner,
    /// Outline is centered on the mesh boundary.
    Center,
}

/// Renders a colored outline around a mesh.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Outline {
    pub color: Color,
    /// Width in pixels.
    pub width: f32,
    pub mode: OutlineMode,
    pub visible: bool,
}

impl Outline {
    pub fn new(color: Color, width: f32) -> Self {
        Self {
            color,
            width: width.max(0.0),
            mode: OutlineMode::Outer,
            visible: true,
        }
    }

    pub fn with_mode(mut self, mode: OutlineMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn hidden(mut self) -> Self {
        self.visible = false;
        self
    }
}

impl Default for Outline {
    fn default() -> Self {
        Self::new(Color::WHITE, 1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_defaults() {
        let o = Outline::default();
        assert_eq!(o.color, Color::WHITE);
        assert!((o.width - 1.0).abs() < 0.001);
        assert_eq!(o.mode, OutlineMode::Outer);
        assert!(o.visible);
    }

    #[test]
    fn outline_negative_width_clamped() {
        let o = Outline::new(Color::BLACK, -5.0);
        assert_eq!(o.width, 0.0);
    }

    #[test]
    fn outline_mode_builder() {
        let o = Outline::new(Color::WHITE, 2.0).with_mode(OutlineMode::Inner);
        assert_eq!(o.mode, OutlineMode::Inner);
    }

    #[test]
    fn outline_hidden_builder() {
        let o = Outline::default().hidden();
        assert!(!o.visible);
    }
}
