#include "System.h"

int main()
{
	BE::System sys;

	if (sys.Initialize())
		sys.RunLoop();
	
	sys.Release();
	return 0;
}