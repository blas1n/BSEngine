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
use bsengine_core::{NetworkAuthority, NetworkId, RpcCall, RpcQueues, RpcTarget, Transform};
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

    /// Spawns an entity the given peer owns, on both sides.
    ///
    /// `Predicted` rather than `Client` because that is the variant a real
    /// player character uses, and because the ownership check the server does
    /// on an incoming call has to accept both.
    fn spawn_owned(&mut self, net_id: u64, peer_id: u64) -> (Entity, Entity) {
        let id = NetworkId {
            id: net_id,
            authority: NetworkAuthority::Predicted { peer_id },
        };
        let on_server = self
            .server
            .world_mut()
            .spawn((id, Transform::default()))
            .id();
        let on_client = self
            .client
            .world_mut()
            .spawn((id, Transform::default()))
            .id();
        (on_server, on_client)
    }

    fn queue_rpc(app: &mut App, call: RpcCall, target: RpcTarget, reliable: bool) {
        app.world_mut()
            .resource_mut::<RpcQueues>()
            .send(call, target, reliable);
    }

    /// Every call that has arrived on `app` so far, drained as it goes.
    ///
    /// Accumulated across frames rather than read at the end, because the queue
    /// is drained by whoever runs handlers and a test that only looked once
    /// would see whatever happened to be left.
    fn collect(app: &mut App, into: &mut Vec<RpcCall>) {
        into.extend(app.world_mut().resource_mut::<RpcQueues>().take_incoming());
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

/// The producer half of reconciliation: a correction for a predicted entity
/// must actually be **emitted**, not merely applicable.
///
/// This test exists because its absence hid a real gap. The scripting-side
/// tests inject a `ReplayRequest` directly and assert it is applied, so they
/// pass whether or not anything ever produces one — a disconnected producer
/// looks exactly like a working one from the consumer's side. Clippy's
/// "unused variable" warning is what actually caught it, which is not a
/// safety net anyone should rely on twice.
#[test]
fn a_correction_for_a_predicted_entity_is_emitted_to_the_scripting_layer() {
    let mut pair = Pair::connect(NetworkConfig::default());
    let peer_id = pair.client.world().resource::<NetworkSession>().my_peer_id;

    // Server-side: the entity the server simulates and is authoritative over.
    pair.server.world_mut().spawn((
        NetworkId {
            id: 1,
            authority: NetworkAuthority::Predicted { peer_id },
        },
        Transform {
            position: Vec3::new(9.0, 0.0, 0.0).into(),
            ..Default::default()
        },
    ));
    // Client-side: the same entity, which this peer predicts.
    pair.client.world_mut().spawn((
        NetworkId {
            id: 1,
            authority: NetworkAuthority::Predicted { peer_id },
        },
        Transform::default(),
    ));

    for _ in 0..4 {
        pair.step();
    }

    let replays = pair
        .client
        .world()
        .resource::<bsengine_core::PendingReplays>();
    assert!(
        replays.0.iter().any(|r| r.net_id == 1),
        "the client must have been handed a correction for its predicted \
         entity; without one, reconciliation never fires in a real game no \
         matter how well the scripting side applies them"
    );

    // And it must be a correction, not an empty shell: the authoritative
    // position has to be the server's.
    let request = replays
        .0
        .iter()
        .find(|r| r.net_id == 1)
        .expect("checked above");
    assert!(
        (request.authoritative.position.0.x - 9.0).abs() < 1e-3,
        "the correction must carry where the server actually says the entity \
         is; got x={}",
        request.authoritative.position.0.x
    );
}

/// Paired with the above: a predicted entity must **not** also be interpolated.
///
/// Buffering it would fight the local prediction and hold the entity a delay in
/// the past — the opposite of the reason it is predicted at all.
#[test]
fn a_predicted_entity_is_corrected_rather_than_interpolated() {
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

    for _ in 0..4 {
        pair.step();
    }

    assert!(
        pair.client
            .world()
            .resource::<bsengine_network::SnapshotBuffers>()
            .get(1)
            .is_none(),
        "a predicted entity must not be in the interpolation buffer"
    );
}

/// The headline for reliability: on a link that drops half the packets, every
/// call still arrives, exactly once, in the order it was sent.
///
/// All three of those are asserted separately. "They all arrived" alone would
/// pass for a channel that delivered each one three times; "none arrived twice"
/// alone would pass for one that dropped them.
#[test]
fn every_reliable_call_arrives_exactly_once_and_in_order_under_loss() {
    let mut pair = Pair::connect(NetworkConfig {
        simulated_loss: 0.5,
        simulator_seed: 11,
        rpc_resend_frames: 2,
        ..Default::default()
    });
    pair.spawn_owned(1, 1);

    let sent: Vec<String> = (0..8).map(|i| format!("call{i}")).collect();
    for name in &sent {
        Pair::queue_rpc(
            &mut pair.client,
            RpcCall {
                net_id: 1,
                name: name.clone(),
                args: String::new(),
            },
            RpcTarget::Server,
            true,
        );
        // One per frame, so the loss the simulator rolls falls on different
        // packets rather than on one burst.
        pair.step();
    }

    let mut arrived = Vec::new();
    for _ in 0..60 {
        pair.step();
        Pair::collect(&mut pair.server, &mut arrived);
    }

    let names: Vec<String> = arrived.iter().map(|c| c.name.clone()).collect();
    assert_eq!(
        names, sent,
        "every call, once each, in the order they were made"
    );
}

/// The instrument check: without the reliable envelope the same link loses
/// calls.
///
/// Without this, a "reliable" channel that did nothing at all would pass the
/// test above on any run where the simulator happened to be kind -- and there
/// would be no way to tell from the green.
#[test]
fn the_same_calls_sent_unreliably_do_not_all_arrive() {
    let mut pair = Pair::connect(NetworkConfig {
        simulated_loss: 0.5,
        simulator_seed: 11,
        rpc_resend_frames: 2,
        ..Default::default()
    });
    pair.spawn_owned(1, 1);

    for i in 0..8 {
        Pair::queue_rpc(
            &mut pair.client,
            RpcCall {
                net_id: 1,
                name: format!("call{i}"),
                args: String::new(),
            },
            RpcTarget::Server,
            false,
        );
        pair.step();
    }

    let mut arrived = Vec::new();
    for _ in 0..60 {
        pair.step();
        Pair::collect(&mut pair.server, &mut arrived);
    }

    assert!(
        arrived.len() < 8,
        "a 50% link that delivered all 8 unreliable calls would mean the loss \
         simulator is not reaching this path, and the test above proves nothing: \
         got {}",
        arrived.len()
    );
}

/// Arguments travel, and travel with the call they belong to.
#[test]
fn a_call_arrives_with_its_own_arguments() {
    let mut pair = Pair::connect(NetworkConfig::default());
    pair.spawn_owned(1, 1);

    // Distinct arguments deliberately: with equal ones a transport that paired
    // a name with the wrong payload would look correct.
    for (name, args) in [("open", r#"{"door":3}"#), ("close", r#"{"door":7}"#)] {
        Pair::queue_rpc(
            &mut pair.client,
            RpcCall {
                net_id: 1,
                name: name.into(),
                args: args.into(),
            },
            RpcTarget::Server,
            true,
        );
    }

    let mut arrived = Vec::new();
    for _ in 0..10 {
        pair.step();
        Pair::collect(&mut pair.server, &mut arrived);
    }

    assert_eq!(arrived.len(), 2, "{arrived:?}");
    assert_eq!(arrived[0].name, "open");
    assert_eq!(arrived[0].args, r#"{"door":3}"#);
    assert_eq!(arrived[1].name, "close");
    assert_eq!(
        arrived[1].args, r#"{"door":7}"#,
        "each call keeps the arguments it was made with"
    );
}

/// A client may not make a call on an entity it does not own.
///
/// This is the whole security property of a server RPC: without it any peer
/// could drive any other peer's character by naming its id.
#[test]
fn the_server_refuses_a_call_on_an_entity_the_caller_does_not_own() {
    let mut pair = Pair::connect(NetworkConfig::default());
    // Entity 1 belongs to the connected peer; entity 2 belongs to somebody else.
    pair.spawn_owned(1, 1);
    pair.spawn_owned(2, 2);

    for net_id in [1, 2] {
        Pair::queue_rpc(
            &mut pair.client,
            RpcCall {
                net_id,
                name: format!("touch{net_id}"),
                args: String::new(),
            },
            RpcTarget::Server,
            true,
        );
    }

    let mut arrived = Vec::new();
    for _ in 0..10 {
        pair.step();
        Pair::collect(&mut pair.server, &mut arrived);
    }

    let names: Vec<&str> = arrived.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["touch1"],
        "the call on the owned entity goes through and the other does not"
    );
}

/// A multicast reaches the clients and also runs on the server.
///
/// Unreal's `NetMulticast` behaves this way, and a server that skipped its own
/// copy would be the one machine that never saw what it announced.
#[test]
fn a_multicast_runs_on_the_client_and_on_the_server() {
    let mut pair = Pair::connect(NetworkConfig::default());
    pair.spawn_owned(1, 1);

    Pair::queue_rpc(
        &mut pair.server,
        RpcCall {
            net_id: 1,
            name: "explode".into(),
            args: String::new(),
        },
        RpcTarget::Multicast,
        true,
    );

    let (mut on_server, mut on_client) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        pair.step();
        Pair::collect(&mut pair.server, &mut on_server);
        Pair::collect(&mut pair.client, &mut on_client);
    }

    assert_eq!(on_server.len(), 1, "the server runs its own copy");
    assert_eq!(on_client.len(), 1, "and the client runs one too");
    assert_eq!(on_client[0].name, "explode");
}

/// A call aimed at an owner reaches that peer.
#[test]
fn a_call_to_the_owner_reaches_that_peer() {
    let mut pair = Pair::connect(NetworkConfig::default());
    pair.spawn_owned(1, 1);

    Pair::queue_rpc(
        &mut pair.server,
        RpcCall {
            net_id: 1,
            name: "youWereHit".into(),
            args: r#"{"damage":12}"#.into(),
        },
        RpcTarget::Owner,
        true,
    );

    let (mut on_server, mut on_client) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        pair.step();
        Pair::collect(&mut pair.server, &mut on_server);
        Pair::collect(&mut pair.client, &mut on_client);
    }

    assert_eq!(on_client.len(), 1, "the owner runs it");
    assert_eq!(on_client[0].args, r#"{"damage":12}"#);
    assert!(
        on_server.is_empty(),
        "and the server does not, or every owner-targeted call would run twice"
    );
}

/// A client cannot multicast.
///
/// Dropped rather than relayed: letting a client reach every other machine is
/// the thing server authority exists to prevent, and all three reference
/// engines refuse it the same way.
#[test]
fn a_client_cannot_send_a_multicast() {
    let mut pair = Pair::connect(NetworkConfig::default());
    pair.spawn_owned(1, 1);

    Pair::queue_rpc(
        &mut pair.client,
        RpcCall {
            net_id: 1,
            name: "everybodyExplode".into(),
            args: String::new(),
        },
        RpcTarget::Multicast,
        true,
    );

    let (mut on_server, mut on_client) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        pair.step();
        Pair::collect(&mut pair.server, &mut on_server);
        Pair::collect(&mut pair.client, &mut on_client);
    }

    assert!(on_server.is_empty(), "the server must not relay it");
    assert!(on_client.is_empty(), "and it must not run locally either");
}

