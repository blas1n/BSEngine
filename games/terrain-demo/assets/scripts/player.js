// Newtons, applied for the one step it is added to -- same convention
// tilt-run's player.js uses (see that script's own comment for the physics
// behind the number): the terrain here is shallow and 6x wider than a single
// tilt-run level, so a slightly larger push keeps a WASD lap across it from
// feeling glacial.
const FORCE_MAGNITUDE = 3.5;

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
