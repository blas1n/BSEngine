//! Remote procedure calls in flight, in both directions.
//!
//! The network layer moves them and the scripting layer runs them, and neither
//! crate depends on the other. They meet here, the same way replay requests and
//! remote held keys already do.
//!
//! # What a call is attached to
//!
//! An entity, named by its [`NetworkId`](crate::NetworkId). Unity's RPCs live on
//! a `NetworkBehaviour`, Unreal's on an `Actor`, Godot's on a `Node` — all three
//! route by the object the call was made on, because that object is also what
//! decides who is allowed to make it and who should receive it. A free-floating
//! "send this string to everyone" would need a second answer to both questions.

use bevy_ecs::prelude::Resource;

/// Who runs a call.
///
/// The same three Unreal spells as `Server` / `Client` / `NetMulticast`, and the
/// same three Unity reaches with `SendTo.Server` / `SendTo.Owner` /
/// `SendTo.ClientsAndHost`. Godot expresses them as a peer id instead, but the
/// three cases games actually use are these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RpcTarget {
    /// Runs on the server. Sent by the client that owns the entity.
    Server,
    /// Runs on the one client that owns the entity. Sent by the server.
    Owner,
    /// Runs on every client, and on the server too.
    ///
    /// Running on the sender as well is Unreal's `NetMulticast` behaviour, and
    /// it is what makes a multicast usable for anything with a visible effect:
    /// a server that skipped its own copy would be the one machine that never
    /// saw the explosion it announced.
    Multicast,
}

/// One call, as it travels.
///
/// The arguments are an opaque string rather than typed fields. The scripting
/// layer is the only thing that reads them, it is dynamically typed, and giving
/// the transport a schema it cannot check would be a claim it is in no position
/// to make — Godot passes Variants here for the same reason.
#[derive(Debug, Clone, PartialEq)]
pub struct RpcCall {
    /// The networked entity the call is attached to.
    pub net_id: u64,
    /// The handler's name, as registered.
    pub name: String,
    /// The arguments, serialised by whoever made the call.
    pub args: String,
}

/// A call waiting to go out, with the routing it was declared with.
#[derive(Debug, Clone, PartialEq)]
pub struct OutgoingRpc {
    /// The call itself.
    pub call: RpcCall,
    /// Who should run it.
    pub target: RpcTarget,
    /// Whether losing it is acceptable.
    ///
    /// Reliable by default in every one of the three engines, and for a reason
    /// that survives the copying: an RPC is said once, so a lost one is not
    /// repaired by the next packet the way a lost transform snapshot is.
    /// Unreliable is the opt-out, for calls sent every frame where the newest
    /// supersedes the last.
    pub reliable: bool,
}

/// Calls queued to send and calls that have arrived.
///
/// Two plain vectors rather than channels: both ends are Bevy systems on the
/// same schedule, so an ordinary resource is enough and a channel would add a
/// second ordering to reason about.
#[derive(Resource, Default, Debug)]
pub struct RpcQueues {
    /// Queued by whoever makes a call; drained by the network layer.
    pub outgoing: Vec<OutgoingRpc>,
    /// Pushed by the network layer; drained by whoever runs handlers.
    pub incoming: Vec<RpcCall>,
}

impl RpcQueues {
    /// Queues a call to be sent.
    pub fn send(&mut self, call: RpcCall, target: RpcTarget, reliable: bool) {
        self.outgoing.push(OutgoingRpc {
            call,
            target,
            reliable,
        });
    }

    /// Takes every arrived call, leaving the queue empty.
    ///
    /// Draining rather than reading: a call left behind would run again on the
    /// next frame, and an RPC that fires twice is exactly what the reliable
    /// channel's deduplication exists to prevent one layer down.
    pub fn take_incoming(&mut self) -> Vec<RpcCall> {
        std::mem::take(&mut self.incoming)
    }

    /// Takes every queued call, leaving the queue empty.
    pub fn take_outgoing(&mut self) -> Vec<OutgoingRpc> {
        std::mem::take(&mut self.outgoing)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn call(name: &str) -> RpcCall {
        RpcCall {
            net_id: 7,
            name: name.into(),
            args: "{}".into(),
        }
    }

    #[test]
    fn a_queued_call_keeps_its_routing() {
        // Distinct targets deliberately: were they the same, a queue that
        // dropped the target and defaulted would look correct.
        let mut queues = RpcQueues::default();
        queues.send(call("a"), RpcTarget::Server, true);
        queues.send(call("b"), RpcTarget::Multicast, false);

        let out = queues.take_outgoing();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].target, RpcTarget::Server);
        assert!(out[0].reliable);
        assert_eq!(out[1].target, RpcTarget::Multicast);
        assert!(!out[1].reliable, "the opt-out has to survive the queue");
    }

    #[test]
    fn taking_a_queue_empties_it() {
        let mut queues = RpcQueues::default();
        queues.send(call("a"), RpcTarget::Server, true);
        queues.incoming.push(call("b"));

        assert_eq!(queues.take_outgoing().len(), 1);
        assert!(
            queues.take_outgoing().is_empty(),
            "a call sent twice is a call that happened twice"
        );
        assert_eq!(queues.take_incoming().len(), 1);
        assert!(
            queues.take_incoming().is_empty(),
            "and a handler must not run again every frame"
        );
    }
}
