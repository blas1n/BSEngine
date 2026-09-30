//! Morph target (blend shape) weights for a mesh.

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// How much of each of its mesh's morph targets a model shows: a smile at
/// 0.7, a blink at 1.0. Unity's blend shapes, Unreal's morph targets and
/// Godot's blend shapes are the same mechanism -- per-vertex offsets stored
/// with the mesh, mixed in by weight before the skeleton bends it -- and all
/// three address them by name and by index, as this does.
///
/// Inserted by the glTF loader on a model whose (first) mesh has targets,
/// with the names the file gives (`extras.targetNames`, the de-facto
/// convention every exporter writes) and the mesh's default weights. Change
/// [`Self::weights`] to change the shape; the change reaches the GPU the
/// same frame. Scripts use `Bsengine.setMorphWeight`.
#[derive(Component, Debug, Clone, PartialEq, Default, Reflect)]
#[reflect(Component, Default)]
pub struct MorphWeights {
    /// One name per target, in target order; empty strings for targets the
    /// file did not name.
    pub names: Vec<String>,
    /// One weight per target, in target order. 0 is the base shape, 1 the
    /// whole target; values outside are allowed and extrapolate, as in every
    /// engine.
    pub weights: Vec<f32>,
}

impl MorphWeights {
    /// The index of the target called `name`, if any.
    pub fn index_of(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n == name)
    }
}
