#include "InputManager.h"
#include <dinput.h>

struct Impl final
{
	IDirectInput8* directInput;
	IDirectInputDevice8* keyboard;
	IDirectInputDevice8* mouse;
	DIMOUSESTATE mouseState;
};

bool InputManager::Init() noexcept
{
	impl = new Impl;

	HRESULT result = DirectInput8Create(hinstance, DIRECTINPUT_VERSION, IID_IDirectInput8, (void**)&impl->directInput, NULL);
	if (FAILED(result)) { return false; }
}

bool InputManager::Update(float deltaTime) noexcept
{

}

void InputManager::Release() noexcept
{
	delete impl;
}
