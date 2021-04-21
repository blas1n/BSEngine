#define WIN32_LEAN_AND_MEAN
#define NOMINMAX

#include "Core.h"

// @note: Error if you include windows.h first
#include <windows.h>

extern int32 GuardedMain(StringView cmdLine);

String ProcessCommandLine()
{
	const auto cmd = std::wstring{ GetCommandLineW() };
	return CastCharSet<Char>(std::wstring_view{ cmd.c_str() });
}

int32 WINAPI WinMain(_In_ HINSTANCE hInInstance, _In_opt_ HINSTANCE, _In_ char*, _In_ int32)
{
	const auto cmdLine = ProcessCommandLine();
	return GuardedMain(StringView{ cmdLine.c_str() });
}
