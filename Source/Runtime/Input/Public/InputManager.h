#pragma once

#include "Core.h"
#include "Accessor.h"
#include "Manager.h"

class INPUT_API InputManager : public Manager, private Accessor<class WindowManager>
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	bool IsPressed(uint8 key) const noexcept { return keyState[key]; }
	bool IsReleased(uint8 key) const noexcept { return !keyState[key]; }

	const IntVector2& GetMousePos() const noexcept { return mousePos; }

private:
	bool ReadKeyboard();
	bool ReadMouse();

private:
	struct InputImpl* impl;

	uint8 keyState[256];
	IntVector2 mousePos;
};
