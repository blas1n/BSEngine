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

	~Scene() = default;

	bool Load(Name inName) noexcept;
	bool Save(Name inName) const noexcept;
	bool Save() const noexcept { return Save(name); }

	Entity* GetEntity(uint32 id) noexcept
	{
		return const_cast<Entity*>(static_cast<const Scene*>(this)->GetEntity(id));
	}

	const Entity* GetEntity(uint32 id) const noexcept;
	
	Name GetName() const noexcept { return name; }

private:
	std::vector<Entity> entities;
	Name name;
};
