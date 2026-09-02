// Drives the car from WASD through the three vehicle input setters.
//
// The engine force is small because the chassis is light: its collider is
// 1.8 x 0.8 x 4.0 at the default density of 1.0, so it masses about 5.8 kg.
// A few hundred newtons on that launches it off the ground entirely, which is
// how the first version of the braking test ended up comparing two airborne
// runs that decelerated identically.
const ENGINE_FORCE = 20.0;
const BRAKE_FORCE = 50.0;
const STEER_ANGLE = 0.5;

function onUpdate(self) {
    let throttle = 0.0;
    if (Bsengine.isKeyPressed("W")) throttle += ENGINE_FORCE;
    if (Bsengine.isKeyPressed("S")) throttle -= ENGINE_FORCE;

    let steering = 0.0;
    if (Bsengine.isKeyPressed("A")) steering += STEER_ANGLE;
    if (Bsengine.isKeyPressed("D")) steering -= STEER_ANGLE;

    // Set all three every frame rather than only on change: these are held
    // inputs, and leaving a stale throttle set is how a car keeps accelerating
    // after the key is released.
    Bsengine.vehicle.setThrottle(self, throttle);
    Bsengine.vehicle.setSteering(self, steering);
    Bsengine.vehicle.setBrake(self, Bsengine.isKeyPressed("Space") ? BRAKE_FORCE : 0.0);
}
