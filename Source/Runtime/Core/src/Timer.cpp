#include "Timer.h"

float Timer::Tick()
{
	auto now = isPaused ? pausedTime : std::chrono::high_resolution_clock::now();
	deltaTime = (now - startTime - pauseDeltaTime) * timeScale;

	pauseDeltaTime = std::chrono::duration<float>(0.0f);
	startTime = std::move(now);
	return GetDeltaTime();
}

void Timer::Reset()
{
	startTime = pausedTime = std::chrono::high_resolution_clock::now();
	deltaTime = pauseDeltaTime = std::chrono::duration<float>(0.0f);
	timeScale = 1.0f;
}

void Timer::Pause()
{
	if (isPaused) return;

	pausedTime = std::chrono::high_resolution_clock::now();
	isPaused = true;
}

void Timer::Unpause()
{
	if (!isPaused) return;

	pauseDeltaTime = std::chrono::high_resolution_clock::now() - pausedTime;
	isPaused = false;
}
