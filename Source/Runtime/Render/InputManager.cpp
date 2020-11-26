#include "InputManager.h"
#include <cassert>
#include <memory>
#include "Game.h"
#include "MathFunctions.h"
#include "Util.h"
#include "Windows.h"

namespace ArenaBoss
{
	InputManager::InputManager()
		: curKeyState(SDL_GetKeyboardState(nullptr)),
		oldKeyState(),
		curButtonState(),
		oldButtonState(),
		mousePos(),
		wheelMove(),
		isRelative(false) {}

	bool InputManager::IsCurDown(Key key) const noexcept
	{
		const auto code = static_cast<int>(key);

		if (code < NUM_MIN_MOUSE)
			return curKeyState[static_cast<SDL_Scancode>(key)];
		else
			return curButtonState & SDL_BUTTON(code - NUM_MIN_MOUSE);
	}

	bool InputManager::IsOldDown(Key key) const noexcept
	{
		const auto code = static_cast<int>(key);

		if (code < NUM_MIN_MOUSE)
			return oldKeyState[static_cast<SDL_Scancode>(key)];
		else
			return oldButtonState & SDL_BUTTON(code - NUM_MIN_MOUSE);
	}

	void InputManager::Update()
	{
		memcpy(oldKeyState, curKeyState, SDL_NUM_SCANCODES);
		oldButtonState = curButtonState;
		wheelMove = 0;

		SDL_Event event;
		while (SDL_PollEvent(&event))
		{
			switch (event.type)
			{
			case SDL_QUIT:
				Game::Exit();

			case SDL_MOUSEWHEEL:
				wheelMove = event.wheel.y;
				break;
			}
		}
		
		int x = 0, y = 0;

		if (isRelative)
			curButtonState = SDL_GetRelativeMouseState(&x, &y);
		else
			curButtonState = SDL_GetMouseState(&x, &y);

		mousePos.Set(x, y);
	}

	void InputManager::SetRelativeMouseMode(bool value)
	{
		SDL_SetRelativeMouseMode(value ? SDL_TRUE : SDL_FALSE);
		SDL_GetRelativeMouseState(nullptr, nullptr);
		isRelative = value;
	}
}