use bevy_ecs::prelude::Component;

/// Defines the region of the screen a camera renders into, using normalized [0.0, 1.0] coordinates.
/// Origin is the top-left corner of the window.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Viewport {
    /// Left edge (0.0 = left of window, 1.0 = right).
    pub x: f32,
    /// Top edge (0.0 = top of window, 1.0 = bottom).
    pub y: f32,
    /// Width as a fraction of the window width.
    pub width: f32,
    /// Height as a fraction of the window height.
    pub height: f32,
}

impl Viewport {
    /// Full-screen viewport — renders to the entire window.
    pub fn full_screen() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            width: 1.0,
            height: 1.0,
        }
    }

    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width: width.max(0.0),
            height: height.max(0.0),
        }
    }

    /// Aspect ratio (width / height). Returns 0 if height is zero.
    pub fn aspect_ratio(&self) -> f32 {
        if self.height == 0.0 {
            0.0
        } else {
            self.width / self.height
        }
    }

    /// Returns `true` if the normalized point (px, py) falls within this viewport.
    pub fn contains(&self, px: f32, py: f32) -> bool {
        px >= self.x && px <= self.x + self.width && py >= self.y && py <= self.y + self.height
    }

    /// Split into left (width_fraction) and right viewports.
    pub fn split_horizontal(self, width_fraction: f32) -> (Self, Self) {
        let fraction = width_fraction.clamp(0.0, 1.0);
        let left_w = self.width * fraction;
        let right_w = self.width * (1.0 - fraction);
        let left = Self::new(self.x, self.y, left_w, self.height);
        let right = Self::new(self.x + left_w, self.y, right_w, self.height);
        (left, right)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::full_screen()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_screen_default() {
        let v = Viewport::full_screen();
        assert_eq!(v.x, 0.0);
        assert_eq!(v.y, 0.0);
        assert_eq!(v.width, 1.0);
        assert_eq!(v.height, 1.0);
    }

    #[test]
    fn aspect_ratio() {
        let v = Viewport::new(0.0, 0.0, 0.5, 1.0);
        assert!((v.aspect_ratio() - 0.5).abs() < 0.001);
    }

    #[test]
    fn aspect_ratio_zero_height() {
        let v = Viewport::new(0.0, 0.0, 1.0, 0.0);
        assert_eq!(v.aspect_ratio(), 0.0);
    }

    #[test]
    fn contains_point() {
        let v = Viewport::new(0.5, 0.0, 0.5, 1.0);
        assert!(v.contains(0.75, 0.5));
        assert!(!v.contains(0.25, 0.5));
    }

    #[test]
    fn split_horizontal_half() {
        let v = Viewport::full_screen();
        let (left, right) = v.split_horizontal(0.5);
        assert!((left.width - 0.5).abs() < 0.001);
        assert!((right.x - 0.5).abs() < 0.001);
        assert!((right.width - 0.5).abs() < 0.001);
    }

    #[test]
    fn negative_dimensions_clamped() {
        let v = Viewport::new(0.0, 0.0, -1.0, -1.0);
        assert_eq!(v.width, 0.0);
        assert_eq!(v.height, 0.0);
    }
}
