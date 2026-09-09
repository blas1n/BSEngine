// Plays the glTF's own clip so `update_skinned_meshes` recomputes joint
// matrices every frame -- which is the cost being measured.
//
// A fox at rest is skinned once and then costs nothing, so a level full of
// still foxes would measure nothing at all. The claim under test ("CPU skinning
// is deliberately for 1~2 characters") is about *animating* characters.
let started = false;

function onUpdate(name) {
    if (!started) {
        Bsengine.playAnimation(name, "Survey");
        started = true;
    }
}
