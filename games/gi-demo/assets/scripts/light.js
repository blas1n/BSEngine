// Circles the light round the room, so the light it bounces -- red off the
// left wall, green off the right -- moves with it: the realtime probe volume
// follows a moving light, which a bake cannot.
const RADIUS = 1.8;
const HEIGHT = 2.2;
const SPEED = 0.6;

function onUpdate(self) {
    const t = Bsengine.getTime() * SPEED;
    Bsengine.setPosition(self, Math.cos(t) * RADIUS, HEIGHT, Math.sin(t) * RADIUS);
}
