use bevy_ecs::prelude::Component;
use glam::Vec2;

use crate::Color;

#[derive(Component, Debug, Clone, PartialEq)]
pub struct Sprite {
    pub color: Color,
    pub flip_x: bool,
    pub flip_y: bool,
    pub custom_size: Option<Vec2>,
    pub anchor: Vec2,
}

impl Default for Sprite {
    fn default() -> Self {
        Self {
            color: Color::WHITE,
            flip_x: false,
            flip_y: false,
            custom_size: None,
            anchor: Vec2::ZERO,
        }
    }
}

impl Sprite {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    pub fn with_flip_x(mut self, flip_x: bool) -> Self {
        self.flip_x = flip_x;
        self
    }

    pub fn with_flip_y(mut self, flip_y: bool) -> Self {
        self.flip_y = flip_y;
        self
    }

    pub fn with_custom_size(mut self, size: Vec2) -> Self {
        self.custom_size = Some(size);
        self
    }

    pub fn with_anchor(mut self, anchor: Vec2) -> Self {
        self.anchor = anchor;
        self
    }
}

/// A single frame within a texture atlas (sprite sheet).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextureAtlas {
    pub index: usize,
    pub columns: usize,
    pub rows: usize,
}

impl TextureAtlas {
    pub fn new(index: usize, columns: usize, rows: usize) -> Self {
        Self {
            index: index.min(columns * rows - 1),
            columns,
            rows,
        }
    }

    /// UV min/max for the current frame: (min, max) both in [0,1].
    pub fn uv_rect(&self) -> (Vec2, Vec2) {
        let col = self.index % self.columns;
        let row = self.index / self.columns;
        let cell_w = 1.0 / self.columns as f32;
        let cell_h = 1.0 / self.rows as f32;
        let min = Vec2::new(col as f32 * cell_w, row as f32 * cell_h);
        let max = min + Vec2::new(cell_w, cell_h);
        (min, max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sprite_default_white_no_flip() {
        let s = Sprite::default();
        assert_eq!(s.color, Color::WHITE);
        assert!(!s.flip_x);
        assert!(!s.flip_y);
        assert_eq!(s.custom_size, None);
        assert_eq!(s.anchor, Vec2::ZERO);
    }

    #[test]
    fn sprite_builder_sets_color_and_flip() {
        let s = Sprite::new().with_color(Color::RED).with_flip_x(true);
        assert_eq!(s.color, Color::RED);
        assert!(s.flip_x);
        assert!(!s.flip_y);
    }

    #[test]
    fn sprite_custom_size() {
        let size = Vec2::new(64.0, 32.0);
        let s = Sprite::new().with_custom_size(size);
        assert_eq!(s.custom_size, Some(size));
    }

    #[test]
    fn texture_atlas_clamps_index() {
        let atlas = TextureAtlas::new(99, 4, 4); // max valid index = 15
        assert_eq!(atlas.index, 15);
    }

    #[test]
    fn texture_atlas_uv_rect_first_frame() {
        let atlas = TextureAtlas::new(0, 4, 4);
        let (min, max) = atlas.uv_rect();
        assert!((min.x).abs() < 0.001);
        assert!((min.y).abs() < 0.001);
        assert!((max.x - 0.25).abs() < 0.001);
        assert!((max.y - 0.25).abs() < 0.001);
    }

    #[test]
    fn texture_atlas_uv_rect_frame_5() {
        // 4 columns → row=1, col=1
        let atlas = TextureAtlas::new(5, 4, 4);
        let (min, _) = atlas.uv_rect();
        assert!((min.x - 0.25).abs() < 0.001); // col 1
        assert!((min.y - 0.25).abs() < 0.001); // row 1
    }
}
