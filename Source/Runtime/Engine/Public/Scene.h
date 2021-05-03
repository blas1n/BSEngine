#pragma once

#include "Entity.h"
#include <vector>

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

	Entity* GetEntity(uint32 index) noexcept
	{
		return (index < entities.size()) ? &*(entities.begin() + index) : nullptr;
	}

	const Entity* GetEntity(uint32 index) const noexcept
	{
		return (index < entities.size()) ? &*(entities.begin() + index) : nullptr;
	}

private:
	Name name;

	std::vector<Entity> entities;
};
