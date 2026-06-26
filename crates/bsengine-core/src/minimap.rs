use bevy_ecs::prelude::Component;

/// Marks an entity as visible on the minimap with an icon and color.
/// The minimap system reads the entity's world-space position and this component
/// each frame to place and draw the icon on the HUD overlay.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Minimap {
    /// Path to the icon sprite shown at this entity's position.
    /// Empty string = use the default category icon.
    pub icon: String,
    /// Icon tint as linear RGBA. `[1, 1, 1, 1]` = no tint.
    pub color: [f32; 4],
    /// Icon size in minimap UI pixels at 1:1 zoom.
    pub size: f32,
    /// Logical category shown in the minimap legend (e.g. `"enemy"`, `"ally"`, `"objective"`).
    pub category: String,
    /// When true the icon rotates with the entity's yaw.
    pub rotate_with_entity: bool,
    /// When true the icon is clamped to the minimap border when off-screen.
    pub clamp_to_edge: bool,
    pub enabled: bool,
}

impl Minimap {
    pub fn new(category: impl Into<String>) -> Self {
        Self {
            icon: String::new(),
            color: [1.0, 1.0, 1.0, 1.0],
            size: 8.0,
            category: category.into(),
            rotate_with_entity: false,
            clamp_to_edge: false,
            enabled: true,
        }
    }

    pub fn with_icon(mut self, path: impl Into<String>) -> Self {
        self.icon = path.into();
        self
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

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size.max(0.0);
        self
    }

    pub fn rotating(mut self) -> Self {
        self.rotate_with_entity = true;
        self
    }

    pub fn clamped_to_edge(mut self) -> Self {
        self.clamp_to_edge = true;
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimap_defaults() {
        let mm = Minimap::new("player");
        assert_eq!(mm.category, "player");
        assert!(mm.icon.is_empty());
        assert_eq!(mm.color, [1.0, 1.0, 1.0, 1.0]);
        assert!((mm.size - 8.0).abs() < 0.001);
        assert!(!mm.rotate_with_entity);
        assert!(mm.enabled);
    }

    #[test]
    fn color_clamped() {
        let mm = Minimap::new("enemy").with_color(-1.0, 2.0, 0.5, 1.0);
        assert_eq!(mm.color[0], 0.0);
        assert_eq!(mm.color[1], 1.0);
    }

    #[test]
    fn size_clamped() {
        let mm = Minimap::new("item").with_size(-5.0);
        assert_eq!(mm.size, 0.0);
    }

    #[test]
    fn rotating_flag() {
        let mm = Minimap::new("tank").rotating();
        assert!(mm.rotate_with_entity);
    }

    #[test]
    fn clamp_to_edge_flag() {
        let mm = Minimap::new("objective").clamped_to_edge();
        assert!(mm.clamp_to_edge);
    }
}
