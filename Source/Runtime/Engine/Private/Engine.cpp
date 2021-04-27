#include "Engine.h"
#include "WindowManager.h"

namespace
{
	template <class T, class... Args>
	int32 CreateManager(T*& manager, Args&&... args) noexcept
	{
		manager = new T{};
		if (!manager) return 1;

		return manager->Init(std::forward<Args>(args)...) ? 0 : 2;
	}

	void RemoveManager(Manager* manager) noexcept
	{
		manager->Release();
		delete manager;
	}
}

int32 Engine::Init() noexcept
{
	int32 error = CreateManager<WindowManager>(window);
	if (error) return error;

	timer.Reset();
	return 0;
}

int32 Engine::Run() noexcept
{
	bool isEnd = false;

	while (!isEnd)
	{
		const float deltaTime = timer.Tick();

		if (!window->Update(deltaTime))
			isEnd = true;
	}

	return 0;
}

void Engine::Release() noexcept
{
	RemoveManager(window);
}
