#include "System/Public/System.h"

int main()
{
	System sys;

	if (sys.Initialize())
		sys.RunLoop();
	
	/// @todo If init return false, log
	
	sys.Release();
	return 0;
}