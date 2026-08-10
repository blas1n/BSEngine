const OFFSET_Y = 4.0;
const OFFSET_Z = 6.0;

function onUpdate(self) {
    const ball = Bsengine.getPosition("Ball");
    if (!ball) return;
    Bsengine.setPosition(self, ball.x, ball.y + OFFSET_Y, ball.z + OFFSET_Z);
    Bsengine.lookAt(self, ball.x, ball.y, ball.z);
}
