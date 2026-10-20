//! Which moving renderers realtime GI leaves out.
//!
//! A realtime light-probe volume traces every renderer in the scene: the
//! [`crate::LightmapStatic`] ones as one tree, rebuilt when that set changes,
//! and every other one as an instance that follows its transform each frame,
//! so a door that swings or a crate that slides darkens and lights the room
//! as it goes. That is the default Lumen and RTXGI ship, and Unreal's
//! per-primitive "Affect Dynamic Indirect Lighting" (on by default) is its
//! switch. This is that switch, turned off: for what should not take part --
//! a muzzle flash, a particle quad, a first-person view model sitting on the
//! camera, which would otherwise darken the probes around the player.
//!
//! (Unity's Enlighten and Godot's SDFGI take the other default -- only static
//! objects -- which leaves realtime GI blind to everything that moves.)

use bevy_ecs::prelude::{Component, ReflectComponent};
use bevy_reflect::prelude::ReflectDefault;
use bevy_reflect::Reflect;

/// This entity's mesh takes no part in realtime GI: it neither blocks nor
/// bounces the light probes see. It is still lit by them.
///
/// Only moving renderers are traced this way; a [`crate::LightmapStatic`]
/// one has opted in, and this marker beside it is ignored.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq, Reflect)]
#[reflect(Component, Default)]
pub struct ExcludeFromGi;
