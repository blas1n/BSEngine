//! A real server and a real client, in one process, over real UDP.
//!
//! # Why one process
//!
//! Two processes are closer to a shipped game, but their frames are not ordered
//! relative to each other, so an assertion about "the client's position after
//! the server's fifth frame" is a race. Here the test steps both apps by hand,
//! so every assertion below can name an exact frame — while still going through
//! real sockets, so the handshake, the wire format and the batching are all
//! genuinely exercised rather than stubbed.
//!
//! # Why the link simulator is in almost every test
//!
//! On a perfect link the interpolated position and the newest received snapshot
//! agree on every frame. An interpolator that does nothing at all therefore
//! passes any test run without induced loss — which is exactly what these tests
//! exist to catch.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bsengine_core::{NetworkAuthority, NetworkId, Transform};
use bsengine_network::{AppliedInputs, NetworkConfig, NetworkPlugin, NetworkSession};
use glam::Vec3;

/// A server app and a client app wired to each other over loopback.
struct Pair {
    server: App,
    client: App,
}

impl Pair {
    /// Binds a server on an ephemeral port and connects one client to it.
    ///
    /// Port 0 rather than a fixed number: a hardcoded port makes this fail
    /// whenever anything else on the machine happens to hold it, which reads as
    /// a broken feature rather than a busy socket.
    fn connect(config: NetworkConfig) -> Self {
        let server_session = NetworkSession::new_server(0).expect("bind server");
        let port = server_session
            .socket
            .local_addr()
            .expect("server address")
            .port();

        let mut server = App::new();
        server.add_plugins(NetworkPlugin);
        server.insert_resource(config.clone());
        server.insert_resource(server_session);

        let mut client = App::new();
        client.add_plugins(NetworkPlugin);
        client.insert_resource(config);
        client.insert_resource(NetworkSession::new_client("127.0.0.1", port).expect("bind client"));

        let mut pair = Self { server, client };
        pair.settle_handshake();
        pair
    }

    /// Steps both apps until the client has a peer id, or fails loudly.
    ///
    /// A silent timeout here would make every later assertion fail for a reason
    /// that has nothing to do with what it is testing.
    fn settle_handshake(&mut self) {
        for _ in 0..200 {
            self.step();
            if self.client.world().resource::<NetworkSession>().connected {
                return;
            }
        }
        panic!("the client never completed its handshake with the server");
    }

    /// One frame on each side, server first so the client sees this frame's
    /// snapshot rather than last frame's.
    fn step(&mut self) {
        self.server.update();
        self.client.update();
    }

    /// Spawns the same replicated entity on both sides and returns both handles.
    fn spawn_replicated(&mut self, net_id: u64, at: Vec3) -> (Entity, Entity) {
        let transform = Transform {
            position: at.into(),
            ..Default::default()
        };
        let on_server = self
            .server
            .world_mut()
            .spawn((
                NetworkId {
                    id: net_id,
                    authority: NetworkAuthority::Server,
                },
                transform.clone(),
            ))
            .id();
        let on_client = self
            .client
            .world_mut()
            .spawn((
                NetworkId {
                    id: net_id,
                    authority: NetworkAuthority::Server,
                },
                transform,
            ))
            .id();
        (on_server, on_client)
    }

    fn move_server_entity(&mut self, entity: Entity, to: Vec3) {
        let mut transform = self
            .server
            .world_mut()
            .get_mut::<Transform>(entity)
            .expect("server entity has a transform");
        transform.position = to.into();
    }

    fn client_position(&self, entity: Entity) -> Vec3 {
        self.client
            .world()
            .get::<Transform>(entity)
            .expect("client entity has a transform")
            .position
            .0
    }
}

/// The headline: a remote entity keeps moving on frames where nothing arrived,
/// and keeps moving *along the server's path* rather than drifting.
///
/// Both halves are asserted. "It moved" alone would pass for an entity wandering
/// anywhere; "it is close to the server" alone would pass for one that never
/// moved at all, since it starts there.
#[test]
fn a_remote_entity_tracks_the_server_under_packet_loss() {
    let mut pair = Pair::connect(NetworkConfig {
        interpolation_delay_ticks: 2,
        simulated_loss: 0.5,
        simulator_seed: 7,
        ..Default::default()
    });
    let (on_server, on_client) = pair.spawn_replicated(1, Vec3::ZERO);

    let mut samples = Vec::new();
    for frame in 1..=40 {
        // A straight line the client's rendered position can be compared to.
        pair.move_server_entity(on_server, Vec3::new(frame as f32, 0.0, 0.0));
        pair.step();
        samples.push(pair.client_position(on_client).x);
    }

    let final_x = *samples.last().expect("sampled");
    assert!(
        final_x > 20.0,
        "the client must have followed the server a long way down the line, \
         got x={final_x} after 40 frames"
    );
    assert!(
        final_x <= 40.0,
        "and must not have run past where the server ever was, got x={final_x}"
    );

    // Monotonic: the server only moved forwards, so a rendered position that
    // went backwards means the interpolator is reordering or extrapolating.
    for pair_of in samples.windows(2) {
        assert!(
            pair_of[1] >= pair_of[0] - 1e-3,
            "the rendered position moved backwards: {} then {}",
            pair_of[0],
            pair_of[1]
        );
    }
}

