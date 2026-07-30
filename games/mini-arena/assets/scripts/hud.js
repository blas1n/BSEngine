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
    Bsengine.ui.setProgressBar("healthbar", 10, 10, 200, 24, hp / maxHp);
    Bsengine.ui.setLabel("healthbar_label", `HP: ${hp}/${maxHp}`, 16, 13, 16);
    Bsengine.setHudText("score", `Score: ${score}`);
    Bsengine.setSaveField("Player", "score", String(score));
}
