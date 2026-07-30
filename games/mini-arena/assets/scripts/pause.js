let paused = false;
let escKeyWasDown = false;
let menuBuilt = false;

function buildMenu() {
    Bsengine.ui.setPanel("pauseBg", "Paused", 490, 260, 300, 160);
    Bsengine.ui.setButton("resumeBtn", "Resume", 520, 320, 240, 40);
    Bsengine.ui.setButton("quitBtn", "Quit", 520, 370, 240, 40);
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
