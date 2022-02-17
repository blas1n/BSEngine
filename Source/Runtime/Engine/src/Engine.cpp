#include "Engine.h"
#include "Accessor.h"
#include "SceneManager.h"
#include "InputManager.h"
#include "ThreadManager.h"
#include "WindowManager.h"

namespace
{
	constexpr static int32 Success = 0;
	constexpr static int32 FailCreate = 1;
	constexpr static int32 FailInit = 2;

	template <class T, class... Args>
	int32 CreateManager(T*& manager, Args&&... args) noexcept
	{
		manager = new T{ std::forward<Args>(args)... };
		if (!manager) return FailCreate;

		Accessor<T>::SetManager(manager);
		return manager->Init() ? Success : FailInit;
	}

	template <class T>
	void RemoveManager(T*& manager) noexcept
	{
		Accessor<T>::SetManager(nullptr);
		manager->Release();
		delete manager;
		manager = nullptr;
	}
}

int32 Engine::Init() noexcept
{
	int32 error = CreateManager(window, Name{ STR(STRINGIFY(GAME_NAME)) });
	if (error) return error;

	error = CreateManager(thread);
	if (error) return error;

	error = CreateManager(plugin);
	if (error) return error;

	error = CreateManager(input);
	if (error) return error;

	error = CreateManager(scene);
	if (error) return error;

	error = CreateManager(render);
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

		if (!input->Update(deltaTime))
			isEnd = true;

		if (!scene->Update(deltaTime))
			isEnd = true;

		if (!render->Update(deltaTime))
			isEnd = true;
	}

	return 0;
}

void Engine::Release() noexcept
{
	RemoveManager(render);
	RemoveManager(scene);
	RemoveManager(input);
	RemoveManager(plugin);
	RemoveManager(thread);
	RemoveManager(window);
}
