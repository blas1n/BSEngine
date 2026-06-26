/// How keyframe values are interpolated between keyframe times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    Linear,
    Step,
    CubicSpline,
}

/// The animated values for a single channel.
///
/// Translations and Scales are `[x, y, z]`.
/// Rotations are `[x, y, z, w]` (quaternion).
#[derive(Debug, Clone)]
pub enum KeyframeValues {
    Translations(Vec<[f32; 3]>),
    Rotations(Vec<[f32; 4]>),
    Scales(Vec<[f32; 3]>),
}

/// One property animated over time, targeting a specific GLTF node by index.
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    /// Index of the target GLTF node (matches the node order in the file).
    pub node_index: usize,
    /// Keyframe timestamps in seconds.
    pub times: Vec<f32>,
    pub values: KeyframeValues,
    pub interpolation: Interpolation,
}

/// A named sequence of animation channels loaded from a GLTF file.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    pub name: String,
    pub channels: Vec<AnimationChannel>,
    /// Duration in seconds (= time of the last keyframe across all channels).
    pub duration: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn animation_clip_stores_duration() {
        let clip = AnimationClip {
            name: "Walk".to_string(),
            channels: vec![],
            duration: 1.5,
        };
        assert!((clip.duration - 1.5).abs() < 1e-6);
        assert_eq!(clip.name, "Walk");
    }

    #[test]
    fn animation_channel_translation_values() {
        let channel = AnimationChannel {
            node_index: 0,
            times: vec![0.0, 0.5, 1.0],
            values: KeyframeValues::Translations(vec![
                [0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0],
            ]),
            interpolation: Interpolation::Linear,
        };
        assert_eq!(channel.times.len(), 3);
        if let KeyframeValues::Translations(ref t) = channel.values {
            assert_eq!(t[1], [0.0, 1.0, 0.0]);
        } else {
            panic!("expected Translations");
        }
    }

    #[test]
    fn interpolation_variants_are_distinct() {
        assert_ne!(Interpolation::Linear, Interpolation::Step);
        assert_ne!(Interpolation::Step, Interpolation::CubicSpline);
    }
}
