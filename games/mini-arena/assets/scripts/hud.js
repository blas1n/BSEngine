let score = 0;

Bsengine.onMessage("Hud", "score", (data) => {
    score += data.amount;
});

Bsengine.onMessage("Hud", "scoreLoaded", (data) => {
    score = data.value;
});

function onUpdate(self) {
    const hp = Math.round(Bsengine.getShield("Player"));
    const maxHp = Math.round(Bsengine.getMaxShield("Player"));
    Bsengine.setHudText("health", `HP: ${hp}/${maxHp}`);
    Bsengine.setHudText("score", `Score: ${score}`);
    Bsengine.setSaveField("Player", "score", String(score));
}
