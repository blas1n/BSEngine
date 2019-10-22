#include "System.h"
#include <SDL/SDL.h>

bool System::Initialize()
{
	if (!SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO | SDL_INIT_GAMECONTROLLER))
	{
		SDL_Log("Unable to initialize SDL: %s", SDL_GetError());
		return false;
	}
}

void System::RunLoop()
{

}

void System::Release()
{

}