/// The difference between interpolating and merely applying what arrived.
///
/// With half the packets dropped, a client that only applied received snapshots
/// would hold still on every dropped frame. This asserts it advanced on more
/// frames than it received.
#[test]
fn the_client_advances_on_frames_where_nothing_arrived() {
    let mut pair = Pair::connect(NetworkConfig {
        interpolation_delay_ticks: 3,
        simulated_loss: 0.5,
        simulator_seed: 11,
        ..Default::default()
    });
    let (on_server, on_client) = pair.spawn_replicated(1, Vec3::ZERO);

    let mut advanced_on = 0;
    let mut previous = pair.client_position(on_client).x;
    for frame in 1..=40 {
        pair.move_server_entity(on_server, Vec3::new(frame as f32, 0.0, 0.0));
        pair.step();
        let now = pair.client_position(on_client).x;
        if now > previous + 1e-4 {
            advanced_on += 1;
        }
        previous = now;
    }

    assert!(
        advanced_on > 25,
        "with half the packets dropped, a client that only applied what arrived \
         would advance on about half of 40 frames; interpolation should carry it \
         through the gaps. Advanced on {advanced_on}"
    );
}

/// Interest management, both halves.
///
/// A filter that sends nothing passes the far assertion; one that sends
/// everything passes the near one. Only together do they say the radius is
/// being applied.
#[test]
fn interest_management_stops_updates_for_a_distant_entity() {
    let mut pair = Pair::connect(NetworkConfig {
        aoi_radius: Some(50.0),
        ..Default::default()
    });

    // The client's own entity, which is what the server measures distance from.
    let observer = pair
        .server
        .world_mut()
        .spawn((
            NetworkId {
                id: 99,
                authority: NetworkAuthority::Client { peer_id: 1 },
            },
            Transform::default(),
        ))
        .id();
    let _ = observer;

    let (near_server, near_client) = pair.spawn_replicated(1, Vec3::new(10.0, 0.0, 0.0));
    let (far_server, far_client) = pair.spawn_replicated(2, Vec3::new(500.0, 0.0, 0.0));

    for frame in 1..=20 {
        pair.move_server_entity(near_server, Vec3::new(10.0 + frame as f32, 0.0, 0.0));
        pair.move_server_entity(far_server, Vec3::new(500.0 + frame as f32, 0.0, 0.0));
        pair.step();
    }

    let near = pair.client_position(near_client).x;
    let far = pair.client_position(far_client).x;

    assert!(
        near > 11.0,
        "the entity inside the radius must keep receiving updates, got x={near}"
    );
    assert!(
        (far - 500.0).abs() < 1e-3,
        "the entity outside the radius must stop receiving them and stay where \
         the client last knew it, got x={far}"
    );
}

/// The existing path, unchanged: with nothing configured a client ends up where
/// the server put it. `net-2p-demo` depends on this.
#[test]
fn a_default_session_replicates_as_it_always_did() {
    let mut pair = Pair::connect(NetworkConfig::default());
    let (on_server, on_client) = pair.spawn_replicated(1, Vec3::ZERO);

    pair.move_server_entity(on_server, Vec3::new(42.0, 0.0, 0.0));
    for _ in 0..5 {
        pair.step();
    }

    let x = pair.client_position(on_client).x;
    assert!(
        (x - 42.0).abs() < 1e-3,
        "with no delay and no loss the client should sit exactly where the \
         server put it, got x={x}"
    );
}

/// A predicted entity's owner sends **input**, never its transform.
///
/// The two messages are not interchangeable: a transform from the client would
/// make it authoritative again and leave the server nothing to disagree with,
/// which is the whole thing prediction exists to arrange.
#[test]
fn a_predicted_entity_sends_input_rather_than_its_transform() {
    let mut pair = Pair::connect(NetworkConfig::default());

    let peer_id = pair.client.world().resource::<NetworkSession>().my_peer_id;
    for world in [pair.server.world_mut(), pair.client.world_mut()] {
        world.spawn((
            NetworkId {
                id: 1,
                authority: NetworkAuthority::Predicted { peer_id },
            },
            Transform::default(),
        ));
    }
    // Something for the client to be holding, so the input is not empty.
    pair.client
        .world_mut()
        .insert_resource(bsengine_core::LocalHeldKeys(vec!["W".to_string()]));

    for _ in 0..4 {
        pair.step();
    }

    let applied = pair.server.world().resource::<AppliedInputs>();
    assert!(
        !applied.0.is_empty(),
        "the server must have applied at least one input for the predicted \
         entity -- an empty map means the client sent a transform, or nothing"
    );

    let remote = pair
        .server
        .world()
        .resource::<bsengine_core::RemoteHeldKeys>();
    assert_eq!(
        remote.0.get(&1).map(Vec::as_slice),
        Some(["W".to_string()].as_slice()),
        "and the keys it published for that entity are the ones the client held"
    );
}

/// A client-authoritative entity is untouched by any of this, which is what
/// makes the change additive rather than a migration.
#[test]
fn a_client_authoritative_entity_still_sends_its_transform() {
    let mut pair = Pair::connect(NetworkConfig::default());

    let peer_id = pair.client.world().resource::<NetworkSession>().my_peer_id;
    for world in [pair.server.world_mut(), pair.client.world_mut()] {
        world.spawn((
            NetworkId {
                id: 2,
                authority: NetworkAuthority::Client { peer_id },
            },
            Transform {
                position: Vec3::new(3.0, 0.0, 0.0).into(),
                ..Default::default()
            },
        ));
    }

    for _ in 0..4 {
        pair.step();
    }

    assert!(
        pair.server.world().resource::<AppliedInputs>().0.is_empty(),
        "a client-authoritative entity must not be feeding the input path"
    );
}
