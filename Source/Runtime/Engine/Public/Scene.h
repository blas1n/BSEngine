#pragma once

#include "Entity.h"
#include <unordered_map>

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

	Entity* AddEntity(const String& name);
	Entity* AddEntity(const String& name, Entity* prefab);

	bool RemoveEntity(const String& name);
	bool RemoveEntity(Entity* entity) { return RemoveEntity(entity->GetName()); }

	[[nodiscard]] Entity* GetEntity(const String& name) noexcept { return &entities.at(name); }
	[[nodiscard]] const Entity* GetEntity(const String& name) const noexcept { return &entities.at(name); }
	
	[[nodiscard]] Name GetName() const noexcept { return name; }

private:
	Entity* AddEntityImpl(Entity& entity);
	void RemoveEntityImpl(Entity& entity);

	void OnChangedName(Entity& entity, const String& after, const String& before);

private:
	std::unordered_map<String, Entity> entities;
	Name name;
};
