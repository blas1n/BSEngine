use bevy_ecs::prelude::Resource;
use std::net::{SocketAddr, UdpSocket};

/// Whether this process acts as a server or client.
#[derive(Debug)]
pub enum NetworkRole {
    /// Authoritative host: accepts client connections and broadcasts entity state.
    Server,
    /// Connects to a remote server and sends only its own client-authoritative transforms.
    Client {
        /// Address of the server this client is connected (or connecting) to.
        server_addr: SocketAddr,
    },
}

/// Live network session — inserted as a Bevy Resource when networking is active.
#[derive(Resource)]
pub struct NetworkSession {
    /// Whether this session is acting as the server or as a client.
    pub role: NetworkRole,
    /// Non-blocking UDP socket used for all send/receive traffic.
    pub socket: UdpSocket,
    /// Server: list of connected client addresses. Client: [server_addr].
    pub peers: Vec<SocketAddr>,
    /// Server is always 0. Clients receive their ID in HELLO_ACK.
    pub my_peer_id: u64,
    /// True once the handshake completes (server always true; client after HELLO_ACK).
    pub connected: bool,
    /// Monotonically-increasing counter the server uses to assign peer IDs.
    pub peer_id_counter: u64,
}

impl NetworkSession {
    /// Binds a non-blocking UDP socket on all interfaces at `port` and starts a server session.
    pub fn new_server(port: u16) -> Result<Self, std::io::Error> {
        let socket = UdpSocket::bind(format!("0.0.0.0:{port}"))?;
        socket.set_nonblocking(true)?;
        tracing::info!("[network] server listening on port {port}");
        Ok(Self {
            role: NetworkRole::Server,
            socket,
            peers: Vec::new(),
            my_peer_id: 0,
            connected: true,
            peer_id_counter: 1,
        })
    }

    /// Binds an ephemeral local UDP socket, sends a HELLO to `host:port`, and starts a client session.
    pub fn new_client(host: &str, port: u16) -> Result<Self, std::io::Error> {
        let server_addr: SocketAddr = format!("{host}:{port}").parse().map_err(|_| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "invalid server address")
        })?;
        let socket = UdpSocket::bind("0.0.0.0:0")?;
        socket.set_nonblocking(true)?;
        let _ = socket.send_to(&crate::packet::encode_hello(), server_addr);
        tracing::info!("[network] client connecting to {server_addr}");
        Ok(Self {
            role: NetworkRole::Client { server_addr },
            socket,
            peers: vec![server_addr],
            my_peer_id: 0,
            connected: false,
            peer_id_counter: 0,
        })
    }

    /// Returns true if this session is acting as the server.
    pub fn is_server(&self) -> bool {
        matches!(self.role, NetworkRole::Server)
    }

    /// Number of known peers (connected clients for a server, or the server itself for a client).
    pub fn peer_count(&self) -> u32 {
        self.peers.len() as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_session_is_server() {
        let s = NetworkSession::new_server(0).unwrap();
        assert!(s.is_server());
        assert!(s.connected);
        assert_eq!(s.my_peer_id, 0);
        assert_eq!(s.peer_count(), 0);
    }

    #[test]
    fn client_session_is_not_server() {
        let srv = UdpSocket::bind("127.0.0.1:0").unwrap();
        let port = srv.local_addr().unwrap().port();
        let c = NetworkSession::new_client("127.0.0.1", port).unwrap();
        assert!(!c.is_server());
        assert!(!c.connected);
        assert_eq!(c.peer_count(), 1);
    }

    #[test]
    fn peer_count_reflects_peers() {
        let mut s = NetworkSession::new_server(0).unwrap();
        assert_eq!(s.peer_count(), 0);
        s.peers.push("127.0.0.1:9000".parse().unwrap());
        assert_eq!(s.peer_count(), 1);
    }
}
