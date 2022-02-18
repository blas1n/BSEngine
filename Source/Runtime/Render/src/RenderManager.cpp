#include "RenderManager.h"

bool RenderManager::Init() noexcept
{
	if (Ensure(rhi, "RHI must set before initialize"))
		return false;

	return true;
}

bool RenderManager::Update(float deltaTime) noexcept
{
	return true;
}

void RenderManager::Release() noexcept
{

}