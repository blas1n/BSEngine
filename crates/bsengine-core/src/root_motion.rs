//! Root motion: moving a character by what its animation's root bone does.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Takes the horizontal travel of an animation's root bone out of the pose
/// and gives it to the entity instead: a walk clip that moves its hips
/// forward moves the character forward, at exactly the pace its feet plant.
///
/// Unity's Apply Root Motion, Unreal's root motion and Godot's
/// `root_motion_track` share this: pick the bone, read how far it moved this
/// frame, remove that from the pose, apply it to the object. Like Unity's
/// default ("bake Y into pose"), vertical motion stays in the pose -- a jump
/// or a bob still plays -- and only the horizontal travel moves the entity.
///
/// **Translation only.** The bone's turning is not extracted; a turning clip
/// turns the mesh in place. glTF skeletons put their root under armature
/// nodes with their own axes (Blender exports a -90° X), and yaw has to be
/// measured in the model's frame through all of them; translation is done
/// that way here, rotation is left for later.
///
/// With `apply_to_transform` off, the travel is only reported, in
/// [`Self::last_delta`] -- for a script that wants to route it through a
/// `CharacterController` (`moveCharacter`) so walls stop it, the way Unity
/// hands root motion to `CharacterController.Move`.
#[derive(Component, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component, Default)]
pub struct RootMotion {
    /// The bone whose travel moves the entity, by glTF node name; empty for
    /// the skin's first joint, which is its root in every exporter seen.
    pub bone: String,
    /// Add the travel to the entity's `Transform`. Off: only report it.
    pub apply_to_transform: bool,
    /// The world-space travel of the last frame. Output; writing it does
    /// nothing.
    pub last_delta: ReflectVec3,
    /// The clip and time the last delta was measured up to -- the next
    /// frame's starting point. Internal; `None` until the first frame, and
    /// reset by a clip change, which starts the count again rather than
    /// measuring a jump between two different clips.
    #[reflect(ignore)]
    pub last_sample: Option<(String, f32)>,
}

impl Default for RootMotion {
    fn default() -> Self {
        Self {
            bone: String::new(),
            apply_to_transform: true,
            last_delta: glam::Vec3::ZERO.into(),
            last_sample: None,
        }
    }
}
