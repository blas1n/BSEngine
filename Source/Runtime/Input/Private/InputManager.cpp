#include "InputManager.h"
#include <dinput.h>
#include "WindowManager.h"

#pragma comment(lib, "dinput8.lib")
#pragma comment(lib, "dxguid.lib")

struct InputImpl final
{
	IDirectInput8* directInput;
	IDirectInputDevice8* keyboard;
	IDirectInputDevice8* mouse;
	DIMOUSESTATE mouseState;
};

bool InputManager::Init() noexcept
{
	impl = new InputImpl;

	const auto hInst = reinterpret_cast<HINSTANCE>
		(Accessor<WindowManager>::GetManager()->GetInstanceHandle());

	HRESULT result = DirectInput8Create(hInst, DIRECTINPUT_VERSION,
		IID_IDirectInput8, reinterpret_cast<void**>(&impl->directInput), nullptr);

	if (FAILED(result)) return false;

	result = impl->directInput->CreateDevice(GUID_SysKeyboard, &impl->keyboard, nullptr);
	if (FAILED(result)) return false;
	
	result = impl->keyboard->SetDataFormat(&c_dfDIKeyboard);
	if (FAILED(result)) return false;

	const auto hWnd = reinterpret_cast<HWND>
		(Accessor<WindowManager>::GetManager()->GetInstanceHandle());

	result = impl->keyboard->SetCooperativeLevel(hWnd, DISCL_FOREGROUND | DISCL_EXCLUSIVE);
	if (FAILED(result)) return false;

	result = impl->keyboard->Acquire();
	if (FAILED(result)) return false;

	result = impl->directInput->CreateDevice(GUID_SysMouse, &impl->mouse, nullptr);
	if (FAILED(result)) return false;

	result = impl->mouse->SetDataFormat(&c_dfDIMouse);
	if (FAILED(result)) return false;

	result = impl->mouse->SetCooperativeLevel(hWnd, DISCL_FOREGROUND | DISCL_NONEXCLUSIVE);
	if (FAILED(result)) return false;

	result = impl->mouse->Acquire();
	if (FAILED(result)) return false;
	
	return true;
}

bool InputManager::Update(float deltaTime) noexcept
{
	bool result = ReadKeyboard();
	if(!result) return false;
	
	result = ReadMouse();
	if(!result) return false;
	
	return true;
}

void InputManager::Release() noexcept
{
	if (impl->mouse)
	{
		impl->mouse->Unacquire();
		impl->mouse->Release();
		impl->mouse = nullptr;
	}

	if (impl->keyboard)
	{
		impl->keyboard->Unacquire();
		impl->keyboard->Release();
		impl->keyboard = nullptr;
	}

	if (impl->directInput)
	{
		impl->directInput->Release();
		impl->directInput = nullptr;
	}

	delete impl;
}

bool InputManager::ReadKeyboard()
{
	const HRESULT result = impl->keyboard->GetDeviceState(
		sizeof(keyState), reinterpret_cast<LPVOID>(&keyState));
	
	if (FAILED(result))
	{
		if ((result == DIERR_INPUTLOST) || (result == DIERR_NOTACQUIRED))
			impl->keyboard->Acquire();
		else
			return false;
	}

	return true;
}

bool InputManager::ReadMouse()
{
	const HRESULT result = impl->mouse->GetDeviceState(
		sizeof(DIMOUSESTATE), reinterpret_cast<LPVOID>(&impl->mouseState));
	
	if (FAILED(result))
	{
		if ((result == DIERR_INPUTLOST) || (result == DIERR_NOTACQUIRED))
			impl->mouse->Acquire();
		else
			return false;
	}

	mousePos = IntVector2{ impl->mouseState.lX, impl->mouseState.lY };

	const IntVector2 windowSize = Accessor<WindowManager>::GetManager()->GetSize();
	mousePos = Clamp(mousePos, IntVector2::Zero, windowSize);

	return true;
}
