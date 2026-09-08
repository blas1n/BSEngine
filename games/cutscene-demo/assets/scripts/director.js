// Starts the intro on the first frame and reports when it ends.
//
// `eventFired` asks about *this frame*, so the HUD line below is written on the
// one frame the beat lands rather than every frame after it.
let started = false;

function onUpdate(name) {
    if (!started) {
        Bsengine.timeline.play(name);
        started = true;
    }

    if (Bsengine.timeline.eventFired("intro_over")) {
        Bsengine.setHudText("cutscene", "intro complete");
        Bsengine.timeline.stop(name);
    }
}
