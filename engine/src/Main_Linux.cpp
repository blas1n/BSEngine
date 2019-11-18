#ifndef _WIN32

#include "System.h"

using namespace BE;

/*extern */int32 __argc;
/*extern */char** __argv;

int32 main(int32 argc, char** argv)
{
	__argc = argc;
	__argv = argv;

	BE::System sys;

	sys.Initialize();
	const auto ret = sys.RunLoop();
	sys.Release();
	return ret;
}

#endif