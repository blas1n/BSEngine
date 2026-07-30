let shaderAssigned = false;

function onUpdate(self) {
    if (!shaderAssigned) {
        Bsengine.setShader(self, "assets/shaders/glow.wgsl");
        shaderAssigned = true;
    }

    const pos = Bsengine.getPosition(self);
    const player = Bsengine.getPosition("Player");
    if (!pos || !player) return;
    const dx = player.x - pos.x;
    const dz = player.z - pos.z;
    if (Math.sqrt(dx * dx + dz * dz) < 0.6) {
        Bsengine.sendMessage("Hud", "score", { amount: 50 });
        Bsengine.destroy(self);
    }
}
