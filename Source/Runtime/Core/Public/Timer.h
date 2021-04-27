#pragma once

#include <chrono>
#include "BSBase/Type.h"

class CORE_API Timer final
{
public:
	Timer() { Reset(); }

	float Tick();
	void Reset();

	void Pause();
	void Unpause();

	[[nodiscard]] float GetTimeScale() const noexcept { return timeScale; };
	void SetTimeScale(float scale = 1.0f) noexcept { timeScale = scale; };

	[[nodiscard]] float GetDeltaTime() const noexcept { return deltaTime.count(); }

private:
	std::chrono::high_resolution_clock::time_point startTime;
	std::chrono::high_resolution_clock::time_point pausedTime;

	std::chrono::duration<float> deltaTime;
	std::chrono::duration<float> pauseDeltaTime;

	float timeScale;
	BSBase::uint8 isPaused : 1;
};
