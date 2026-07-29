let shaderAssigned = false;
let elapsed = 0.0;
const PULSE_SPEED = 3.0;
const BASE_R = 0.2, BASE_G = 0.6, BASE_B = 1.0;

function onUpdate(self) {
    if (!shaderAssigned) {
        // CWD-relative, same as `gltf:` scene paths -- NOT project-dir-relative
        // like `script:` paths. Confirmed against ScriptCommand::SetCustomShader
        // in bsengine-scripting/src/plugin.rs (no ProjectDir join) and the
        // std::fs::read_to_string call in bsengine-render/src/plugin.rs.
        Bsengine.setShader(self, "games/mini-arena/assets/shaders/glow.wgsl");
        shaderAssigned = true;
    }

    const dt = Bsengine.getDeltaTime();
    elapsed += dt;
    const pulse = 0.6 + 0.4 * Math.sin(elapsed * PULSE_SPEED);
    Bsengine.setEmissive(self, BASE_R * pulse, BASE_G * pulse, BASE_B * pulse);

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
