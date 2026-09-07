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

/// One predicted entity that needs correcting, and the input to re-apply.
///
/// # Why a request rather than a direct call
///
/// The networking layer is what learns that a correction is due, but replaying
/// means **re-running the entity's movement script**, and only the scripting
/// layer can do that. Neither crate depends on the other, so the correction
/// travels as data through here — the same route the input takes in the other
/// direction.
#[derive(Debug, Clone)]
pub struct ReplayRequest {
    /// Which entity, by network id.
    pub net_id: u64,
    /// Where the server says it actually was, as of the last input it applied.
    pub authoritative: crate::Transform,
    /// The inputs the server had not yet seen, oldest first, to re-apply on top.
    ///
    /// Empty means the server has caught up with everything sent, in which case
    /// the correction is just the snap.
    pub replay: Vec<Vec<String>>,
}

/// Corrections waiting for the scripting layer to apply.
///
/// Drained every frame. A correction left here would be applied twice and move
/// the entity twice as far, so draining is not optional bookkeeping.
#[derive(Resource, Default, Debug, Clone)]
pub struct PendingReplays(pub Vec<ReplayRequest>);
