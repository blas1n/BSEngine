#pragma once

#include <cstdint>

namespace ArenaBoss
{
	class Game final
	{
	public:
		Game();

		Game(const Game&) = delete;
		Game(Game&&) noexcept = default;

		Game& operator=(const Game&) = delete;
		Game& operator=(Game&&) noexcept = default;

		~Game();

		int Run();

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
}