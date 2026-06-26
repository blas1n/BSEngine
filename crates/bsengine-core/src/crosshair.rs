use bevy_ecs::prelude::Component;

/// Visual style of the crosshair shape drawn on-screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CrosshairStyle {
    /// Classic four-line cross with a centre gap.
    #[default]
    Cross,
    /// Circular ring.
    Circle,
    /// Cross and circle combined.
    CrossCircle,
    /// Single dot only.
    Dot,
}

/// Screen-space crosshair attached to a camera or player entity.
/// The UI system reads this to draw the aiming reticle each frame.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct Crosshair {
    pub style: CrosshairStyle,
    /// Base RGBA color of the crosshair lines.
    pub color: [f32; 4],
    /// Outer size of the crosshair in screen pixels.
    pub size: f32,
    /// Width of each line in pixels.
    pub thickness: f32,
    /// Gap between the centre and the start of the lines, in pixels.
    /// Expands dynamically with `spread` to show weapon inaccuracy.
    pub gap: f32,
    /// Additional spread in pixels added by the game (e.g. from movement or recoil).
    /// The UI adds this to `gap` for the rendered gap.
    pub spread: f32,
    /// Maximum `spread` value — clamped when set via `add_spread`.
    pub max_spread: f32,
    /// Rate at which `spread` returns to 0 each second.
    pub spread_decay: f32,
    pub enabled: bool,
}

impl Crosshair {
    pub fn new(style: CrosshairStyle) -> Self {
        Self {
            style,
            color: [1.0, 1.0, 1.0, 1.0],
            size: 24.0,
            thickness: 2.0,
            gap: 4.0,
            spread: 0.0,
            max_spread: 32.0,
            spread_decay: 20.0,
            enabled: true,
        }
    }

    pub fn with_color(mut self, color: [f32; 4]) -> Self {
        self.color = color;
        self
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Instantaneous effective gap in pixels (base + current spread).
    pub fn effective_gap(&self) -> f32 {
        self.gap + self.spread
    }

    /// Add spread (recoil, movement) clamped to `max_spread`.
    pub fn add_spread(&mut self, amount: f32) {
        self.spread = (self.spread + amount).min(self.max_spread);
    }

    /// Decay spread toward 0 by `dt` seconds.
    pub fn tick(&mut self, dt: f32) {
        self.spread = (self.spread - self.spread_decay * dt).max(0.0);
    }
}

impl Default for Crosshair {
    fn default() -> Self {
        Self::new(CrosshairStyle::Cross)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crosshair_defaults() {
        let c = Crosshair::default();
        assert_eq!(c.style, CrosshairStyle::Cross);
        assert!((c.size - 24.0).abs() < 0.001);
        assert_eq!(c.spread, 0.0);
        assert!(c.enabled);
    }

    #[test]
    fn effective_gap_includes_spread() {
        let mut c = Crosshair::default();
        c.add_spread(10.0);
        assert!((c.effective_gap() - 14.0).abs() < 0.001);
    }

    #[test]
    fn spread_clamped_to_max() {
        let mut c = Crosshair::default();
        c.add_spread(100.0);
        assert_eq!(c.spread, c.max_spread);
    }

    #[test]
    fn spread_decays_to_zero() {
        let mut c = Crosshair::default();
        c.add_spread(10.0);
        c.tick(1.0); // decay 20/s → 0
        assert_eq!(c.spread, 0.0);
    }

    #[test]
    fn disabled_crosshair() {
        let c = Crosshair::new(CrosshairStyle::Dot).disabled();
        assert!(!c.enabled);
        assert_eq!(c.style, CrosshairStyle::Dot);
    }
}
