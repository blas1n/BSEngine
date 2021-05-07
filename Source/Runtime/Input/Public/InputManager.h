#pragma once

#include "Core.h"
#include "Accessor.h"
#include "Manager.h"
#include "InputCode.h"

class INPUT_API InputManager : public Manager, private Accessor<class WindowManager>
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	const IntVector2& GetMousePos() const noexcept { return mousePos; }

private:
	bool ReadKeyboard();
	bool ReadMouse();

private:
	struct InputImpl* impl;

	uint8 keyState[256];
	uint8 mouseState[8];
	IntVector2 mousePos;
};
