use bevy_ecs::prelude::{Component, Resource};

/// How the skybox texture is laid out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SkyboxProjection {
    /// Equirectangular (lat-long) panorama — single 2:1 image.
    #[default]
    Equirectangular,
    /// Six separate faces packed into a cross layout (horizontal or vertical).
    Cubemap,
}

/// Resource that holds the active skybox image path.
/// Set by scene loading or script `setSkybox(path)`.
/// The render system reads this and loads the texture when the path changes.
#[derive(Resource, Default, Clone)]
pub struct SkyboxPath(pub Option<String>);

/// Renders an environment background behind all geometry for a camera entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Skybox {
    /// Asset path to the HDR/equirectangular or cubemap texture.
    pub path: String,
    /// Brightness multiplier applied to the skybox.
    pub intensity: f32,
    /// Y-axis rotation of the skybox in radians.
    pub rotation: f32,
    pub projection: SkyboxProjection,
    pub enabled: bool,
}

impl Skybox {
    pub fn new(path: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            intensity: 1.0,
            rotation: 0.0,
            projection: SkyboxProjection::Equirectangular,
            enabled: true,
        }
    }

    pub fn with_intensity(mut self, intensity: f32) -> Self {
        self.intensity = intensity.max(0.0);
        self
    }

    pub fn with_rotation(mut self, radians: f32) -> Self {
        self.rotation = radians;
        self
    }

    pub fn with_projection(mut self, projection: SkyboxProjection) -> Self {
        self.projection = projection;
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
    fn skybox_defaults() {
        let s = Skybox::new("env/day.hdr");
        assert_eq!(s.path, "env/day.hdr");
        assert!((s.intensity - 1.0).abs() < 0.001);
        assert_eq!(s.rotation, 0.0);
        assert_eq!(s.projection, SkyboxProjection::Equirectangular);
        assert!(s.enabled);
    }

    #[test]
    fn skybox_intensity_clamped() {
        let s = Skybox::new("env/night.hdr").with_intensity(-1.0);
        assert_eq!(s.intensity, 0.0);
    }

    #[test]
    fn skybox_cubemap_projection() {
        let s = Skybox::new("env/cube.hdr").with_projection(SkyboxProjection::Cubemap);
        assert_eq!(s.projection, SkyboxProjection::Cubemap);
    }

    #[test]
    fn skybox_disabled() {
        let s = Skybox::new("env/day.hdr").disabled();
        assert!(!s.enabled);
    }
}
