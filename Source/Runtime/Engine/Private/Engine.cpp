#include "Engine.h"
#include "Accessor.h"
#include "SceneManager.h"
#include "InputManager.h"
#include "ThreadManager.h"
#include "WindowManager.h"

namespace
{
	template <class T, class... Args>
	int32 CreateManager(T*& manager, Args&&... args) noexcept
	{
		manager = new T{};
		if (!manager) return 1;

		Accessor<T>::SetManager(manager);
		return manager->Init(std::forward<Args>(args)...) ? 0 : 2;
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
	int32 error = CreateManager(window);
	if (error) return error;

	error = CreateManager(thread);
	if (error) return error;

	error = CreateManager(input);
	if (error) return error;

	error = CreateManager(scene);
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
	}

	return 0;
}

void Engine::Release() noexcept
{
	RemoveManager(scene);
	RemoveManager(input);
	RemoveManager(thread);
	RemoveManager(window);
}
