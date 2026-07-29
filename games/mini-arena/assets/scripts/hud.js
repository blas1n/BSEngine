let score = 0;

Bsengine.onMessage("Hud", "score", (data) => {
    score += data.amount;
});

function onUpdate(self) {
    const hp = Math.round(Bsengine.getShield("Player"));
    const maxHp = Math.round(Bsengine.getMaxShield("Player"));
    Bsengine.setHudText("health", `HP: ${hp}/${maxHp}`);
    Bsengine.setHudText("score", `Score: ${score}`);
}
