// Adapted from games/terrain-demo/assets/scripts/player.js, which already
// drives this exact character controller correctly. Writing a second movement
// idiom for the same controller would be invention for its own sake.
//
// Newtons, applied for the one step they are added to. Larger than
// terrain-demo's 3.5 because this terrain is 160x160 against its 40x40 -- a
// WASD lap has four times the distance to cover.
const FORCE_MAGNITUDE = 8.0;

function onUpdate(self) {
    let fx = 0.0;
    let fz = 0.0;
    if (Bsengine.isKeyPressed("W")) fz -= FORCE_MAGNITUDE;
    if (Bsengine.isKeyPressed("S")) fz += FORCE_MAGNITUDE;
    if (Bsengine.isKeyPressed("A")) fx -= FORCE_MAGNITUDE;
    if (Bsengine.isKeyPressed("D")) fx += FORCE_MAGNITUDE;
    if (fx !== 0.0 || fz !== 0.0) {
        Bsengine.addForce(self, fx, 0.0, fz);
    }
}
