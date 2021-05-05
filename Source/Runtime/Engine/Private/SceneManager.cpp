#include "SceneManager.h"
#include "ThreadManager.h"

bool SceneManager::Update(float deltaTime) noexcept
{
	if (isSwapScene)
	{
		isFrontScene = !isFrontScene;
		isSwapScene = false;
	}

	onUpdates[isFrontScene](deltaTime);
	return !isEnd;
}

std::future<bool> SceneManager::Load(Name name) noexcept
{
	Delegate<bool(void)> load{ [this, name] { return LoadImpl(name); } };
	return Accessor<ThreadManager>::GetManager()->AddTask(std::move(load));
}

void SceneManager::RegisterUpdate(const Delegate<void(float)>& callback)
{
	std::shared_lock lock{ mutex };
	onUpdates[isFrontScene != isLoadScene] += callback;
}

void SceneManager::RegisterUpdate(Delegate<void(float)>&& callback)
{
	std::shared_lock lock{ mutex };
	onUpdates[isFrontScene != isLoadScene] += std::move(callback);
}

bool SceneManager::LoadImpl(Name name) noexcept
{
	mutex.lock();
	isLoadScene = true;
	mutex.unlock();

	const bool isSuccess = scenes[!isFrontScene].Load(name);

	mutex.lock();
	isLoadScene = false;
	isSwapScene = true;
	mutex.unlock();

	return isSuccess;
}
