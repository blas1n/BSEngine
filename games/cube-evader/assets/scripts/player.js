const PLAYER_SPEED = 0.1;
const ENEMY_SPEED = 0.05;
const COLLISION_DIST = 1.2;
const BOUNDS = 9.0;
const ENEMY_NAMES = ["Enemy1", "Enemy2", "Enemy3"];

const enemies = ENEMY_NAMES.map(() => {
    const angle = Math.random() * Math.PI * 2;
    return {
        dx: Math.cos(angle),
        dz: Math.sin(angle),
        timer: 0,
        changeInterval: 60 + Math.floor(Math.random() * 60),
    };
});

let score = 0;

function onUpdate() {
    const pt = Bsengine.getTransform("Player");
    if (!pt) return;

    let { x, y, z } = pt;
    if (Bsengine.isKeyPressed("W")) z -= PLAYER_SPEED;
    if (Bsengine.isKeyPressed("S")) z += PLAYER_SPEED;
    if (Bsengine.isKeyPressed("A")) x -= PLAYER_SPEED;
    if (Bsengine.isKeyPressed("D")) x += PLAYER_SPEED;

    x = Math.max(-BOUNDS, Math.min(BOUNDS, x));
    z = Math.max(-BOUNDS, Math.min(BOUNDS, z));
    Bsengine.setTransform("Player", x, y, z);

    for (let i = 0; i < ENEMY_NAMES.length; i++) {
        const name = ENEMY_NAMES[i];
        const state = enemies[i];
        const et = Bsengine.getTransform(name);
        if (!et) continue;

        state.timer++;
        if (state.timer >= state.changeInterval) {
            state.timer = 0;
            state.changeInterval = 60 + Math.floor(Math.random() * 60);
            const angle = Math.random() * Math.PI * 2;
            state.dx = Math.cos(angle);
            state.dz = Math.sin(angle);
        }

        let ex = et.x + state.dx * ENEMY_SPEED;
        let ez = et.z + state.dz * ENEMY_SPEED;

        if (ex < -BOUNDS || ex > BOUNDS) { state.dx = -state.dx; ex = et.x; }
        if (ez < -BOUNDS || ez > BOUNDS) { state.dz = -state.dz; ez = et.z; }

        Bsengine.setTransform(name, ex, et.y, ez);

        const dx = x - ex;
        const dz = z - ez;
        if (Math.sqrt(dx * dx + dz * dz) < COLLISION_DIST) {
            score++;
            Bsengine.log("Caught! Score: " + score);
            Bsengine.setTransform("Player", 0, 0.5, 0);
        }
    }
}
