#pragma once

#include "BSMath.h"
#include "Manager.h"

class WINDOW_API WindowManager final : public Manager
{
public:
	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	void* GetInstanceHandle() const noexcept { return hInstance; }
	void* GetWindowHandle() const noexcept { return hWnd; }

	const IntVector2& GetSize() const noexcept { return size; }

private:
	struct HINSTANCE__* hInstance = nullptr;
	struct HWND__* hWnd = nullptr;

	IntVector2 size;
};
