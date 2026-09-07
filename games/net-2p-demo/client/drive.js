// Input-driven movement for the predicted player.
//
// Both peers run this file: the client to predict, the server to decide. That
// is the whole point of the shared-simulation model -- one movement rule, not
// one per side that could disagree.
//
// `isKeyPressed` reads the input of whichever entity this script is running
// for, so on the server it sees the *client's* keys rather than the keyboard of
// the machine hosting the game.
const SPEED = 4.0;

function onUpdate(name) {
    const dt = Bsengine.getDeltaTime();
    let dx = 0.0;
    let dz = 0.0;
    if (Bsengine.isKeyPressed("A")) { dx -= 1.0; }
    if (Bsengine.isKeyPressed("D")) { dx += 1.0; }
    if (Bsengine.isKeyPressed("W")) { dz -= 1.0; }
    if (Bsengine.isKeyPressed("S")) { dz += 1.0; }
    if (dx === 0.0 && dz === 0.0) { return; }

    const p = Bsengine.getTransform(name).position;
    Bsengine.setPosition(name, p.x + dx * SPEED * dt, p.y, p.z + dz * SPEED * dt);
}
