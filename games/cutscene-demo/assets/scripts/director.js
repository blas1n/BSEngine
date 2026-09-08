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

    // Reported every frame so the recording can read it both during the
    // cutscene and after the stop above. Without the second reading nothing
    // observes that stopping took effect -- `stop` would be a call the demo
    // makes and never checks.
    Bsengine.setHudText("playing", Bsengine.timeline.isPlaying(name) ? "yes" : "no");
}
