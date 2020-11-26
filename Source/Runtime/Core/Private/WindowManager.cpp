#include "WindowManager.h"
#include <SDL2/SDL.h>
#include <exception>

namespace ArenaBoss
{
	WindowManager::WindowManager(const char* inTitle,
		uint32_t width, uint32_t height, ScreenMode inScreenMode)
		: window(nullptr), title(inTitle), screenMode(inScreenMode)
	{
		if (SDL_Init(SDL_INIT_VIDEO | SDL_INIT_AUDIO) != 0)
			throw std::exception{ "Failed to initialze GLFW" };

		SDL_GL_SetAttribute(SDL_GL_CONTEXT_PROFILE_MASK, SDL_GL_CONTEXT_PROFILE_CORE);

		SDL_GL_SetAttribute(SDL_GL_CONTEXT_MAJOR_VERSION, 3);
		SDL_GL_SetAttribute(SDL_GL_CONTEXT_MINOR_VERSION, 3);

		SDL_GL_SetAttribute(SDL_GL_RED_SIZE, 8);
		SDL_GL_SetAttribute(SDL_GL_GREEN_SIZE, 8);
		SDL_GL_SetAttribute(SDL_GL_BLUE_SIZE, 8);
		SDL_GL_SetAttribute(SDL_GL_ALPHA_SIZE, 8);

		SDL_GL_SetAttribute(SDL_GL_DOUBLEBUFFER, 1);
		SDL_GL_SetAttribute(SDL_GL_ACCELERATED_VISUAL, 1);

		window = SDL_CreateWindow("ArenaBoss",
			SDL_WINDOWPOS_UNDEFINED,
			SDL_WINDOWPOS_UNDEFINED,
			static_cast<int>(width),
			static_cast<int>(height),
			SDL_WINDOW_OPENGL | GetSDLScreenMode());

		if (!window)
			throw std::exception{ "Failed to create window" };
	}

	WindowManager::~WindowManager()
	{
		SDL_DestroyWindow(window);
	}

	void WindowManager::SetTitle(const std::string& inTitle) noexcept
	{
		title = inTitle;
		SDL_SetWindowTitle(window, inTitle.c_str());
	}

	Math::UintVector2 WindowManager::GetSize() const noexcept
	{
		int w, h;
		SDL_GetWindowSize(window, &w, &h);
		return Math::UintVector2{ static_cast<uint32_t>(w), static_cast<uint32_t>(h) };
	}

	void WindowManager::SetSize(uint32_t width, uint32_t height) noexcept
	{
		SDL_SetWindowSize(window, static_cast<int>(width), static_cast<int>(height));
	}

	void WindowManager::SetScreenMode(ScreenMode inScreenMode) noexcept
	{
		screenMode = inScreenMode;
		SDL_SetWindowFullscreen(window, GetSDLScreenMode());
	}

	Uint32 WindowManager::GetSDLScreenMode() const noexcept
	{
		constexpr static Uint32 Mode[3]{ 0, SDL_WINDOW_FULLSCREEN, SDL_WINDOW_FULLSCREEN_DESKTOP };
		return Mode[static_cast<uint8_t>(screenMode);
	}
}