#include "Engine.h"

int32 Engine::Init() noexcept
{
	return 0;
}

int32 Engine::Run() noexcept
{
	while (!isEnd)
	{
		
	}

	return errorCode;
}

void Engine::Release() noexcept
{
	
}

void Engine::Exit(int32 error) noexcept
{
	errorCode = error;
	isEnd = true;
}
