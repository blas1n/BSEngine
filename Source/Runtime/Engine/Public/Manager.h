#pragma once

#include "Core.h"

class ENGINE_API Manager
{
public:
	Manager(class Engine* inEngine)
		: engine(inEngine) {}

	Manager(const Manager&) = delete;
	Manager(Manager&&) noexcept = delete;

	Manager& operator=(const Manager&) = delete;
	Manager& operator=(Manager&&) noexcept = delete;

	virtual ~Manager() {}

	[[nodiscard]] virtual int32 Init() noexcept { return 0; }
	virtual void Update(float deltaTime) noexcept {}
	virtual void Release() noexcept {}

protected:
	Engine* GetEngine() noexcept { return engine; }
	const Engine* GetEngine() const noexcept { return engine; }

private:
	Engine* engine;
};
