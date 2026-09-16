// Server-side script: hosts on port 7777, owns ServerPlayer
Bsengine.network.startServer(7777);

const RADIUS = 2.0;
const CENTRE_X = -2.0;

let t = 0.0;
let laps = 0;

// Remote procedure calls, both directions.
//
// The routing is declared once per name, which is what Unity, Unreal and Godot
// all do. A multicast runs on every client *and* here on the server, so this
// side sees its own announcement -- that is Unreal's NetMulticast behaviour,
// and a server that skipped its own copy would be the one machine that never
// saw what it announced.
Bsengine.network.registerRpc("lapComplete", { target: "multicast" }, (entity, args) => {
    Bsengine.setHudText("lap", `${entity} finished lap ${args.lap}`);
});

// Called by the client, on the entity the client owns. A call on an entity the
// caller does not own is refused by the server, which is the whole point of a
// server RPC: otherwise any peer could act as any other peer.
Bsengine.network.registerRpc("wave", { target: "server" }, (entity, args) => {
    Bsengine.setHudText("wave", `${entity} waved (${args.n})`);
});

// The entry point is a top-level `onUpdate(self)`, the same as every other
// script in games/. This file used to call `Bsengine.onUpdate(cb)` and
// `Bsengine.setPosition(...)`, neither of which existed, so it threw on its
// first frame and neither player ever moved. See
// crates/bsengine-scripting/tests/prelude_names.rs for the guard that now
// makes that impossible to ship again.
function onUpdate() {
    t += Bsengine.getDeltaTime();

    const lap = Math.floor(t / (Math.PI * 2));
    if (lap > laps) {
        laps = lap;
        Bsengine.network.callRpc("ServerPlayer", "lapComplete", { lap });
    }

    Bsengine.setPosition(
        "ServerPlayer",
        Math.cos(t) * RADIUS + CENTRE_X,
        0.5,
        Math.sin(t) * RADIUS,
    );
}
