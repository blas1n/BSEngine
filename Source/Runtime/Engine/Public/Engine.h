#pragma once

#include "Core.h"
#include <vector>

class ENGINE_API Engine final
{
public:
	Engine() noexcept
		: timer(), window(nullptr) {}

	Engine(const Engine&) = delete;
	Engine(Engine&&) noexcept = delete;

	Engine& operator=(const Engine&) = delete;
	Engine& operator=(Engine&&) noexcept = delete;

	~Engine() = default;

	[[nodiscard]] int32 Init() noexcept;
	[[nodiscard]] int32 Run() noexcept;
	void Release() noexcept;

private:
	Timer timer;

	class WindowManager* window;
};
