// Server-side script: hosts on port 7777, owns ServerPlayer
Bsengine.network.startServer(7777);

const RADIUS = 2.0;
const CENTRE_X = -2.0;

let t = 0.0;

// The entry point is a top-level `onUpdate(self)`, the same as every other
// script in games/. This file used to call `Bsengine.onUpdate(cb)` and
// `Bsengine.setPosition(...)`, neither of which existed, so it threw on its
// first frame and neither player ever moved. See
// crates/bsengine-scripting/tests/prelude_names.rs for the guard that now
// makes that impossible to ship again.
function onUpdate() {
    t += Bsengine.getDeltaTime();
    Bsengine.setPosition(
        "ServerPlayer",
        Math.cos(t) * RADIUS + CENTRE_X,
        0.5,
        Math.sin(t) * RADIUS,
    );
}
