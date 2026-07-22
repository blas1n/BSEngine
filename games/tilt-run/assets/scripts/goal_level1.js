const NEXT_SCENE = "assets/scenes/level2.ron";
const IS_FINAL_LEVEL = false;

let cleared = false;

Bsengine.onCollision("Goal", (other, started) => {
    if (!started || other !== "Ball" || cleared) return;
    cleared = true;
    if (IS_FINAL_LEVEL) {
        Bsengine.setHudText(1, "All Clear!");
    } else {
        Bsengine.setHudText(1, "Level Complete!");
        Bsengine.loadScene(NEXT_SCENE);
    }
});
