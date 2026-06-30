const PADDLE_SPEED = 0.12;
const BOUNDS_X = 7.5;
const TOP_Y = 9.0;
const BOTTOM_Y = -6.0;
const PADDLE_Y = -4.0;
const PADDLE_HALF_W = 1.8;
const BRICK_HALF_W = 1.25;
const BRICK_HALF_H = 0.4;
const OFF_SCREEN = 100;

const BRICK_COLS = [-4.5, -1.5, 1.5, 4.5];
const BRICK_ROWS = [8.0, 6.5, 5.0, 3.5, 2.0];

const BRICK_NAMES = [];
for (let row = 0; row < 5; row++) {
    for (let col = 0; col < 4; col++) {
        BRICK_NAMES.push("Brick_" + row + "_" + col);
    }
}

let ballDx = 0.05;
let ballDy = 0.09;
let bricksRemaining = 20;
let gameOver = false;
let gameWon = false;
let restartCooldown = 0;

function resetGame() {
    ballDx = 0.05;
    ballDy = 0.09;
    bricksRemaining = 20;
    gameOver = false;
    gameWon = false;
    restartCooldown = 0;
    Bsengine.setTransform("Ball", 0.0, -2.0, 0.0);
    Bsengine.setTransform("Paddle", 0.0, -4.0, 0.0);
    for (let row = 0; row < 5; row++) {
        for (let col = 0; col < 4; col++) {
            Bsengine.setTransform("Brick_" + row + "_" + col, BRICK_COLS[col], BRICK_ROWS[row], 0.0);
        }
    }
    Bsengine.log("Restarted! Good luck!");
}

function onUpdate(self) {
    if (gameOver || gameWon) {
        if (restartCooldown > 0) { restartCooldown--; return; }
        if (Bsengine.isKeyDown("Space")) resetGame();
        return;
    }

    const paddle = Bsengine.getTransform("Paddle");
    if (!paddle) return;
    let px = paddle.x;
    if (Bsengine.isKeyPressed("A") || Bsengine.isKeyPressed("Left"))  px -= PADDLE_SPEED;
    if (Bsengine.isKeyPressed("D") || Bsengine.isKeyPressed("Right")) px += PADDLE_SPEED;
    px = Math.max(-6.0, Math.min(6.0, px));
    Bsengine.setTransform("Paddle", px, paddle.y, paddle.z);

    const ball = Bsengine.getTransform(self);
    if (!ball) return;
    let bx = ball.x + ballDx;
    let by = ball.y + ballDy;

    if (bx < -BOUNDS_X) { bx = -BOUNDS_X; ballDx = -ballDx; }
    if (bx >  BOUNDS_X) { bx =  BOUNDS_X; ballDx = -ballDx; }
    if (by >  TOP_Y)    { by =  TOP_Y;     ballDy = -ballDy; }

    if (ballDy < 0 && by <= PADDLE_Y + 0.5 && by >= PADDLE_Y - 0.5) {
        if (Math.abs(bx - px) < PADDLE_HALF_W) {
            by = PADDLE_Y + 0.5;
            ballDy = Math.abs(ballDy);
            const offset = (bx - px) / PADDLE_HALF_W;
            ballDx = offset * 0.1;
        }
    }

    if (by < BOTTOM_Y) {
        gameOver = true;
        restartCooldown = 120;
        Bsengine.log("GAME OVER! Press Space to retry.");
        Bsengine.setTransform(self, bx, by, ball.z);
        return;
    }

    for (let i = 0; i < BRICK_NAMES.length; i++) {
        const name = BRICK_NAMES[i];
        const bt = Bsengine.getTransform(name);
        if (!bt || bt.x > OFF_SCREEN - 1) continue;

        if (Math.abs(bx - bt.x) < BRICK_HALF_W + 0.3 &&
            Math.abs(by - bt.y) < BRICK_HALF_H + 0.3) {
            Bsengine.setTransform(name, OFF_SCREEN, OFF_SCREEN, OFF_SCREEN);
            ballDy = -ballDy;
            bricksRemaining--;
            Bsengine.log("Brick destroyed! Remaining: " + bricksRemaining);
            if (bricksRemaining <= 0) {
                gameWon = true;
                Bsengine.log("YOU WIN! All bricks cleared! Press Space to play again.");
                Bsengine.setTransform(self, bx, by, ball.z);
                return;
            }
            break;
        }
    }

    Bsengine.setTransform(self, bx, by, ball.z);
}