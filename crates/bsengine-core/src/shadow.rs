use bevy_ecs::prelude::Component;

/// Algorithm used to soften shadow edges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowFilterMethod {
    /// Nearest-neighbour (hard shadows).
    None,
    /// Percentage-closer filtering — balanced quality/cost.
    Pcf,
    /// Percentage-closer soft shadows — high quality, higher cost.
    Pcss,
}

/// Controls shadow casting and receiving on a mesh entity.
#[derive(Component, Debug, Clone, PartialEq)]
pub struct Shadow {
    /// Whether this entity casts shadows onto other geometry.
    pub cast: bool,
    /// Whether this entity receives shadows from other casters.
    pub receive: bool,
    pub filter: ShadowFilterMethod,
    /// Bias offset to reduce self-shadowing artefacts. Larger = less acne, more peter-panning.
    pub bias: f32,
    /// Normal bias to further reduce surface acne on steep angles.
    pub normal_bias: f32,
    /// Maximum distance from the camera at which this entity casts shadows.
    /// `None` = no limit (shadows as far as the shadow map covers).
    pub cast_distance: Option<f32>,
    pub enabled: bool,
}

impl Shadow {
    pub fn new() -> Self {
        Self {
            cast: true,
            receive: true,
            filter: ShadowFilterMethod::Pcf,
            bias: 0.002,
            normal_bias: 0.6,
            cast_distance: None,
            enabled: true,
        }
    }

    pub fn caster_only() -> Self {
        Self {
            receive: false,
            ..Self::new()
        }
    }

    pub fn receiver_only() -> Self {
        Self {
            cast: false,
            ..Self::new()
        }
    }

    pub fn hard() -> Self {
        Self {
            filter: ShadowFilterMethod::None,
            ..Self::new()
        }
    }

    pub fn with_filter(mut self, filter: ShadowFilterMethod) -> Self {
        self.filter = filter;
        self
    }

    pub fn with_bias(mut self, bias: f32) -> Self {
        self.bias = bias.max(0.0);
        self
    }

    pub fn with_normal_bias(mut self, bias: f32) -> Self {
        self.normal_bias = bias.max(0.0);
        self
    }

    pub fn with_cast_distance(mut self, distance: f32) -> Self {
        self.cast_distance = Some(distance.max(0.0));
        self
    }

    pub fn disabled(mut self) -> Self {
        self.enabled = false;
        self
    }

    pub fn is_effective(&self) -> bool {
        self.enabled && (self.cast || self.receive)
    }
}

impl Default for Shadow {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_casts_and_receives() {
        let s = Shadow::new();
        assert!(s.cast);
        assert!(s.receive);
        assert!(s.is_effective());
    }

    #[test]
    fn caster_only_does_not_receive() {
        let s = Shadow::caster_only();
        assert!(s.cast);
        assert!(!s.receive);
    }

    #[test]
    fn receiver_only_does_not_cast() {
        let s = Shadow::receiver_only();
        assert!(!s.cast);
        assert!(s.receive);
    }

    #[test]
    fn disabled_not_effective() {
        let s = Shadow::new().disabled();
        assert!(!s.is_effective());
    }

    #[test]
    fn cast_distance_set() {
        let s = Shadow::new().with_cast_distance(50.0);
        assert_eq!(s.cast_distance, Some(50.0));
    }
}
