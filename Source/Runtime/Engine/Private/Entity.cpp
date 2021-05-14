#include "Entity.h"

Entity::~Entity()
{
	for (const auto& compList : components)
		for (const auto comp : compList.second)
			delete comp;

	components.clear();
}

Json Entity::Serialize() const
{
	Json json = Json::object();
	json["name"] = name;
	Json& comps = json["components"] = Json::array();

	for (const auto& compList : components)
		for (const auto* comp : compList.second)
		comps.push_back(comp->Serialize());

	return json;
}

void Entity::Deserialize(const Json& json)
{
	name = json["name"].get<String>();

	for (const auto& comp : json["components"])
	{
		const auto strName = comp["name"].get<std::string>();
		const Name name = CastCharSet<Char>(std::string_view{ strName.c_str() });
		components[name].emplace_back(CreateComponent(name, this))->Deserialize(comp);
	}
}

void Entity::SetName(StringView inName) noexcept
{
	name = std::move(inName);
	//Todo: apply name in scene
}

Name Entity::GetComponentName(StringView functionName)
{
	const size_t begin = functionName.find(STR('<'));
	const size_t end = functionName.find(STR('>'));
	return functionName.substr(begin + 1, end - begin - 1);
}
