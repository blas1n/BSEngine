//! Network session and wire-protocol layer for BSEngine.
//!
//! `NetworkPlugin` manages a `NetworkSession` with a `NetworkRole`
//! (host/client), and the (private) `packet` module defines the
//! `TransformData` wire format used to replicate entity transforms across
//! the network.
#![warn(missing_docs)]

mod packet;
mod plugin;
mod session;

pub use plugin::NetworkPlugin;
pub use session::{NetworkRole, NetworkSession};
