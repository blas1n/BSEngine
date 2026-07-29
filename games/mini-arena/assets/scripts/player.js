const MOVE_SPEED = 4.5;
const RUN_MULTIPLIER = 1.8;
const ATTACK_RANGE = 2.0;
const ATTACK_DAMAGE = 15.0;
const CHECKPOINT_PATH = "games/mini-arena/checkpoint.json";
let attackKeyWasDown = false;
let enterKeyWasDown = false;

function onUpdate(self) {
    const enterDown = Bsengine.isKeyDown("Enter");
    const dead = Bsengine.getShield(self) <= 0.0;
    if (dead) {
        Bsengine.setHudText("status", "You died. Press Enter to reload checkpoint.");
        if (enterDown && !enterKeyWasDown) {
            Bsengine.load(CHECKPOINT_PATH);
            const restoredScore = Bsengine.getSaveField(self, "score");
            Bsengine.sendMessage("Hud", "scoreLoaded", { value: parseInt(restoredScore || "0", 10) });
            const restoredHealth = parseFloat(Bsengine.getSaveField(self, "health") || "100");
            Bsengine.restoreShield(self, restoredHealth - Bsengine.getShield(self));
        }
        enterKeyWasDown = enterDown;
        return;
    }
    enterKeyWasDown = enterDown;

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

    const attackDown = Bsengine.isKeyDown("Space");
    if (attackDown && !attackKeyWasDown) {
        const pos = Bsengine.getPosition(self);
        const fwd = Bsengine.getForwardVector(self);
        if (pos && fwd) {
            const hit = Bsengine.raycast(
                { x: pos.x, y: pos.y + 0.9, z: pos.z },
                { x: fwd[0], y: fwd[1], z: fwd[2] },
                ATTACK_RANGE
            );
            if (hit && hit.entityName === "Enemy") {
                Bsengine.damageShield("Enemy", ATTACK_DAMAGE);
                Bsengine.sendMessage("Enemy", "hit", { dirX: fwd[0], dirZ: fwd[2] });
                if (Bsengine.getShield("Enemy") <= 0.0) {
                    Bsengine.destroy("Enemy");
                    Bsengine.sendMessage("Hud", "score", { amount: 100 });
                }
            }
        }
    }
    attackKeyWasDown = attackDown;

    Bsengine.setSaveField(self, "health", String(Bsengine.getShield(self)));
    Bsengine.save(CHECKPOINT_PATH);
}
