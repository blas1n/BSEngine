#include <SDL/SDL.h>
#include "System/Root/System.h"

int main() {
	System sys;

	if (sys.Initialize())
		sys.RunLoop();
	else
		SDL_LogError(SDL_LOG_CATEGORY_SYSTEM, "Failed to initialize engine.");
	
	sys.Release();
	return 0;
}