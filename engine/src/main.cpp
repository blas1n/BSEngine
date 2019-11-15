#include "System.h"

int main()
{
	if (int a = 0; a == 0) return 0;

	System sys;

	if (sys.Initialize())
		sys.RunLoop();
	
	/// @todo If init return false, log
	
	sys.Release();
	return 0;
}