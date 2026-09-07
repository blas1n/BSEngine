use bevy_app::{App, Plugin, Update};
use bevy_ecs::prelude::*;
use bsengine_core::{NetworkAuthority, NetworkId, Transform};

use crate::{
    config::NetworkConfig,
    interpolation::{sample, SnapshotBuffers},
    packet::{
        decode_batch_tick, encode_client_transform, encode_transform_batch, TransformData,
        BATCH_HEADER_LEN, MSG_CLIENT_TRANSFORM, MSG_DISCONNECT, MSG_HELLO, MSG_HELLO_ACK,
        MSG_TRANSFORM_BATCH,
    },
    session::{NetworkRole, NetworkSession},
    sim::LinkSimulator,
};

/// Bevy plugin that wires up the UDP send/receive systems for entity transform replication.
pub struct NetworkPlugin;

/// The server's simulation frame counter, stamped into every batch it sends.
///
/// Counts sends rather than reading a clock, so the number a client receives and
/// the number a test expects are the same one. Wraps rather than saturating: a
/// `u32` of 60Hz frames is over two years, and a counter that stopped moving
/// would silently freeze every client's interpolation.
#[derive(bevy_ecs::prelude::Resource, Default, Debug)]
pub struct ServerTick(pub u32);

impl Plugin for NetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ServerTick>();
        app.init_resource::<RenderTick>();
        app.init_resource::<SnapshotBuffers>();
        app.init_resource::<NetworkConfig>();
        app.init_resource::<SimulatedLink>();
        app.add_systems(Update, network_receive_system);
        // Between receive and send: it consumes what receive buffered, and a
        // client's own send must not read a transform this just wrote for a
        // remote entity.
        app.add_systems(
            Update,
            interpolate_remote_entities
                .after(network_receive_system)
                .before(network_send_system),
        );
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
                // Client: record a server-broadcast snapshot. Deliberately not
                // applied here -- `interpolate_remote_entities` decides what an
                // entity renders at, one delay behind, which is what turns a gap
                // between packets into smooth motion instead of a freeze and a
                // jump.
                let Some(tick) = decode_batch_tick(&data) else {
                    continue;
                };
                let count = data[1] as usize;
                let mut offset = BATCH_HEADER_LEN;
                for _ in 0..count {
                    if offset + 48 > data.len() {
                        break;
                    }
                    let net_id =
                        u64::from_le_bytes(data[offset..offset + 8].try_into().unwrap_or([0; 8]));
                    let td = TransformData::from_bytes(&data[offset + 8..offset + 48]);
                    offset += 48;

                    world
                        .resource_mut::<SnapshotBuffers>()
                        .push(net_id, tick, td);
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

/// The simulated link this process sends through.
///
/// Held as a resource rather than rebuilt per frame because its whole value is
/// its *sequence*: a simulator reconstructed each frame would restart from its
/// seed and deliver the same decision every time, which is a constant link
/// wearing a simulator's name.
#[derive(Resource)]
pub struct SimulatedLink {
    simulator: LinkSimulator,
    /// What the config was when this was built, so a changed setting rebuilds it.
    built_for: (f32, u64),
}

impl SimulatedLink {
    /// Whether the next packet gets through, rebuilding if the config changed.
    fn should_deliver(&mut self, loss: f32, seed: u64) -> bool {
        if self.built_for != (loss, seed) {
            self.simulator = LinkSimulator::new(loss, seed);
            self.built_for = (loss, seed);
        }
        self.simulator.should_deliver()
    }
}

impl Default for SimulatedLink {
    fn default() -> Self {
        Self {
            simulator: LinkSimulator::new(0.0, 0),
            built_for: (0.0, 0),
        }
    }
}

/// Whether `subject` is close enough to `observer` to be worth sending.
///
/// `None` means no limit — what the engine did before interest management
/// existed, and what a peer with no entity of its own gets, since there is no
/// position to measure from.
fn within_interest(observer: glam::Vec3, subject: glam::Vec3, radius: Option<f32>) -> bool {
    match radius {
        // Squared, to avoid a square root per entity per peer per frame.
        Some(r) => observer.distance_squared(subject) <= r * r,
        None => true,
    }
}

/// The client's own clock, in server ticks, deliberately behind the newest
/// snapshot it holds.
///
/// A float because it advances one tick per rendered frame and must be able to
/// sit *between* two snapshots — that in-between value is the whole of what
/// interpolation renders.
#[derive(Resource, Default, Debug)]
pub struct RenderTick(pub f32);

/// Writes each remote entity's `Transform` from its snapshot buffer.
///
/// # The clock
///
/// The render tick advances one per frame on its own, so an entity keeps moving
/// on frames where no packet arrived — which is the entire difference between
/// interpolation and just applying whatever turned up. It is bounded on both
/// sides: never past the newest snapshot held (there is no data there, and
/// letting the clock run away during an outage would make the eventual resync a
/// large visible jump), and never further behind than the configured delay
/// (which is how it catches up after one).
///
/// # What it does not touch
///
/// Entities this peer is authoritative over. Their transforms come from local
/// simulation, and overwriting them with buffered server state is exactly the
/// fight that client-side prediction exists to arbitrate — which is sub-step
/// 2/2, not this one.
fn interpolate_remote_entities(world: &mut World) {
    let Some(session) = world.get_resource::<NetworkSession>() else {
        return;
    };
    // The server holds the authoritative state locally; there is nothing to
    // interpolate towards.
    if matches!(session.role, NetworkRole::Server) {
        return;
    }
    let my_peer_id = session.my_peer_id;

    let delay = world
        .get_resource::<NetworkConfig>()
        .map_or(0, |c| c.interpolation_delay_ticks) as f32;

    let Some(newest) = world.resource::<SnapshotBuffers>().newest_tick() else {
        return;
    };
    let newest = newest as f32;

    let render_tick = {
        let mut clock = world.resource_mut::<RenderTick>();
        clock.0 += 1.0;
        if clock.0 > newest {
            clock.0 = newest;
        }
        if clock.0 < newest - delay {
            clock.0 = newest - delay;
        }
        clock.0
    };

    let remote: Vec<(Entity, u64)> = {
        let mut q = world.query::<(Entity, &NetworkId)>();
        q.iter(world)
            .filter(|(_, nid)| nid.is_replicated())
            .filter(|(_, nid)| {
                !matches!(nid.authority, NetworkAuthority::Client { peer_id } if peer_id == my_peer_id)
            })
            .map(|(entity, nid)| (entity, nid.id))
            .collect()
    };

    for (entity, net_id) in remote {
        let sampled = world
            .resource::<SnapshotBuffers>()
            .get(net_id)
            .and_then(|buffer| sample(buffer, render_tick));
        if let Some(transform) = sampled {
            if let Some(mut t) = world.get_mut::<Transform>(entity) {
                *t = transform;
            }
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

    // Bumped before the session borrow: `session` is held immutably for the
    // rest of this function, so the resource cannot be taken mutably later.
    let tick = {
        let mut server_tick = world.resource_mut::<ServerTick>();
        server_tick.0 = server_tick.0.wrapping_add(1);
        server_tick.0
    };

    let (radius, loss, seed) =
        world
            .get_resource::<NetworkConfig>()
            .map_or((None, 0.0, 0), |config| {
                (
                    config.aoi_radius,
                    config.simulated_loss,
                    config.simulator_seed,
                )
            });

    // Decided up front, one roll per peer this frame, because the borrow of
    // `session` below is immutable and holds for the rest of the function.
    let peer_count = world
        .get_resource::<NetworkSession>()
        .map_or(0, |s| s.peers.len());
    let deliver: Vec<bool> = {
        let mut link = world.resource_mut::<SimulatedLink>();
        (0..peer_count.max(1))
            .map(|_| link.should_deliver(loss, seed))
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
            // One batch per peer rather than one broadcast, because interest is
            // measured from each peer's own position. With no radius configured
            // every peer's batch is the same one the engine always sent.
            for (index, peer) in session.peers.iter().enumerate() {
                // Peers are assigned ids from 1 in connection order, which is
                // the order they were pushed here.
                let peer_id = index as u64 + 1;
                let observer = entities
                    .iter()
                    .find(|(_, authority, _)| {
                        matches!(authority, NetworkAuthority::Client { peer_id: owner } if *owner == peer_id)
                    })
                    .map(|(_, _, t)| t.position.0);

                let batch: Vec<(u64, TransformData)> = entities
                    .iter()
                    .filter(|(_, _, t)| match observer {
                        Some(from) => within_interest(from, t.position.0, radius),
                        // No entity of its own means no position to measure
                        // from. Sending everything is the honest answer;
                        // sending nothing would look exactly like a broken
                        // filter.
                        None => true,
                    })
                    .map(|(id, _, t)| (*id, TransformData::from_transform(t)))
                    .collect();

                // Dropped here rather than at the receiver so the packet never
                // exists, which is what a lossy link actually does.
                if deliver.get(index).copied().unwrap_or(true) {
                    if let Some(pkt) = encode_transform_batch(tick, 0, &batch) {
                        let _ = session.socket.send_to(&pkt, peer);
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    /// Both halves, because an interest filter that sends nothing passes the
    /// first assertion and one that sends everything passes the second.
    #[test]
    fn interest_keeps_the_near_entity_and_drops_the_far_one() {
        let observer = Vec3::ZERO;

        assert!(
            within_interest(observer, Vec3::new(5.0, 0.0, 0.0), Some(50.0)),
            "an entity inside the radius must be sent"
        );
        assert!(
            !within_interest(observer, Vec3::new(500.0, 0.0, 0.0), Some(50.0)),
            "an entity outside it must not be"
        );
    }

    /// Exactly on the boundary counts as inside. Stated so the edge is a
    /// decision rather than whatever the comparison happened to do.
    #[test]
    fn an_entity_exactly_on_the_radius_is_included() {
        assert!(within_interest(
            Vec3::ZERO,
            Vec3::new(50.0, 0.0, 0.0),
            Some(50.0)
        ));
    }

    /// No radius is the pre-AOI behaviour and must stay reachable, since it is
    /// also what a peer with no entity of its own gets.
    #[test]
    fn no_radius_sends_everything() {
        assert!(within_interest(Vec3::ZERO, Vec3::new(1e6, 0.0, 0.0), None));
    }
}
