#pragma once

#include <cstdint>
#include <SDL2/SDL.h>
#include <string>
#include "Vector2.h"

namespace ArenaBoss
{
	enum class ScreenMode : uint8_t
	{
		Window,
		FullScreen,
		Borderless
	};

	class WindowManager final
	{
	public:
		WindowManager(const char* inTitle, uint32_t inWidth,
			uint32_t inHeight, ScreenMode inScreenMode);

		WindowManager(const WindowManager&) = delete;
		WindowManager(WindowManager&&) = delete;

		WindowManager& operator=(const WindowManager&) = delete;
		WindowManager& operator=(WindowManager&&) = delete;

		~WindowManager();

		inline void SwapBuffer() noexcept { SDL_GL_SwapWindow(window); }

		inline SDL_Window* GetWindow() noexcept { return window; }
		inline const SDL_Window* GetWindow() const noexcept { return window; }

		inline const std::string& GetTitle() const noexcept { return title; }
		void SetTitle(const std::string& inTitle) noexcept;

		inline uint32_t GetWidth() const noexcept { return GetSize().x; }
		inline uint32_t GetHeight() const noexcept { return GetSize().y; }

		Math::UintVector2 GetSize() const noexcept;
		void SetSize(uint32_t width, uint32_t height) noexcept;

		inline ScreenMode GetScreenMode() const noexcept { return screenMode; }
		void SetScreenMode(ScreenMode inScreenMode) noexcept;

	private:
		Uint32 GetSDLScreenMode() const noexcept;

	private:
		SDL_Window* window;
		std::string title;
		ScreenMode screenMode;
	};
}