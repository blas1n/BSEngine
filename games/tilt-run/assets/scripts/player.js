// Newtons, applied for the one step it is added to. The old value was 0.045 --
// fifty times smaller -- because forces used to survive the step and pile up,
// so holding a key ramped the push from 0.045 to whatever the hold was long
// enough to reach, and releasing the key never took it away. The ball weighs
// about 0.52kg, so 2.5N is roughly 3.4 m/s^2 once a sphere's rolling inertia is
// paid for: a couple of seconds of holding W to cross a level.
const FORCE_MAGNITUDE = 2.5;
const FALL_Y_THRESHOLD = -5.0;

// Where the ball began, captured on the first frame and restored after a
// fall. One record rather than seven loose numbers, now that the transform
// accessors hand back value types.
let start = null;
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
    // getTransform, not getPosition: this script also remembers the ball's
    // starting rotation so a fall can restore it.
    const t = Bsengine.getTransform(self);
    if (!t) return;

    if (start === null) {
        start = { position: t.position.clone(), rotation: t.rotation.clone() };
    }

    if (t.position.y < FALL_Y_THRESHOLD) {
        Bsengine.setPosition(self, start.position);
        Bsengine.setRotation(self, start.rotation);
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
