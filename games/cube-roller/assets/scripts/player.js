const SPEED = 0.1;

function onUpdate(self) {
    const t = Bsengine.getTransform(self);
    if (!t) return;

    let { x, y, z } = t;

    if (Bsengine.isKeyPressed("W")) z -= SPEED;
    if (Bsengine.isKeyPressed("S")) z += SPEED;
    if (Bsengine.isKeyPressed("A")) x -= SPEED;
    if (Bsengine.isKeyPressed("D")) x += SPEED;

    Bsengine.setTransform(self, x, y, z);
}
