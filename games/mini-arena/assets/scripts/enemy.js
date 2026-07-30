let navReady = false;
const CHASE_RANGE = 12.0;
const REPATH_INTERVAL = 0.5;
let repathTimer = 0.0;
const CONTACT_RANGE = 1.2;
const CONTACT_DAMAGE = 5.0;
const CONTACT_COOLDOWN = 1.0;
let contactCooldown = 0.0;

let knockbackTimer = 0.0;
let knockbackDir = { x: 0.0, z: 0.0 };
const KNOCKBACK_SPEED = 8.0;
const KNOCKBACK_DURATION = 0.25;

Bsengine.onMessage("Enemy", "hit", (data) => {
    knockbackTimer = KNOCKBACK_DURATION;
    knockbackDir = { x: data.dirX, z: data.dirZ };
});

function onUpdate(self) {
    if (knockbackTimer > 0.0) {
        const dt = Bsengine.getDeltaTime();
        const step = KNOCKBACK_SPEED * dt;
        Bsengine.moveEntity(self, knockbackDir.x * step, 0.0, knockbackDir.z * step);
        knockbackTimer -= dt;
        return;
    }

    if (!navReady) {
        // 40x40 grid, 1 unit cells, centered on the arena (matches Ground's
        // 20x20 half-extent from main.ron).
        Bsengine.navmesh.init(40, 40, 1.0, -20.0, 0.0, -20.0);
        navReady = true;
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
