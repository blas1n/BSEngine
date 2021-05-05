#pragma once

#include "Core.h"
#include "Manager.h"

class INPUT_API InputManager : public Manager
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

private:
	struct Impl* impl;

	uint8 keyState[256];
	IntVector2 mousePos;
};
