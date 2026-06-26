use bevy_ecs::prelude::Component;

/// How network authority is assigned for this entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NetworkAuthority {
    /// The server owns simulation authority; clients receive state updates.
    #[default]
    Server,
    /// A specific client (by peer ID) owns simulation authority.
    Client { peer_id: u64 },
    /// No network replication — local-only entity.
    Local,
}

/// Unique network identity for a replicated entity.
/// The networking layer uses this to match server entities to client entities
/// and route state updates to the correct ECS entity.
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetworkId {
    /// Globally unique, stable identifier assigned at spawn time.
    pub id: u64,
    pub authority: NetworkAuthority,
}

impl NetworkId {
    /// Create a server-authoritative network identity.
    pub fn server(id: u64) -> Self {
        Self {
            id,
            authority: NetworkAuthority::Server,
        }
    }

    /// Create a client-authoritative identity owned by `peer_id`.
    pub fn client(id: u64, peer_id: u64) -> Self {
        Self {
            id,
            authority: NetworkAuthority::Client { peer_id },
        }
    }

    /// Create a non-replicated identity (local-only).
    pub fn local(id: u64) -> Self {
        Self {
            id,
            authority: NetworkAuthority::Local,
        }
    }

    /// Returns `true` if this entity is owned by the given peer.
    pub fn is_owned_by(&self, peer_id: u64) -> bool {
        matches!(self.authority, NetworkAuthority::Client { peer_id: p } if p == peer_id)
    }

    /// Returns `true` if this entity replicates across the network.
    pub fn is_replicated(&self) -> bool {
        !matches!(self.authority, NetworkAuthority::Local)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_authority_defaults() {
        let n = NetworkId::server(42);
        assert_eq!(n.id, 42);
        assert_eq!(n.authority, NetworkAuthority::Server);
        assert!(n.is_replicated());
    }

    #[test]
    fn client_authority_ownership() {
        let n = NetworkId::client(1, 7);
        assert!(n.is_owned_by(7));
        assert!(!n.is_owned_by(3));
    }

    #[test]
    fn local_not_replicated() {
        let n = NetworkId::local(99);
        assert!(!n.is_replicated());
    }

    #[test]
    fn server_not_owned_by_peer() {
        let n = NetworkId::server(1);
        assert!(!n.is_owned_by(0));
    }

    #[test]
    fn client_is_replicated() {
        let n = NetworkId::client(5, 2);
        assert!(n.is_replicated());
    }
}
