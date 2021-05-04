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

	const auto jsonStr = json["name"].get<std::string>();
	const Name jsonName = CastCharSet<Char>(std::string_view{ jsonStr.c_str() });
	if (Ensure(jsonName == name))
		return false;

	for (const auto& entity : json["entities"])
		entities.emplace_back(this, entity["id"]).Deserialize(entity);

	return true;
}

bool Scene::Save(Name inName) const noexcept
{
	std::filesystem::path path{ STR("Assets") };
	path /= inName.ToString();
	
	Json json = Json::object();

	json["name"] = CastCharSet<char>(StringView{ inName.ToString().c_str() });
	Json entityJson = json["entities"] = Json::array();

	for (const auto& entity : entities)
		entityJson.push_back(entity.Serialize());

	std::ofstream stream{ path.string() };
	stream << json;
	return true;
}

const Entity* Scene::GetEntity(uint32 id) const noexcept
{
	const size_t size = entities.size();
	int32 curIdx = static_cast<int32>(Min(id, size - 1));
	
	while (curIdx >= 0 && curIdx < size)
	{
		const auto& entity = entities[curIdx];
		if (id == entity.GetId())
			return &entity;
		
		if (id > entity.GetId())
			++curIdx;
		else
			--curIdx;
	}
	
	return nullptr;
}
