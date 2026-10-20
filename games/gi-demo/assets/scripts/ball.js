// Rolls the ball to and fro along the floor beside the red wall. It is not
// static, so the realtime probe volume traces it where it is each frame: the
// floor under it darkens as it passes and the red off the wall dims behind it
// -- an indirect shadow that moves, which neither a bake nor a static-only GI
// can give.
const X = -1.2;
const Y = 0.5;
const REACH = 1.6;
const SPEED = 0.45;

function onUpdate(self) {
    const t = Bsengine.getTime() * SPEED;
    Bsengine.setPosition(self, X, Y, Math.sin(t) * REACH);
}
