#pragma once

#include "Manager.h"
#include "BSMath.h"
#include "Name.h"

class WINDOW_API WindowManager final : public Manager
{
public:
	WindowManager(Name inGameName) : gameName(std::move(inGameName)) {}

	[[nodiscard]] bool Init() noexcept override;
	[[nodiscard]] bool Update(float deltaTime) noexcept override;
	void Release() noexcept override;

	const struct Handle& GetHandle() const noexcept { return *handle; }
	const IntVector2& GetSize() const noexcept { return size; }

private:
	Handle* handle;

	Name gameName;
	IntVector2 size;
};
