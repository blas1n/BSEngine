use bevy_ecs::prelude::Component;
use glam::Vec4;

/// RGBA color in linear [0, 1] space. Use as a component to tint a mesh entity.
/// The render pipeline multiplies vertex/material color by this value.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for Color {
    fn default() -> Self {
        Self::WHITE
    }
}

impl Color {
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const RED: Self = Self {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Self = Self {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Linear interpolation between two colors.
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    pub fn to_vec4(self) -> Vec4 {
        Vec4::new(self.r, self.g, self.b, self.a)
    }
}

impl From<Color> for Vec4 {
    fn from(c: Color) -> Vec4 {
        c.to_vec4()
    }
}

impl From<[f32; 4]> for Color {
    fn from([r, g, b, a]: [f32; 4]) -> Self {
        Self { r, g, b, a }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_is_white() {
        assert_eq!(Color::default(), Color::WHITE);
    }

    #[test]
    fn rgb_sets_alpha_one() {
        let c = Color::rgb(0.5, 0.3, 0.1);
        assert_eq!(c.a, 1.0);
    }

    #[test]
    fn with_alpha_overrides_alpha() {
        let c = Color::RED.with_alpha(0.5);
        assert_eq!(c.r, 1.0);
        assert_eq!(c.a, 0.5);
    }

    #[test]
    fn lerp_midpoint() {
        let c = Color::BLACK.lerp(Color::WHITE, 0.5);
        assert!((c.r - 0.5).abs() < 0.001);
        assert!((c.g - 0.5).abs() < 0.001);
        assert!((c.b - 0.5).abs() < 0.001);
    }

    #[test]
    fn lerp_t0_returns_self() {
        let c = Color::RED.lerp(Color::BLUE, 0.0);
        assert_eq!(c, Color::RED);
    }

    #[test]
    fn lerp_t1_returns_other() {
        let c = Color::RED.lerp(Color::BLUE, 1.0);
        assert_eq!(c, Color::BLUE);
    }

    #[test]
    fn to_vec4_maps_components() {
        let v = Color::rgba(0.1, 0.2, 0.3, 0.4).to_vec4();
        assert!((v.x - 0.1).abs() < 0.001);
        assert!((v.y - 0.2).abs() < 0.001);
        assert!((v.z - 0.3).abs() < 0.001);
        assert!((v.w - 0.4).abs() < 0.001);
    }

    #[test]
    fn from_array() {
        let c = Color::from([0.1, 0.2, 0.3, 0.4]);
        assert_eq!(c.r, 0.1);
    }

    #[test]
    fn into_vec4() {
        let v: Vec4 = Color::WHITE.into();
        assert_eq!(v, Vec4::ONE);
    }
}
