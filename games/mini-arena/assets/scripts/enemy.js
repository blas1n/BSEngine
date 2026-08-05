let navReady = false;
let humStarted = false;
const HUM_PATH = "assets/sounds/enemy-hum.wav";
const CHASE_RANGE = 12.0;
const REPATH_INTERVAL = 0.5;
let repathTimer = 0.0;
const CONTACT_RANGE = 1.2;
const CONTACT_DAMAGE = 5.0;
const CONTACT_COOLDOWN = 1.0;
let contactCooldown = 0.0;

// Knockback is the physics engine's job now. The Enemy is a Dynamic body with
// a CharacterBody, so an impulse is simply an impulse: it decays through the
// body's damping, it stops against walls, and the nav agent steers *through* it
// instead of erasing it, because the agent applies impulses too.
//
// What this replaced was a hand-rolled timer that pushed the transform with
// moveEntity for a quarter of a second, ignoring walls and mass alike. There is
// no "knockback" concept left in this script — there is a force applied on hit.
// Chosen to land near the old fake's total displacement of about 2 units, so
// the fight reads the same. The Enemy's capsule masses about 0.42, so this is
// roughly a 6 m/s kick; with no damping on a scene-authored Dynamic body it is
// the nav agent's own counter-steering that brings it to a stop.
const KNOCKBACK_IMPULSE = 2.5;

Bsengine.onMessage("Enemy", "hit", (data) => {
    Bsengine.addImpulse(
        "Enemy",
        data.dirX * KNOCKBACK_IMPULSE,
        0.0,
        data.dirZ * KNOCKBACK_IMPULSE,
    );
});

function onUpdate(self) {
    if (Bsengine.isPaused()) return;

    if (!navReady) {
        // 40x40 grid, 1 unit cells, centered on the arena (matches Ground's
        // 20x20 half-extent from main.ron).
        Bsengine.navmesh.init(40, 40, 1.0, -20.0, 0.0, -20.0);
        navReady = true;
    }

    if (!humStarted) {
        // Positional: the hum comes from wherever the Enemy is, so it pans and
        // fades as it circles the Player. The Enemy carries AudioEmitter and
        // the Camera carries AudioListener; nothing here computes volume or
        // panning, and nothing here names a position — the entity is the
        // position.
        Bsengine.playSound3D("Enemy", HUM_PATH, { volume: 0.4, loop: true });
        humStarted = true;
    }

    const player = Bsengine.getPosition("Player");
    const me = Bsengine.getPosition(self);
    if (!player || !me) return;

    const dx = player.x - me.x;
    const dz = player.z - me.z;
    const dist = Math.sqrt(dx * dx + dz * dz);

    if (dist > CHASE_RANGE) {
        Bsengine.navmesh.clearDestination(self);
        return;
    }

    const dt = Bsengine.getDeltaTime();
    repathTimer -= dt;
    if (repathTimer <= 0.0) {
        Bsengine.navmesh.setDestination(self, player.x, player.y, player.z);
        repathTimer = REPATH_INTERVAL;
    }

    contactCooldown = Math.max(0.0, contactCooldown - dt);
    if (dist < CONTACT_RANGE && contactCooldown <= 0.0) {
        Bsengine.damageShield("Player", CONTACT_DAMAGE);
        contactCooldown = CONTACT_COOLDOWN;
    }
}
