use bevy_ecs::prelude::Component;

/// Depth-of-field post-processing applied by a camera.
/// Objects outside the focal range are blurred proportionally to their distance from it.
#[derive(Component, Debug, Clone, Copy, PartialEq)]
pub struct DepthOfField {
    /// Distance in world units to the sharpest focus plane.
    pub focal_distance: f32,
    /// Half-width of the in-focus band around `focal_distance`.
    /// Objects within [focal_distance - range, focal_distance + range] are fully sharp.
    pub focal_range: f32,
    /// Maximum blur radius (in pixels) applied to fully out-of-focus areas.
    pub max_blur: f32,
    /// Relative scale of the bokeh blur behind the focal plane vs in front.
    /// 1.0 = symmetric, >1.0 = more blur behind (far), <1.0 = more blur in front (near).
    pub bokeh_scale: f32,
    pub enabled: bool,
}

impl DepthOfField {
    pub fn new(focal_distance: f32) -> Self {
        Self {
            focal_distance: focal_distance.max(0.0),
            focal_range: 1.0,
            max_blur: 8.0,
            bokeh_scale: 1.0,
            enabled: true,
        }
    }

    pub fn with_focal_range(mut self, range: f32) -> Self {
        self.focal_range = range.max(0.0);
        self
    }

    pub fn with_max_blur(mut self, max_blur: f32) -> Self {
        self.max_blur = max_blur.max(0.0);
        self
    }

    pub fn with_bokeh_scale(mut self, scale: f32) -> Self {
        self.bokeh_scale = scale.max(0.0);
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Returns the blur strength [0.0, 1.0] for an object at `object_distance` world units.
    pub fn blur_factor(&self, object_distance: f32) -> f32 {
        let diff = (object_distance - self.focal_distance).abs();
        let out_of_focus = (diff - self.focal_range).max(0.0);
        (out_of_focus / self.max_blur.max(1.0)).clamp(0.0, 1.0)
    }
}

impl Default for DepthOfField {
    fn default() -> Self {
        Self::new(10.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dof_defaults() {
        let d = DepthOfField::default();
        assert!((d.focal_distance - 10.0).abs() < 0.001);
        assert!((d.focal_range - 1.0).abs() < 0.001);
        assert!((d.max_blur - 8.0).abs() < 0.001);
        assert!(d.enabled);
    }

    #[test]
    fn dof_in_focus_no_blur() {
        let d = DepthOfField::new(10.0).with_focal_range(2.0);
        assert_eq!(d.blur_factor(10.0), 0.0); // at focal point
        assert_eq!(d.blur_factor(11.5), 0.0); // within range
    }

    #[test]
    fn dof_out_of_focus_increases_blur() {
        let d = DepthOfField::new(10.0)
            .with_focal_range(0.0)
            .with_max_blur(10.0);
        let bf = d.blur_factor(15.0);
        assert!(bf > 0.0 && bf <= 1.0);
    }

    #[test]
    fn dof_negative_focal_distance_clamped() {
        let d = DepthOfField::new(-5.0);
        assert_eq!(d.focal_distance, 0.0);
    }

    #[test]
    fn dof_disabled() {
        let d = DepthOfField::default().disabled();
        assert!(!d.enabled);
    }
}
