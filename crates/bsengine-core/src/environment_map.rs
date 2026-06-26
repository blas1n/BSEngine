use bevy_ecs::prelude::Component;

/// Image-based lighting (IBL) environment map for indirect lighting and reflections.
/// Attach to a camera or light entity; the render system samples the cubemaps each frame.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct EnvironmentMap {
    /// Path to the diffuse irradiance cubemap (low-res, blurred for diffuse GI).
    pub diffuse_path: String,
    /// Path to the prefiltered specular radiance cubemap (mipmapped for roughness).
    pub specular_path: String,
    /// Overall intensity multiplier for the environment lighting.
    pub intensity: f32,
    /// Rotation offset in radians applied to both cubemaps around the Y axis.
    pub rotation: f32,
    pub enabled: bool,
}

impl EnvironmentMap {
    pub fn new(diffuse_path: impl Into<String>, specular_path: impl Into<String>) -> Self {
        Self {
            diffuse_path: diffuse_path.into(),
            specular_path: specular_path.into(),
            intensity: 1.0,
            rotation: 0.0,
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

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn environment_map_defaults() {
        let em = EnvironmentMap::new("diffuse.ktx2", "specular.ktx2");
        assert_eq!(em.diffuse_path, "diffuse.ktx2");
        assert_eq!(em.specular_path, "specular.ktx2");
        assert!((em.intensity - 1.0).abs() < 0.001);
        assert_eq!(em.rotation, 0.0);
        assert!(em.enabled);
    }

    #[test]
    fn intensity_clamped() {
        let em = EnvironmentMap::new("a", "b").with_intensity(-1.0);
        assert_eq!(em.intensity, 0.0);
    }

    #[test]
    fn rotation_stored() {
        let em = EnvironmentMap::new("a", "b").with_rotation(1.57);
        assert!((em.rotation - 1.57).abs() < 0.001);
    }

    #[test]
    fn disabled_flag() {
        let em = EnvironmentMap::new("a", "b").disabled();
        assert!(!em.enabled);
    }
}
