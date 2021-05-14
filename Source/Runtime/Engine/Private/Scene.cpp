#include "Scene.h"
#include <filesystem>
#include "Core.h"

bool Scene::Load(Name inName) noexcept
{
	name = inName;
	entities.clear();

	std::filesystem::path path{ STR("Assets") };
	path /= name.ToString();

	std::ifstream stream{ path.string() };
	Json json;
	stream >> json;

	const auto jsonStr = json["name"].get<String>();
	const Name jsonName{ jsonStr.c_str() };
	if (Ensure(jsonName == name))
		return false;

	for (const auto& entity : json["entities"])
	{
		const auto name = entity["name"].get<String>();
		entities[name].Deserialize(entity);
	}

	return true;
}

bool Scene::Save(Name inName) const noexcept
{
	std::filesystem::path path{ STR("Assets") };
	path /= inName.ToString();
	
	Json json = Json::object();
	json["name"] = inName.ToString();

	Json& entityJson = json["entities"] = Json::array();

	for (const auto& entity : entities)
		entityJson.push_back(entity.second.Serialize());

	std::ofstream stream{ path.string() };
	stream << json;
	return true;
}

Entity* Scene::AddEntity(const String& name)
{
	return &entities.insert(std::make_pair(name, Entity{})).first->second;
}

Entity* Scene::AddEntity(const String& name, Entity* prefab)
{
	return &entities.insert(std::make_pair(name, Entity{ *prefab })).first->second;
}

bool Scene::RemoveEntity(Entity* entity)
{
	for (auto iter = entities.cbegin(); iter != entities.cend(); ++iter)
	{
		if (&iter->second == entity)
		{
			entities.erase(iter);
			return true;
		}
	}

	return false;
}
