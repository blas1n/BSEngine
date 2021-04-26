#include "Engine.h"
#include "Manager.h"

int32 Engine::Init() noexcept
{
	int32 error = LoadManager();
	if (error) return error;

	for (const auto manager : managers)
		if ((error = manager->Init()))
			return error;

	return 0;
}

int32 Engine::Run() noexcept
{
	while (!isEnd)
	{
		// Todo: v-sync

		for (const auto manager : managers)
			manager->Update();

		++tickCount;
	}

	return errorCode;
}

void Engine::Release() noexcept
{
	for (auto iter = managers.rbegin(); iter != managers.rend(); ++iter)
		(*iter)->Release();

	UnloadManager();
}

void Engine::Exit(int32 error) noexcept
{
	errorCode = error;
	isEnd = true;
}

int32 Engine::LoadManager() noexcept
{
	return 0;
}

void Engine::UnloadManager() noexcept
{
	while (!managers.empty())
	{
		if (const auto manager = managers.back())
			delete manager;

		managers.pop_back();
	}
}
