//! The two resources the networking and scripting layers use to hand input to
//! each other.
//!
//! # Why they live here
//!
//! `bsengine-network` needs the local player's held keys (to send them) and
//! needs to publish a remote player's held keys (so a script can read them).
//! `bsengine-scripting` owns the key-name conversion and the per-entity input
//! its ops read. Neither crate depends on the other, and neither should: they
//! meet in `bsengine-core`, which both already depend on.
//!
//! This is the same shape item 52 used for the ragdoll blend weight, where the
//! producer could not name the consumer's type.

use std::collections::HashMap;

use bevy_ecs::prelude::Resource;

/// Key names held down on this machine this frame.
///
/// Published by the scripting layer, which already converts key codes to the
/// names scripts use, so there is exactly one such conversion in the engine
/// rather than a second one that could drift from it.
#[derive(Resource, Default, Debug, Clone)]
pub struct LocalHeldKeys(pub Vec<String>);

/// Key names held down by the owner of each remotely-driven entity, keyed by
/// **network id**.
///
/// Published by the networking layer from received input, and read by the
/// scripting layer, which resolves the id to an entity name — it already queries
/// `(Entity, &Name, &NetworkId)` together, and `Name` lives in `bsengine-scene`,
/// which the networking crate does not and should not depend on. Keying by the
/// one identifier both layers already share is what keeps that dependency out.
///
/// An entity absent from here has no remote owner and falls back to the local
/// keyboard — which is every entity in a single-player game, and is what makes
/// this addition invisible to them.
#[derive(Resource, Default, Debug, Clone)]
pub struct RemoteHeldKeys(pub HashMap<u64, Vec<String>>);
