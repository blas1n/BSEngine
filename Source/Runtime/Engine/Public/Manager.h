#pragma once

#include <cstdint>

class Manager
{
	Manager(const Manager&) = delete;
	Manager(Manager&&) = delete;
	
	Manager& operator=(const Manager&) = delete;
	Manager& operator=(Manager&&) = delete;

	virtual ~Manager() = default;

	[[nodiscard]] virtual int32_t Init() noexcept;
	[[nodiscard]] virtual int32_t Update() noexcept {}
	virtual void Release() noexcept;
};
