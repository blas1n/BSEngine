// Client-side script: connects to localhost:7777, moves ClientPlayer
Bsengine.network.connect("127.0.0.1", 7777);

const RADIUS = 1.5;
const CENTRE_X = 2.0;

let t = 0.0;
let sinceWave = 0.0;
let waves = 0;

// The same two declarations as the server's. The two sides run different
// scripts here -- unlike Unity and Unreal, where both ends are the same
// compiled binary -- so each side declares the calls it takes part in, the way
// a Godot scene does.
Bsengine.network.registerRpc("lapComplete", { target: "multicast" }, (entity, args) => {
    Bsengine.setHudText("lap", `the server's ${entity} finished lap ${args.lap}`);
});

// Declared here because this side is the *sender*: the declaration carries the
// routing, so calling without one has nowhere to go.
Bsengine.network.registerRpc("wave", { target: "server" }, () => {});

// The entry point is a top-level `onUpdate(self)`, the same as every other
// script in games/. This file used to call `Bsengine.onUpdate(cb)` and
// `Bsengine.setPosition(...)`, neither of which existed, so it threw on its
// first frame and neither player ever moved. See
// crates/bsengine-scripting/tests/prelude_names.rs for the guard that now
// makes that impossible to ship again.
function onUpdate() {
    t += Bsengine.getDeltaTime();

    sinceWave += Bsengine.getDeltaTime();
    if (sinceWave > 3.0) {
        sinceWave = 0.0;
        waves += 1;
        // On ClientPlayer, which this peer owns -- the server checks that.
        Bsengine.network.callRpc("ClientPlayer", "wave", { n: waves });
    }

    // Counter-circle, opposite the server's.
    Bsengine.setPosition(
        "ClientPlayer",
        Math.cos(-t) * RADIUS + CENTRE_X,
        0.5,
        Math.sin(-t) * RADIUS,
    );
}