/// Replication keeps working while calls are in flight.
///
/// The reliable channel shares a socket with the snapshot stream, and a resend
/// loop that starved it would be a regression no RPC test would notice.
#[test]
fn transform_replication_still_works_alongside_calls() {
    let mut pair = Pair::connect(NetworkConfig::default());
    let (on_server, on_client) = pair.spawn_replicated(9, Vec3::ZERO);
    pair.spawn_owned(1, 1);

    for i in 0..10 {
        Pair::queue_rpc(
            &mut pair.server,
            RpcCall {
                net_id: 1,
                name: format!("tick{i}"),
                args: String::new(),
            },
            RpcTarget::Multicast,
            true,
        );
        pair.move_server_entity(on_server, Vec3::new(i as f32, 0.0, 0.0));
        pair.step();
    }

    assert!(
        (pair.client_position(on_client).x - 9.0).abs() < 1e-3,
        "the client should still be tracking the server's transform, got {:?}",
        pair.client_position(on_client)
    );
}

/// A server and *two* clients, so "send it to the owner" and "send it to
/// everyone" stop being the same sentence.
///
/// With a single client every routing rule collapses into "send it down the one
/// socket", and a transport that ignored the target entirely would pass every
/// test above.
struct Room {
    server: App,
    first: App,
    second: App,
}

