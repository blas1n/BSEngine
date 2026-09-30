//! Root motion: moving a character by what its animation's root bone does.

use crate::ReflectVec3;
use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// Takes the horizontal travel and the turning of an animation's root bone
/// out of the pose and gives them to the entity instead: a walk clip that
/// moves its hips forward moves the character forward, at exactly the pace
/// its feet plant, and a clip that turns the hips turns the character.
///
/// Unity's Apply Root Motion, Unreal's root motion and Godot's
/// `root_motion_track` share this: pick the bone, read how far it moved this
/// frame, remove that from the pose, apply it to the object. Like Unity's
/// default ("bake Y into pose"), vertical motion stays in the pose -- a jump
/// or a bob still plays -- and only the horizontal travel moves the entity.
///
/// Turning is yaw only -- rotation about the model's up axis, measured in
/// the model's frame through every armature node above the bone (Blender
/// exports a -90° X), as Unity and Unreal extract it; the bone's lean and
/// roll stay in the pose, as the vertical bob does. With it on, each frame's
/// travel is measured in the root's own facing, so a clip that walks an arc
/// moves the character along the arc once, turning as it goes -- not turned
/// by the extraction *and* again by the path.
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
    /// Extract the root's yaw too, turning the entity; off leaves the
    /// turning in the pose (the mesh turns in place, the entity does not),
    /// Unity's "Root Transform Rotation: Bake Into Pose".
    pub apply_rotation: bool,
    /// The world-space travel of the last frame. Output; writing it does
    /// nothing.
    pub last_delta: ReflectVec3,
    /// The last frame's turn about the entity's up axis, in radians
    /// (positive turns left, counter-clockwise seen from above). Output.
    pub last_rotation_delta: f32,
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
            apply_rotation: true,
            last_delta: glam::Vec3::ZERO.into(),
            last_rotation_delta: 0.0,
            last_sample: None,
        }
    }
}
