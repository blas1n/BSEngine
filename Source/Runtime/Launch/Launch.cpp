#define WIN32_LEAN_AND_MEAN

#include <windows.h>
#include "Game.h"

int WINAPI WinMain(HINSTANCE hInstance, HINSTANCE hPrevInstance, PSTR pScmdline, int iCmdshow)
{
	Game game;
	auto ret = game.Init();

	if (!ret)
		ret = game.Run();

	game.Release();
	return ret;
}