impl Room {
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

        let mut apps = Vec::new();
        for _ in 0..2 {
            let mut app = App::new();
            app.add_plugins(NetworkPlugin);
            app.insert_resource(config.clone());
            app.insert_resource(
                NetworkSession::new_client("127.0.0.1", port).expect("bind client"),
            );
            // Settled one at a time: peer ids are assigned in connection order,
            // and a test that says "the first client" has to be able to mean it.
            for _ in 0..200 {
                server.update();
                app.update();
                if app.world().resource::<NetworkSession>().connected {
                    break;
                }
            }
            assert!(
                app.world().resource::<NetworkSession>().connected,
                "a client never completed its handshake"
            );
            apps.push(app);
        }
        let second = apps.pop().expect("two clients");
        let first = apps.pop().expect("two clients");
        Self {
            server,
            first,
            second,
        }
    }

    fn step(&mut self) {
        self.server.update();
        self.first.update();
        self.second.update();
    }

    /// Spawns an entity owned by `peer_id` on the server and on both clients.
    fn spawn_owned(&mut self, net_id: u64, peer_id: u64) {
        let id = NetworkId {
            id: net_id,
            authority: NetworkAuthority::Predicted { peer_id },
        };
        for app in [&mut self.server, &mut self.first, &mut self.second] {
            app.world_mut().spawn((id, Transform::default()));
        }
    }
}

