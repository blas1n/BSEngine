const MOVE_SPEED = 4.5;
const RUN_MULTIPLIER = 1.8;
const ATTACK_RANGE = 2.0;
const ATTACK_DAMAGE = 15.0;
// Player/Enemy collider radius (Capsule radius: 0.35 in main.ron). The attack
// raycast origin must start outside the caster's own collider by more than
// this, or Bsengine.raycast's underlying Rapier query (solid ray casting)
// reports an immediate self-hit at distance 0 against the caster's own body
// -- which is exactly what happened before this offset was added: `hit`
// always resolved to `{entityName: "Player", distance: 0}` and the
// `hit.entityName === "Enemy"` check could never pass, no matter the aim,
// range, or approach. Confirmed via a throwaway HUD-text probe of the raw
// hit result during this plan's E2E test authoring. (A second, independent
// bug compounded this: Bsengine.raycast()'s hit object used to serialize as
// `{ entity_name: ... }`, not `{ entityName: ... }`, so `hit.entityName`
// below was always undefined regardless of what was actually hit -- fixed
// engine-side in bsengine-scripting's RaycastHitJson.)
const SELF_RADIUS_CLEARANCE = 0.5;
const CHECKPOINT_PATH = "games/mini-arena/checkpoint.json";
let attackKeyWasDown = false;
let enterKeyWasDown = false;

function onUpdate(self) {
    if (Bsengine.isPaused()) return;

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
            // Without this, "You died..." stays on screen indefinitely after
            // a successful reload -- nothing else ever clears the "status"
            // HUD slot once the death branch stops running each frame.
            Bsengine.clearHudText("status");
        }
        enterKeyWasDown = enterDown;
        return;
    }
    enterKeyWasDown = enterDown;

    let dx = 0.0;
    let dz = 0.0;
    if (Bsengine.isKeyPressed("W")) dz -= 1.0;
    if (Bsengine.isKeyPressed("S")) dz += 1.0;
    if (Bsengine.isKeyPressed("A")) dx -= 1.0;
    if (Bsengine.isKeyPressed("D")) dx += 1.0;

    const running = Bsengine.isKeyPressed("Left") || Bsengine.isKeyPressed("Right");
    const len = Math.sqrt(dx * dx + dz * dz);
    let speed = 0.0;

    if (len > 0.0001) {
        dx /= len;
        dz /= len;
        speed = running ? MOVE_SPEED * RUN_MULTIPLIER : MOVE_SPEED;

        const dt = Bsengine.getDeltaTime();
        Bsengine.moveEntity(self, dx * speed * dt, 0.0, dz * speed * dt);

        // Bsengine.getForwardVector() returns rotation * (0,0,-1). Setting
        // yaw from atan2(dx, dz) directly (this component's original code)
        // makes that forward vector come out as (-dx, 0, -dz) -- exactly
        // backwards from the direction just moved in, confirmed with a
        // throwaway HUD-text probe (pressing S+D, i.e. dx=dz=+0.71, reported
        // fwd=[-0.71,0,-0.71]) during this plan's E2E test authoring. Every
        // consumer of "forward" (this attack raycast, and the knockback
        // direction sent to Enemy below) was therefore aiming/pushing the
        // opposite way the player was actually walking. atan2(-dx, -dz)
        // yields the yaw whose forward vector equals (dx, 0, dz).
        const yaw = Math.atan2(-dx, -dz);
        Bsengine.setRotationEuler(self, 0.0, yaw * 180.0 / Math.PI, 0.0);
    }

    Bsengine.asmSetFloat(self, "speed", speed);

    const attackDown = Bsengine.isKeyDown("Space");
    if (attackDown && !attackKeyWasDown) {
        const pos = Bsengine.getPosition(self);
        const fwd = Bsengine.getForwardVector(self);
        if (pos && fwd) {
            // Attack/enemy collider is Capsule(half_height: 0.5, radius: 0.35)
            // -- a total vertical half-extent of 0.85 centered on the entity's
            // Transform.y (0.0 for both Player and Enemy). +0.9 sat just above
            // that (0.85), so this horizontal-only ray (yaw-only rotation,
            // fwd.y is always ~0) skimmed over the top of the collider and
            // could never register a hit at any range or angle. +0.5 keeps
            // the ray inside the capsule's cylindrical midsection.
            const hit = Bsengine.raycast(
                {
                    x: pos.x + fwd.x * SELF_RADIUS_CLEARANCE,
                    y: pos.y + 0.5,
                    z: pos.z + fwd.z * SELF_RADIUS_CLEARANCE,
                },
                { x: fwd.x, y: fwd.y, z: fwd.z },
                ATTACK_RANGE
            );
            if (hit && hit.entityName === "Enemy") {
                // Sparks at the impact point. The emitter is a parked entity
                // that gets moved there first, because a burst emits from
                // wherever its entity currently is.
                if (hit.point) {
                    Bsengine.setPosition("HitSparks", hit.point[0], hit.point[1], hit.point[2]);
                    Bsengine.burstParticles("HitSparks");
                }
                // Bsengine.damageShield() only queues a ScriptCommand,
                // applied to the world after every script finishes this
                // frame; Bsengine.getShield() reads a snapshot captured at
                // the *start* of the frame. Checking getShield("Enemy")
                // right after damageShield() therefore always sees the
                // pre-this-hit value -- it takes one extra hit past the
                // "should be lethal" point before a stale re-check finally
                // reflects the damage (30-max/15-per-hit Enemy took 3 hits
                // to die instead of the intended 2), confirmed by
                // instrumenting the raw hit/shield values during this plan's
                // E2E test authoring. Computing the post-hit value locally
                // from the pre-hit snapshot (already correct, since any
                // *earlier* hit's damage was flushed by a prior frame) side
                // steps the staleness entirely.
                const preHitShield = Bsengine.getShield("Enemy");
                Bsengine.damageShield("Enemy", ATTACK_DAMAGE);
                Bsengine.sendMessage("Enemy", "hit", { dirX: fwd.x, dirZ: fwd.z });
                if (preHitShield - ATTACK_DAMAGE <= 0.0) {
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
