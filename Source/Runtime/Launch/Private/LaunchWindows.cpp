#define WIN32_LEAN_AND_MEAN

#include <windows.h>
#include "Core.h"

extern int32 GuardedMain(StringView cmdLine);

String ProcessCommandLine()
{
	return CastCharSet<Char>(std::wstring_view{ GetCommandLineW() });
}

int32 WINAPI WinMain(_In_ HINSTANCE hInInstance, _In_opt_ HINSTANCE, _In_ char*, _In_ int32)
{
	const auto cmdLine = ProcessCommandLine();
	return GuardedMain(StringView{ cmdLine.c_str() });
}