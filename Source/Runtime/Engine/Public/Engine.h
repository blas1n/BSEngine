#pragma once

#include <vector>
#include "Core.h"

class ENGINE_API Engine final
{
public:
	Engine() noexcept = default;

	Engine(const Engine&) = delete;
	Engine(Engine&&) noexcept = delete;

	Engine& operator=(const Engine&) = delete;
	Engine& operator=(Engine&&) noexcept = delete;

	~Engine() = default;

	[[nodiscard]] int32 Init() noexcept;
	[[nodiscard]] int32 Run() noexcept;
	void Release() noexcept;

	void Exit(int32 error = 0) noexcept;

private:
	[[nodiscard]] int32 LoadManager() noexcept;
	void UnloadManager() noexcept;

private:
	std::vector<class Manager*> managers;

	int32 errorCode;
	uint8 isEnd : 1;
};
