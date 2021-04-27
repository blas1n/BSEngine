#pragma once

class FRAMEWORK_API Manager
{
public:
	Manager() = default;

	Manager(const Manager&) = delete;
	Manager(Manager&&) noexcept = delete;

	Manager& operator=(const Manager&) = delete;
	Manager& operator=(Manager&&) noexcept = delete;

	virtual ~Manager() = default;

	[[nodiscard]] virtual bool Init() noexcept { return true; }
	[[nodiscard]] virtual bool Update(float deltaTime) noexcept { return true; }
	virtual void Release() noexcept {}
};
