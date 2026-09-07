//! Client-side prediction and server reconciliation, with a real movement
//! script running on both peers.
//!
//! # Why this test lives in the scripting crate
//!
//! Because both halves have to be here. `bsengine-network` decides *that* a
//! correction is due and `bsengine-scripting` performs it by re-running the
//! movement script — and scripting depends on network, so the network crate
//! cannot dev-depend back on it. This is the only crate that can see the whole
//! path.
//!
//! # Why every test here induces latency
//!
//! With none, the prediction and the authoritative state agree on every frame,
//! so a client that ignored corrections entirely would pass. The latency is the
//! instrument, not the scenario.

use bevy_app::App;
use bevy_ecs::prelude::*;
use bsengine_app::new_app;
use bsengine_core::{
    LocalHeldKeys, NetworkAuthority, NetworkId, PendingReplays, RemoteHeldKeys, ReplayRequest,
    Transform,
};
use bsengine_scene::{Name, ScriptPath};
use bsengine_scripting::{Script, ScriptingPlugin};
use glam::Vec3;
use std::sync::atomic::{AtomicU32, Ordering};

/// A movement script of the shape a game would actually write: read a key, move.
///
/// Deliberately not a clock — `games/net-2p-demo` moves on `t += deltaTime` and
/// therefore has nothing to predict, which is what made it useless as a fixture
/// for this.
const MOVE_SCRIPT: &str = r#"
function onUpdate(name) {
    if (Bsengine.isKeyPressed("D")) {
        const p = Bsengine.getTransform(name).position;
        Bsengine.setPosition(name, p.x + 1.0, p.y, p.z);
    }
}
"#;

/// A throwaway script file, removed on drop.
struct ScriptFile(std::path::PathBuf);

impl Drop for ScriptFile {
    fn drop(&mut self) {
        std::fs::remove_file(&self.0).ok();
    }
}