/// An owner-targeted call reaches its own owner and nobody else.
///
/// Both owners are exercised in one test on purpose. Aimed only at the first
/// client's entity, "send it to the owner", "send it to everyone" and "send it
/// to whichever peer connected first" all name the same socket -- and a
/// transport doing any of those three would pass.
#[test]
fn an_owner_targeted_call_reaches_that_owner_and_no_one_else() {
    let mut room = Room::connect(NetworkConfig::default());
    room.spawn_owned(1, 1);
    room.spawn_owned(2, 2);

    for net_id in [1, 2] {
        Pair::queue_rpc(
            &mut room.server,
            RpcCall {
                net_id,
                name: format!("hit{net_id}"),
                args: String::new(),
            },
            RpcTarget::Owner,
            true,
        );
    }

    let (mut first, mut second) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        room.step();
        Pair::collect(&mut room.first, &mut first);
        Pair::collect(&mut room.second, &mut second);
    }

    let names =
        |calls: &[RpcCall]| -> Vec<String> { calls.iter().map(|c| c.name.clone()).collect() };
    assert_eq!(
        names(&first),
        vec!["hit1".to_string()],
        "peer 1 gets the call on its own entity and not the other one"
    );
    assert_eq!(
        names(&second),
        vec!["hit2".to_string()],
        "and peer 2 gets its own -- which a transport that always sent to the          first peer would fail"
    );
}

/// And a multicast reaches both of them.
///
/// The other half of the pair above: without it, a transport that dropped every
/// call except the owner's would pass that test.
#[test]
fn a_multicast_reaches_both_clients() {
    let mut room = Room::connect(NetworkConfig::default());
    room.spawn_owned(1, 1);

    Pair::queue_rpc(
        &mut room.server,
        RpcCall {
            net_id: 1,
            name: "explode".into(),
            args: String::new(),
        },
        RpcTarget::Multicast,
        true,
    );

    let (mut first, mut second) = (Vec::new(), Vec::new());
    for _ in 0..10 {
        room.step();
        Pair::collect(&mut room.first, &mut first);
        Pair::collect(&mut room.second, &mut second);
    }

    assert_eq!(first.len(), 1, "{first:?}");
    assert_eq!(second.len(), 1, "both clients run a multicast: {second:?}");
}

/// One client cannot make a call on the other client's entity.
///
/// The same refusal as the single-client test, but with a peer that genuinely
/// exists on the other end -- there the unowned id belonged to nobody, and a
/// server that only rejected unknown ids would have passed.
#[test]
fn a_client_cannot_call_on_another_clients_entity() {
    let mut room = Room::connect(NetworkConfig::default());
    room.spawn_owned(1, 1);
    room.spawn_owned(2, 2);

    // The second client reaches for the first client's entity, and for its own.
    for net_id in [1, 2] {
        Pair::queue_rpc(
            &mut room.second,
            RpcCall {
                net_id,
                name: format!("touch{net_id}"),
                args: String::new(),
            },
            RpcTarget::Server,
            true,
        );
    }

    let mut arrived = Vec::new();
    for _ in 0..10 {
        room.step();
        Pair::collect(&mut room.server, &mut arrived);
    }

    let names: Vec<&str> = arrived.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["touch2"],
        "only the call on its own entity should be accepted"
    );
}
