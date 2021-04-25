#include "Engine.h"
#include <exception>
#include "Accessor.h"
#include "ComponentManager.h"
#include "ConfigFile.h"
#include "InputManager.h"
#include "Log.h"
#include "MathFunctions.h"
#include "RenderManager.h"
#include "ResourceManager.h"
#include "SceneManager.h"
#include "WindowManager.h"

int32 Engine::Init()
{
	ConfigFile config{ STR("Config.ini") };

	const char* name = config("Common", "Name")->c_str();
	uint32_t width = std::stoi(*config("Common", "Width"));
	uint32_t height = std::stoi(*config("Common", "Height"));
	std::string screenStr = *config("Common", "ScreenMode");

	ScreenMode screenMode = ScreenMode::Window;
	if (screenStr == "FullScreen")
		screenMode = ScreenMode::FullScreen;
	else if (screenStr == "Borderless")
		screenMode = ScreenMode::Borderless;

	
}

int32 Engine::Run()
{
	while (!isEnd)
	{
		// GLFW version v-sync.
		// Calc delta time.

		
	}

	return 0;
}

void Engine::Release()
{
	
}
