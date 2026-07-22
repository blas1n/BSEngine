const FORCE_MAGNITUDE = 0.045;
const FALL_Y_THRESHOLD = -5.0;

let startX = null;
let startY = null;
let startZ = null;

function onUpdate(self) {
    const t = Bsengine.getTransform(self);
    if (!t) return;

    if (startX === null) {
        startX = t.x;
        startY = t.y;
        startZ = t.z;
    }

    if (t.y < FALL_Y_THRESHOLD) {
        Bsengine.setTransform(self, startX, startY, startZ);
        Bsengine.setVelocity(self, 0.0, 0.0, 0.0);
        Bsengine.setAngularVelocity(self, 0.0, 0.0, 0.0);
        Bsengine.setHudText(0, "Fell! Retry");
        return;
    }

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
