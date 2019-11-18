#ifdef _WIN32

#pragma comment(linker, "/entry:WinMainCRTStartup /subsystem:console")

#include "System.h"

#ifndef _WINDOWS_
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#endif

using namespace BE;

int32 APIENTRY WinMain(_In_ HINSTANCE hInstance,
	_In_opt_ HINSTANCE hPrevInstance, _In_ LPSTR cmdLine, _In_ int32 nCmdShow)
{
	BE::System sys;

	sys.Initialize();
	const auto ret = sys.RunLoop();
	sys.Release();
	return ret;
}

#endif