#ifndef _WIN32

#include "System.h"

using namespace BE;

/*extern */Int32 __argc;
/*extern */char** __argv;

Int32 main(Int32 argc, char** argv)
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