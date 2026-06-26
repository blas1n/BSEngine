use bevy_ecs::prelude::Component;

use crate::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextAlignment {
    #[default]
    Left,
    Center,
    Right,
    Justified,
}

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Text {
    pub value: String,
    pub font_size: f32,
    pub color: Color,
    pub alignment: TextAlignment,
    pub line_height: f32,
}

impl Text {
    pub fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            font_size: 16.0,
            color: Color::WHITE,
            alignment: TextAlignment::Left,
            line_height: 1.2,
        }
    }

    pub fn with_font_size(mut self, size: f32) -> Self {
        self.font_size = size.max(0.0);
        self
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_alignment(mut self, alignment: TextAlignment) -> Self {
        self.alignment = alignment;
        self
    }

    pub fn with_line_height(mut self, line_height: f32) -> Self {
        self.line_height = line_height.max(0.0);
        self
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }
}

impl Default for Text {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_defaults() {
        let t = Text::new("Hello");
        assert_eq!(t.value, "Hello");
        assert!((t.font_size - 16.0).abs() < 0.001);
        assert_eq!(t.color, Color::WHITE);
        assert_eq!(t.alignment, TextAlignment::Left);
        assert!((t.line_height - 1.2).abs() < 0.001);
    }

    #[test]
    fn text_builder() {
        let t = Text::new("Hi")
            .with_font_size(32.0)
            .with_color(Color::BLACK)
            .with_alignment(TextAlignment::Center)
            .with_line_height(1.5);
        assert!((t.font_size - 32.0).abs() < 0.001);
        assert_eq!(t.alignment, TextAlignment::Center);
        assert!((t.line_height - 1.5).abs() < 0.001);
    }

    #[test]
    fn text_font_size_clamped_to_zero() {
        let t = Text::new("x").with_font_size(-10.0);
        assert_eq!(t.font_size, 0.0);
    }

    #[test]
    fn text_is_empty() {
        assert!(Text::new("").is_empty());
        assert!(!Text::new("hi").is_empty());
    }
}
