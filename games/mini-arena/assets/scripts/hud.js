let score = 0;

Bsengine.onMessage("Hud", "score", (data) => {
    score += data.amount;
});

Bsengine.onMessage("Hud", "scoreLoaded", (data) => {
    score = data.value;
});

// The health bar sits in the top-right corner. The engine draws HUD lines
// (`setHudText`: the score, the death message) stacked down from the top-left,
// where no script can move them, and the bar used to be there too -- at
// (10, 10), on top of "Score: 0". Anchored right, the offsets are from the
// window's right edge: the bar ends 10 units in, as it began 10 units in
// before, and the label keeps its 6-unit inset.
const TOP_RIGHT = { anchor: "top-right" };

function onUpdate(self) {
    const hp = Math.round(Bsengine.getShield("Player"));
    const maxHp = Math.round(Bsengine.getMaxShield("Player"));
    Bsengine.ui.setProgressBar("healthbar", -210, 10, 200, 24, hp / maxHp, TOP_RIGHT);
    Bsengine.ui.setLabel("healthbar_label", `HP: ${hp}/${maxHp}`, -204, 13, 16, TOP_RIGHT);
    Bsengine.setHudText("score", `Score: ${score}`);
    Bsengine.setSaveField("Player", "score", String(score));
}
