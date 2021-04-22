#include "Engine.h"

int32 GuardedMain(StringView cmdLine)
{
	Engine engine;

	int32 error = engine.Init();
	if (error) return error;

	error = engine.Run();
	engine.Release();
	return error;
}
