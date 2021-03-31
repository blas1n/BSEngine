#pragma once

#include <cstdint>

class Game final
{
public:
	Game() noexcept = default;

	Game(const Game&) = delete;
	Game(Game&&) noexcept = default;

	Game& operator=(const Game&) = delete;
	Game& operator=(Game&&) noexcept = default;

	~Game() = default;

	[[nodiscard]] int32_t Init() noexcept;
	[[nodiscard]] int32_t Run() noexcept;
	void Release() noexcept;

	inline static void Exit() noexcept { isRun = false; }

private:
	inline static bool isRun = true;

	class WindowManager* windowManager = nullptr;
	class RenderManager* renderManager = nullptr;
	class InputManager* inputManager = nullptr;
	class SceneManager* sceneManager = nullptr;
	class ResourceManager* resourceManager = nullptr;
	class ComponentManager* componentManager = nullptr;

	uint32_t ticksCount = 0u;
};
