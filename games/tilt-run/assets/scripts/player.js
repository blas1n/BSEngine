const FORCE_MAGNITUDE = 0.045;
const FALL_Y_THRESHOLD = -5.0;

let startX = null;
let startY = null;
let startZ = null;
let startRotX = null;
let startRotY = null;
let startRotZ = null;
let startRotW = null;
let gameOver = false;

// Sent by the final level's goal script once IS_FINAL_LEVEL clears — stops
// the ball for good instead of leaving it controllable forever with no
// further objective.
Bsengine.onMessage("Ball", "gameOver", () => {
    gameOver = true;
    Bsengine.setVelocity("Ball", 0.0, 0.0, 0.0);
    Bsengine.setAngularVelocity("Ball", 0.0, 0.0, 0.0);
    Bsengine.resetForces("Ball");
});

function onUpdate(self) {
    const t = Bsengine.getTransform(self);
    if (!t) return;

    if (startX === null) {
        startX = t.x;
        startY = t.y;
        startZ = t.z;
        startRotX = t.rx;
        startRotY = t.ry;
        startRotZ = t.rz;
        startRotW = t.rw;
    }

    if (t.y < FALL_Y_THRESHOLD) {
        Bsengine.setTransform(self, startX, startY, startZ);
        Bsengine.setRotation(self, startRotX, startRotY, startRotZ, startRotW);
        Bsengine.setVelocity(self, 0.0, 0.0, 0.0);
        Bsengine.setAngularVelocity(self, 0.0, 0.0, 0.0);
        Bsengine.resetForces(self);
        Bsengine.setHudText(0, "Fell! Retry");
        return;
    }

    if (gameOver) return;

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
