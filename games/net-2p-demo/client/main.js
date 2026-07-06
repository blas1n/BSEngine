// Client-side script: connects to localhost:7777, moves ClientPlayer
Bsengine.network.connect("127.0.0.1", 7777);

let t = 0.0;
Bsengine.onUpdate((dt) => {
    t += dt;
    // Move ClientPlayer in a counter-circle
    const x = Math.cos(-t) * 1.5 + 2.0;
    const z = Math.sin(-t) * 1.5;
    Bsengine.setPosition("ClientPlayer", x, 0.5, z);
});