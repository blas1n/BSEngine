use bevy_ecs::prelude::Component;

/// Renders the entity's mesh as a water surface with wave animation and SSR.
/// Expects a flat mesh on the XZ plane; the shader displaces vertices each frame.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct WaterBody {
    /// Base water color as linear RGBA.
    pub color: [f32; 4],
    /// Physically-based depth color applied below `depth_fade` metres.
    pub deep_color: [f32; 4],
    /// Distance in world units below the surface at which `deep_color` takes over.
    pub depth_fade: f32,
    /// Wave height (amplitude) in world units.
    pub wave_height: f32,
    /// Wave speed multiplier. 1 = natural, 2 = twice as fast.
    pub wave_speed: f32,
    /// Water surface roughness [0, 1]. 0 = perfect mirror.
    pub roughness: f32,
    /// Screen-space reflection intensity [0, 1].
    pub ssr_intensity: f32,
    pub enabled: bool,
}

impl WaterBody {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.color = [
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        ];
        self
    }

    pub fn with_deep_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.deep_color = [
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        ];
        self
    }

    pub fn with_depth_fade(mut self, distance: f32) -> Self {
        self.depth_fade = distance.max(0.0);
        self
    }

    pub fn with_wave_height(mut self, height: f32) -> Self {
        self.wave_height = height.max(0.0);
        self
    }

    pub fn with_wave_speed(mut self, speed: f32) -> Self {
        self.wave_speed = speed.max(0.0);
        self
    }

    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.clamp(0.0, 1.0);
        self
    }

    pub fn with_ssr_intensity(mut self, intensity: f32) -> Self {
        self.ssr_intensity = intensity.clamp(0.0, 1.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

impl Default for WaterBody {
    fn default() -> Self {
        Self {
            color: [0.05, 0.33, 0.48, 0.85],
            deep_color: [0.01, 0.10, 0.22, 1.0],
            depth_fade: 5.0,
            wave_height: 0.15,
            wave_speed: 1.0,
            roughness: 0.05,
            ssr_intensity: 0.8,
            enabled: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_body_defaults() {
        let wb = WaterBody::default();
        assert!((wb.wave_height - 0.15).abs() < 0.001);
        assert!((wb.wave_speed - 1.0).abs() < 0.001);
        assert!((wb.roughness - 0.05).abs() < 0.001);
        assert!(wb.enabled);
    }

    #[test]
    fn roughness_clamped() {
        let wb = WaterBody::new().with_roughness(2.0);
        assert!((wb.roughness - 1.0).abs() < 0.001);
    }

    #[test]
    fn ssr_intensity_clamped() {
        let wb = WaterBody::new().with_ssr_intensity(-1.0);
        assert_eq!(wb.ssr_intensity, 0.0);
    }

    #[test]
    fn wave_height_clamped() {
        let wb = WaterBody::new().with_wave_height(-5.0);
        assert_eq!(wb.wave_height, 0.0);
    }

    #[test]
    fn color_channels_clamped() {
        let wb = WaterBody::new().with_color(-1.0, 2.0, 0.5, 1.0);
        assert_eq!(wb.color[0], 0.0);
        assert_eq!(wb.color[1], 1.0);
    }
}
