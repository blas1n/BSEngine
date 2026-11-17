let paused = false;
let escKeyWasDown = false;
let menuBuilt = false;

// Offsets from the centre of the window, in UI units. At the 1280x720 design
// size they are the absolute positions this menu always had (490/260 and
// 520/320 and 520/370), but they follow the window: the UI scales with the
// shorter side (`[ui] scale`), so a phone held upright is 720 units wide, and
// a menu at x = 490 with a width of 300 ran off its right edge.
function buildMenu() {
    const center = { anchor: "center" };
    Bsengine.ui.setPanel("pauseBg", "Paused", -150, -100, 300, 160, center);
    Bsengine.ui.setButton("resumeBtn", "Resume", -120, -40, 240, 40, center);
    Bsengine.ui.setButton("quitBtn", "Quit", -120, 10, 240, 40, center);
    menuBuilt = true;
}

function teardownMenu() {
    Bsengine.ui.remove("pauseBg");
    Bsengine.ui.remove("resumeBtn");
    Bsengine.ui.remove("quitBtn");
    menuBuilt = false;
}

function onUpdate(self) {
    const escDown = Bsengine.isKeyDown("Escape");
    if (escDown && !escKeyWasDown) {
        paused = !paused;
        if (paused) { buildMenu(); Bsengine.pause(); } else { teardownMenu(); Bsengine.resume(); }
    }
    escKeyWasDown = escDown;

    if (!paused) return;

    if (Bsengine.ui.isClicked("resumeBtn")) {
        paused = false;
        teardownMenu();
        Bsengine.resume();
    }
    if (Bsengine.ui.isClicked("quitBtn")) {
        Bsengine.quit();
    }
}
