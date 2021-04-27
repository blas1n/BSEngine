#pragma once

#include "BSMath.h"
#include "Manager.h"

class WINDOW_API WindowManager final : public Manager
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

private:
	struct HINSTANCE__* hInstance;
	struct HWND__* hWnd;

	IntVector2 size;
};
