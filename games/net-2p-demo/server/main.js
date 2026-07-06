// Server-side script: hosts on port 7777, owns ServerPlayer
Bsengine.network.startServer(7777);

let t = 0.0;
Bsengine.onUpdate((dt) => {
    t += dt;
    // Move ServerPlayer in a circle
    const x = Math.cos(t) * 2.0 - 2.0;
    const z = Math.sin(t) * 2.0;
    Bsengine.setPosition("ServerPlayer", x, 0.5, z);
});