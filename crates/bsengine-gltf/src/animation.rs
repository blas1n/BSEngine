/// How keyframe values are interpolated between keyframe times.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// Straight-line interpolation between adjacent keyframes.
    Linear,
    /// Hold the previous keyframe's value until the next keyframe time.
    Step,
    /// Cubic Hermite spline interpolation using in/out tangents.
    CubicSpline,
}

/// The animated values for a single channel.
///
/// Translations and Scales are `[x, y, z]`.
/// Rotations are `[x, y, z, w]` (quaternion).
#[derive(Debug, Clone)]
pub enum KeyframeValues {
    /// Per-keyframe translation vectors.
    Translations(Vec<[f32; 3]>),
    /// Per-keyframe rotation quaternions.
    Rotations(Vec<[f32; 4]>),
    /// Per-keyframe scale vectors.
    Scales(Vec<[f32; 3]>),
}

/// One property animated over time, targeting a specific GLTF node by index.
#[derive(Debug, Clone)]
pub struct AnimationChannel {
    /// Index of the target GLTF node (matches the node order in the file).
    pub node_index: usize,
    /// Keyframe timestamps in seconds.
    pub times: Vec<f32>,
    /// The animated values, one per keyframe time.
    pub values: KeyframeValues,
    /// How to interpolate between the keyframe values.
    pub interpolation: Interpolation,
}

/// A named sequence of animation channels loaded from a GLTF file.
#[derive(Debug, Clone)]
pub struct AnimationClip {
    /// The clip's name, as given in the GLTF file (or a fallback if unnamed).
    pub name: String,
    /// The animated channels that make up this clip.
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
