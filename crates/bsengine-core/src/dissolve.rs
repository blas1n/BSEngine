use bevy_ecs::prelude::Component;

/// Controls a clip-noise dissolve shader effect on the mesh material.
/// At `progress == 0` the mesh is fully visible; at `progress == 1` it is fully
/// dissolved (every pixel clipped). The material system feeds `progress` and the
/// edge parameters to the shader each frame.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Dissolve {
    /// Dissolve progress in [0, 1]. 0 = visible, 1 = fully dissolved.
    pub progress: f32,
    /// Width of the glowing burn edge in UV space. 0 = sharp clip, no edge.
    pub edge_width: f32,
    /// Edge glow colour as linear RGBA.
    pub edge_color: [f32; 4],
    /// Noise texture tiling factor. Higher = finer grain.
    pub noise_scale: f32,
    pub enabled: bool,
}

impl Dissolve {
    /// Create a dissolve at the given initial progress (0 = visible, 1 = gone).
    pub fn new(progress: f32) -> Self {
        Self {
            progress: progress.clamp(0.0, 1.0),
            edge_width: 0.05,
            edge_color: [1.0, 0.4, 0.0, 1.0], // orange glow
            noise_scale: 5.0,
            enabled: true,
        }
    }

    pub fn with_edge_width(mut self, width: f32) -> Self {
        self.edge_width = width.max(0.0);
        self
    }

    pub fn with_edge_color(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.edge_color = [
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            a.clamp(0.0, 1.0),
        ];
        self
    }

    pub fn with_noise_scale(mut self, scale: f32) -> Self {
        self.noise_scale = scale.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns `true` when fully dissolved (`progress >= 1`).
    pub fn is_dissolved(&self) -> bool {
        self.progress >= 1.0
    }
}

impl Default for Dissolve {
    fn default() -> Self {
        Self::new(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dissolve_defaults() {
        let d = Dissolve::default();
        assert_eq!(d.progress, 0.0);
        assert!((d.edge_width - 0.05).abs() < 0.001);
        assert!(!d.is_dissolved());
        assert!(d.enabled);
    }

    #[test]
    fn progress_clamped() {
        let d = Dissolve::new(2.0);
        assert!((d.progress - 1.0).abs() < 0.001);
        assert!(d.is_dissolved());
    }

    #[test]
    fn edge_color_clamped() {
        let d = Dissolve::new(0.5).with_edge_color(-1.0, 2.0, 0.5, 1.0);
        assert_eq!(d.edge_color[0], 0.0);
        assert_eq!(d.edge_color[1], 1.0);
    }

    #[test]
    fn noise_scale_clamped() {
        let d = Dissolve::new(0.0).with_noise_scale(-3.0);
        assert_eq!(d.noise_scale, 0.0);
    }

    #[test]
    fn disabled_flag() {
        let d = Dissolve::new(0.0).disabled();
        assert!(!d.enabled);
    }
}
