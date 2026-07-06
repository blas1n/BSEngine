use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::{NetworkAuthority, NetworkId, Transform};

use crate::{
    packet::{
        encode_client_transform, encode_transform_batch, TransformData, MSG_CLIENT_TRANSFORM,
        MSG_DISCONNECT, MSG_HELLO, MSG_HELLO_ACK, MSG_TRANSFORM_BATCH,
    },
    session::{NetworkRole, NetworkSession},
};

pub struct NetworkPlugin;

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, network_receive_system);
        app.add_systems(Update, network_send_system.after(network_receive_system));
    }
}

/// Drain incoming UDP packets and apply state changes.
fn network_receive_system(world: &mut World) {
    let mut buf = [0u8; 8192];

    // Collect all incoming packets without holding a borrow on world.
    let packets: Vec<(Vec<u8>, std::net::SocketAddr)> = {
        let Some(session) = world.get_resource::<NetworkSession>() else {
            return;
        };
        let mut packets = Vec::new();
        loop {
            match session.socket.recv_from(&mut buf) {
                Ok((n, addr)) => packets.push((buf[..n].to_vec(), addr)),
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(_) => break,
            }
        }
        packets
    };

    for (data, addr) in packets {
        if data.is_empty() {
            continue;
        }
        match data[0] {
            MSG_HELLO => {
                // Server: assign peer_id, register peer, send ACK.
                if let Some(mut session) = world.get_resource_mut::<NetworkSession>() {
                    if session.is_server() {
                        let peer_id = session.peer_id_counter;
                        session.peer_id_counter += 1;
                        if !session.peers.contains(&addr) {
                            session.peers.push(addr);
                        }
                        let ack = crate::packet::encode_hello_ack(peer_id);
                        let _ = session.socket.send_to(&ack, addr);
                        tracing::debug!("[network] client {addr} assigned peer_id={peer_id}");
                    }
                }
            }
            MSG_HELLO_ACK => {
                // Client: store assigned peer_id, mark connected.
                if data.len() >= 9 {
                    let peer_id = u64::from_le_bytes(data[1..9].try_into().unwrap_or([0; 8]));
                    if let Some(mut session) = world.get_resource_mut::<NetworkSession>() {
                        session.my_peer_id = peer_id;
                        session.connected = true;
                        tracing::debug!("[network] connected as peer_id={peer_id}");
                    }
                }
            }
            MSG_TRANSFORM_BATCH => {
                // Client: apply server-broadcast transform snapshot.
                if data.len() < 2 {
                    continue;
                }
                let count = data[1] as usize;
                let mut offset = 2;
                for _ in 0..count {
                    if offset + 48 > data.len() {
                        break;
                    }
                    let net_id =
                        u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap_or([0; 8]));
                    let td = TransformData::from_bytes(&data[offset + 8..offset + 48]);
                    offset += 48;

                    let entity = {
                        let mut q = world.query::<(Entity, &NetworkId)>();
                        q.iter(world)
                            .find(|(_, nid)| nid.id == net_id)
                            .map(|(e, _)| e)
                    };
                    if let Some(e) = entity {
                        if let Some(mut t) = world.get_mut::<Transform>(e) {
                            *t = td.to_transform();
                        }
                    }
                }
            }
            MSG_CLIENT_TRANSFORM => {
                // Server: apply client-authoritative transform.
                if data.len() < 49 {
                    continue;
                }
                let net_id = u64::from_le_bytes(data[1..9].try_into().unwrap_or([0; 8]));
                let td = TransformData::from_bytes(&data[9..49]);

                let entity = {
                    let mut q = world.query::<(Entity, &NetworkId)>();
                    q.iter(world)
                        .find(|(_, nid)| nid.id == net_id)
                        .map(|(e, _)| e)
                };
                if let Some(e) = entity {
                    if let Some(mut t) = world.get_mut::<Transform>(e) {
                        *t = td.to_transform();
                    }
                }
            }
            MSG_DISCONNECT => {
                if let Some(mut session) = world.get_resource_mut::<NetworkSession>() {
                    session.peers.retain(|p| *p != addr);
                    tracing::debug!("[network] peer {addr} disconnected");
                }
            }
            _ => {}
        }
    }
}

/// Broadcast/send transforms based on role.
fn network_send_system(world: &mut World) {
    // Collect entity data first (releases all world borrows before touching session).
    let entities: Vec<(u64, NetworkAuthority, Transform)> = {
        let mut q = world.query::<(&NetworkId, &Transform)>();
        q.iter(world)
            .filter(|(nid, _)| nid.is_replicated())
            .map(|(nid, t)| (nid.id, nid.authority, t.clone()))
            .collect()
    };

    let Some(session) = world.get_resource::<NetworkSession>() else {
        return;
    };

    match &session.role {
        NetworkRole::Server => {
            if session.peers.is_empty() {
                return;
            }
            let batch: Vec<(u64, TransformData)> = entities
                .iter()
                .map(|(id, _, t)| (*id, TransformData::from_transform(t)))
                .collect();
            if let Some(pkt) = encode_transform_batch(&batch) {
                for peer in &session.peers {
                    let _ = session.socket.send_to(&pkt, peer);
                }
            }
        }
        NetworkRole::Client { server_addr } => {
            let server = *server_addr;
            let my_id = session.my_peer_id;
            for (net_id, authority, t) in &entities {
                if matches!(authority, NetworkAuthority::Client { peer_id } if *peer_id == my_id) {
                    let pkt = encode_client_transform(*net_id, TransformData::from_transform(t));
                    let _ = session.socket.send_to(&pkt, server);
                }
            }
        }
    }
}
