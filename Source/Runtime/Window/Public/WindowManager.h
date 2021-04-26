#pragma once

#include "Manager.h"

class WINDOW_API WindowManager final : public Manager
{
public:
	using Manager::Manager;

	[[nodiscard]] int32 Init() noexcept override;
	void Update(float deltaTime) noexcept override;
	void Release() noexcept override;

private:
	struct HINSTANCE__* hInstance;
	struct HWND__* hWnd;

	IntVector2 size;
};
