#pragma once

#include "Name.h"

class ENGINE_API Scene final
{
public:
	Scene() = default;

	Scene(const Scene&) = delete;
	Scene(Scene&&) noexcept = default;

	Scene& operator=(const Scene&) = delete;
	Scene& operator=(Scene&&) noexcept = default;

	~Scene() { Release(); }

	bool Init(Name inName) noexcept;
	void Release() noexcept;

	bool Load() noexcept;
	bool Save() noexcept;

private:
	Name name;
};
