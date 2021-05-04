#include "Scene.h"
#include <filesystem>
#include "Core.h"

bool Scene::Init(Name inName) noexcept
{
	Release();
	name = inName;
	return true;
}

void Scene::Release() noexcept
{
	entities.clear();
}

bool Scene::Load() noexcept
{
	Release();

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

bool Scene::Save() const noexcept
{
	std::filesystem::path path{ STR("Assets") };
	path /= name.ToString();
	
	Json json = Json::object();

	json["name"] = CastCharSet<char>(StringView{ name.ToString().c_str() });
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
