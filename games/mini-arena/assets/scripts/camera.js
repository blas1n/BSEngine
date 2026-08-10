const OFFSET = { x: 0.0, y: 4.0, z: 7.0 };
const LERP_SPEED = 4.0;

function onUpdate(self) {
    const target = Bsengine.getPosition("Player");
    if (!target) return;

    const cam = Bsengine.getPosition(self);
    if (!cam) return;

    const dt = Bsengine.getDeltaTime();
    const t = Math.min(1.0, LERP_SPEED * dt);

    const desiredX = target.x + OFFSET.x;
    const desiredY = target.y + OFFSET.y;
    const desiredZ = target.z + OFFSET.z;

    const newX = cam.x + (desiredX - cam.x) * t;
    const newY = cam.y + (desiredY - cam.y) * t;
    const newZ = cam.z + (desiredZ - cam.z) * t;

    Bsengine.setPosition(self, newX, newY, newZ);
    Bsengine.lookAt(self, target.x, target.y + 0.5, target.z);
}
