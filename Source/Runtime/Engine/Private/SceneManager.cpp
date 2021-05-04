#include "SceneManager.h"

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

bool SceneManager::Load(Name name) noexcept
{
	isLoadScene = true;
	
	const bool isSuccess = scenes[!isFrontScene].Load(name);

	isLoadScene = false;
	isSwapScene = true;

	return isSuccess;
}
