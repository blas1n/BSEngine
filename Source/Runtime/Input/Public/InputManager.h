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

	bool IsPressed(uint8 key) const noexcept { return keyStates[isFrontState][key]; }
	bool IsReleased(uint8 key) const noexcept { return !keyStates[isFrontState][key]; }
	bool IsPress(uint8 key) const noexcept { return keyStates[isFrontState][key] && !keyStates[!isFrontState][key]; }
	bool IsRelease(uint8 key) const noexcept { return !keyStates[isFrontState][key] && keyStates[!isFrontState][key]; }

	const IntVector2& GetMousePos() const noexcept { return mousePos; }

private:
	bool ReadKeyboard();
	bool ReadMouse();

private:
	struct InputImpl* impl;

	uint8 keyStates[2][256];
	IntVector2 mousePos;

	uint8 isFrontState : 1;
};
