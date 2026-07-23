const IS_FINAL_LEVEL = true;

let cleared = false;

Bsengine.onCollision("Goal", (other, started) => {
    if (!started || other !== "Ball" || cleared) return;
    cleared = true;
    if (IS_FINAL_LEVEL) {
        Bsengine.setHudText(1, "All Clear!");
        Bsengine.sendMessage("Ball", "gameOver", true);
    }
});
