use bevy_ecs::prelude::Component;

/// Makes a mesh surface emit light independently of scene lighting.
/// The renderer additively blends the emissive contribution on top of the lit surface.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Emissive {
    /// HDR color of the emitted light. Values > 1.0 are valid (HDR).
    pub color: [f32; 4],
    /// Overall intensity multiplier. 0.0 = off; 1.0 = use color as-is; > 1.0 = super-bright.
    pub intensity: f32,
    /// When true, the emissive contribution is also injected into the bloom pass.
    pub contributes_to_bloom: bool,
    pub enabled: bool,
}

impl Emissive {
    pub fn new(color: [f32; 4], intensity: f32) -> Self {
        Self {
            color,
            intensity: intensity.max(0.0),
            contributes_to_bloom: true,
            enabled: true,
        }
    }

    /// Convenience: white emissive at the given intensity.
    pub fn white(intensity: f32) -> Self {
        Self::new([1.0, 1.0, 1.0, 1.0], intensity)
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0);
        self
    }

    pub fn without_bloom(mut self) -> Self {
        self.contributes_to_bloom = false;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Final HDR color after intensity is applied.
    pub fn hdr_color(&self) -> [f32; 4] {
        [
            self.color[0] * self.intensity,
            self.color[1] * self.intensity,
            self.color[2] * self.intensity,
            self.color[3],
        ]
    }
}

impl Default for Emissive {
    fn default() -> Self {
        Self::white(1.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emissive_defaults() {
        let e = Emissive::default();
        assert_eq!(e.color, [1.0, 1.0, 1.0, 1.0]);
        assert!((e.intensity - 1.0).abs() < 0.001);
        assert!(e.contributes_to_bloom);
        assert!(e.enabled);
    }

    #[test]
    fn hdr_color_applies_intensity() {
        let e = Emissive::new([0.5, 0.5, 0.5, 1.0], 2.0);
        let hdr = e.hdr_color();
        assert!((hdr[0] - 1.0).abs() < 0.001);
        assert!((hdr[3] - 1.0).abs() < 0.001);
    }

    #[test]
    fn intensity_clamped_to_zero() {
        let e = Emissive::default().with_intensity(-5.0);
        assert_eq!(e.intensity, 0.0);
    }

    #[test]
    fn without_bloom() {
        let e = Emissive::white(1.0).without_bloom();
        assert!(!e.contributes_to_bloom);
    }

    #[test]
    fn disabled_flag() {
        let e = Emissive::default().disabled();
        assert!(!e.enabled);
    }
}
