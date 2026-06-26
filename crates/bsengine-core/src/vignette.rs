use bevy_ecs::prelude::Component;

/// Screen-space vignette post-processing applied by a camera.
/// Darkens the edges of the screen, drawing the eye to the centre.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Vignette {
    /// Strength of the vignette. 0 = invisible, 1 = fully black at the furthest edges.
    pub intensity: f32,
    /// How quickly the darkening falls off from the edges inward.
    /// Higher values produce a sharper, smaller vignette ring.
    pub smoothness: f32,
    /// Color of the vignette overlay (usually black but can be tinted).
    pub color: [f32; 3],
    pub enabled: bool,
}

impl Vignette {
    pub fn new(intensity: f32) -> Self {
        Self {
            intensity: intensity.clamp(0.0, 1.0),
            smoothness: 0.5,
            color: [0.0, 0.0, 0.0],
            enabled: true,
        }
    }

    pub fn with_smoothness(mut self, smoothness: f32) -> Self {
        self.smoothness = smoothness.max(0.0);
        self
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.color = [r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0)];
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for Vignette {
    fn default() -> Self {
        Self::new(0.5)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vignette_defaults() {
        let v = Vignette::default();
        assert!((v.intensity - 0.5).abs() < 0.001);
        assert!((v.smoothness - 0.5).abs() < 0.001);
        assert_eq!(v.color, [0.0, 0.0, 0.0]);
        assert!(v.enabled);
    }

    #[test]
    fn intensity_clamped() {
        let v = Vignette::new(3.0);
        assert!((v.intensity - 1.0).abs() < 0.001);
    }

    #[test]
    fn smoothness_clamped() {
        let v = Vignette::default().with_smoothness(-1.0);
        assert_eq!(v.smoothness, 0.0);
    }

    #[test]
    fn color_channels_clamped() {
        let v = Vignette::default().with_color(-1.0, 2.0, 0.5);
        assert_eq!(v.color, [0.0, 1.0, 0.5]);
    }

    #[test]
    fn disabled_flag() {
        let v = Vignette::new(0.5).disabled();
        assert!(!v.enabled);
    }
}
