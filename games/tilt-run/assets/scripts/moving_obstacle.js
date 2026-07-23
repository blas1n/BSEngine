const AMPLITUDE = 2.0;
const PERIOD_SECONDS = 2.5;

let centerX = null;
let centerY = null;
let centerZ = null;

function onUpdate(self) {
    if (centerX === null) {
        const t = Bsengine.getTransform(self);
        if (!t) return;
        centerX = t.x;
        centerY = t.y;
        centerZ = t.z;
    }
    const time = Bsengine.getTime();
    const phase = (time / PERIOD_SECONDS) * Math.PI * 2.0;
    const offset = Math.sin(phase) * AMPLITUDE;
    Bsengine.setTransform(self, centerX + offset, centerY, centerZ);
}
