const MOVE_SPEED = 4.5;
const RUN_MULTIPLIER = 1.8;

function onUpdate(self) {
    let dx = 0.0;
    let dz = 0.0;
    if (Bsengine.isKeyDown("W")) dz -= 1.0;
    if (Bsengine.isKeyDown("S")) dz += 1.0;
    if (Bsengine.isKeyDown("A")) dx -= 1.0;
    if (Bsengine.isKeyDown("D")) dx += 1.0;

    const running = Bsengine.isKeyDown("Left") || Bsengine.isKeyDown("Right");
    const len = Math.sqrt(dx * dx + dz * dz);
    let speed = 0.0;

    if (len > 0.0001) {
        dx /= len;
        dz /= len;
        speed = running ? MOVE_SPEED * RUN_MULTIPLIER : MOVE_SPEED;

        const dt = Bsengine.getDeltaTime();
        const pos = Bsengine.getPosition(self);
        if (pos) {
            Bsengine.setTransform(self, pos.x + dx * speed * dt, pos.y, pos.z + dz * speed * dt);
        }

        const yaw = Math.atan2(dx, dz);
        Bsengine.setRotationEuler(self, 0.0, yaw * 180.0 / Math.PI, 0.0);
    }

    Bsengine.asmSetFloat(self, "speed", speed);
}
