//! Network session and wire-protocol layer for BSEngine.
//!
//! `NetworkPlugin` manages a `NetworkSession` with a `NetworkRole`
//! (host/client), and the (private) `packet` module defines the
//! `TransformData` wire format used to replicate entity transforms across
//! the network.
//!
//! # Who is authoritative today
//!
//! A client simulates its own entities and sends their transforms; the server
//! relays them. That is **client-authoritative**, and it is why there is no
//! reconciliation here: the server never disagrees with a client about a client's
//! own entity, so there is nothing to reconcile. Server-authoritative simulation
//! with client-side prediction is item 56's second sub-step, and it is an opt-in
//! per entity rather than a replacement, so this model keeps working.
//!
//! What this half adds is everything on the server→client path: a tick on the
//! wire, snapshot interpolation so remote entities move smoothly when packets
//! arrive unevenly, distance-based interest management, and a deterministic
//! latency/loss simulator — which is less a feature than the instrument that
//! makes the other two observable at all.
#![warn(missing_docs)]

mod config;
mod interpolation;
mod packet;
mod plugin;
mod session;
mod sim;

pub use config::NetworkConfig;
pub use interpolation::{sample, SnapshotBuffers};
pub use plugin::{NetworkPlugin, ServerTick};
pub use session::{NetworkRole, NetworkSession};
pub use sim::{DelayQueue, LinkSimulator};