fn write_script() -> ScriptFile {
    static N: AtomicU32 = AtomicU32::new(0);
    let path = std::env::temp_dir().join(format!(
        "bsengine-prediction-{}-{}.js",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, MOVE_SCRIPT).expect("write script");
    ScriptFile(path)
}

/// Builds a headless app with a named, networked, scripted entity, and steps it
/// until the script has actually loaded.
///
/// "Run until it arrives" rather than a fixed frame count, matching this crate's
/// existing script tests: how many frames a load takes is `bevy_asset`'s
/// business, and pinning a number turns a slow filesystem into a failure that
/// says nothing about prediction.
fn app_with_script(
    entity_name: &str,
    net_id: u64,
    authority: NetworkAuthority,
) -> (App, Entity, ScriptFile) {
    let script = write_script();

    let mut app = new_app();
    app.add_plugins(bsengine_asset::AssetPlugin);
    app.add_plugins(ScriptingPlugin {
        project_dir: String::new(),
    });
    app.insert_resource(LocalHeldKeys::default());
    app.insert_resource(RemoteHeldKeys::default());
    // Owned by `NetworkPlugin` in a real app. Inserted here because this test
    // stands in for the networking layer: it publishes the corrections a server
    // would have sent, and asserts on what the scripting layer does with them.
    app.insert_resource(PendingReplays::default());

    let entity = app
        .world_mut()
        .spawn((
            Name(entity_name.to_string()),
            Transform::default(),
            NetworkId {
                id: net_id,
                authority,
            },
            ScriptPath(script.0.to_string_lossy().to_string()),
        ))
        .id();

    let mut frames = 0;
    loop {
        app.update();
        frames += 1;
        if app.world().get::<Script>(entity).is_some() {
            break;
        }
        assert!(
            frames < 300,
            "the movement script never loaded, so nothing below was measured"
        );
    }
    (app, entity, script)
}

fn position_x(app: &App, entity: Entity) -> f32 {
    app.world()
        .get::<Transform>(entity)
        .expect("entity has a transform")
        .position
        .0
        .x
}

/// A correction puts the entity where the server says it was, then re-applies
/// the input the server had not yet seen.
///
/// The expected position is stated exactly rather than as a band: the whole
/// question is whether **all three** unacknowledged inputs were replayed, and a
/// tolerance wide enough to be safe would also admit replaying two of them.
#[test]
fn a_correction_replays_every_unacknowledged_input() {
    let (mut app, entity, _script) =
        app_with_script("Player", 1, NetworkAuthority::Predicted { peer_id: 1 });

    // The server says the entity was at x=10 as of the input it had applied,
    // and three inputs since then are still unacknowledged -- each of which
    // moves the entity one unit right.
    let held = vec!["D".to_string()];
    app.world_mut()
        .resource_mut::<PendingReplays>()
        .0
        .push(ReplayRequest {
            net_id: 1,
            authoritative: Transform {
                position: Vec3::new(10.0, 0.0, 0.0).into(),
                ..Default::default()
            },
            replay: vec![held.clone(), held.clone(), held.clone()],
        });

    app.update();

    assert!(
        (position_x(&app, entity) - 13.0).abs() < 1e-3,
        "10 from the server plus three replayed inputs is 13; got {}. A lower \
         number means inputs the server has not seen were dropped, which would \
         make the entity jump backwards on every packet",
        position_x(&app, entity)
    );
}

/// The snap on its own, with nothing to replay.
///
/// Paired with the test above: without this, an implementation that replayed
/// but never applied the authoritative transform would still reach 13 from a
/// starting position of 10.
#[test]
fn a_correction_with_nothing_outstanding_is_just_the_snap() {
    let (mut app, entity, _script) =
        app_with_script("Player", 1, NetworkAuthority::Predicted { peer_id: 1 });

    app.world_mut()
        .resource_mut::<PendingReplays>()
        .0
        .push(ReplayRequest {
            net_id: 1,
            authoritative: Transform {
                position: Vec3::new(7.0, 0.0, 0.0).into(),
                ..Default::default()
            },
            replay: Vec::new(),
        });

    app.update();

    assert!(
        (position_x(&app, entity) - 7.0).abs() < 1e-3,
        "with nothing outstanding the entity sits exactly where the server put \
         it; got {}",
        position_x(&app, entity)
    );
}

/// A correction must be consumed, not merely read.
///
/// Left in place it would be applied again the next frame and move the entity
/// twice as far — a bug that looks like the physics being wrong rather than the
/// networking.
#[test]
fn a_correction_is_applied_once_and_not_again() {
    let (mut app, entity, _script) =
        app_with_script("Player", 1, NetworkAuthority::Predicted { peer_id: 1 });

    app.world_mut()
        .resource_mut::<PendingReplays>()
        .0
        .push(ReplayRequest {
            net_id: 1,
            authoritative: Transform {
                position: Vec3::new(4.0, 0.0, 0.0).into(),
                ..Default::default()
            },
            replay: vec![vec!["D".to_string()]],
        });

    app.update();
    let after_first = position_x(&app, entity);
    app.update();
    let after_second = position_x(&app, entity);

    assert!(
        (after_first - 5.0).abs() < 1e-3,
        "4 from the server plus one replayed input; got {after_first}"
    );
    assert!(
        (after_second - after_first).abs() < 1e-3,
        "the second frame must not re-apply the same correction; went from \
         {after_first} to {after_second}"
    );
}

/// The replay's input must not survive into the live frame.
///
/// If it did, the entity would keep moving as though the key were still held
/// after the player let go — which reads to a player as their controls sticking,
/// and would never be traced back to reconciliation.
#[test]
fn replayed_input_does_not_leak_into_the_live_frame() {
    // The local player is holding nothing.
    let (mut app, entity, _script) =
        app_with_script("Player", 1, NetworkAuthority::Predicted { peer_id: 1 });

    app.world_mut()
        .resource_mut::<PendingReplays>()
        .0
        .push(ReplayRequest {
            net_id: 1,
            authoritative: Transform {
                position: Vec3::new(0.0, 0.0, 0.0).into(),
                ..Default::default()
            },
            // One replayed input holding D.
            replay: vec![vec!["D".to_string()]],
        });

    app.update();
    let after_correction = position_x(&app, entity);

    // Several quiet frames with no key held and no correction.
    for _ in 0..3 {
        app.update();
    }

    assert!(
        (position_x(&app, entity) - after_correction).abs() < 1e-3,
        "with nothing held the entity must stand still after the correction; it \
         went from {after_correction} to {} , which means the replayed input \
         outlived its replay",
        position_x(&app, entity)
    );
}
