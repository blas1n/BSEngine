#include "Entity.h"

Entity::~Entity()
{
	for (const auto comp : components)
		delete comp.second;

	components.clear();
}

Json Entity::Serialize() const
{
	Json json = Json::object();
	json["id"] = id;
	json["name"] = CastCharSet<char>(StringView{ name.c_str() });
	Json comps = json["components"] = Json::array();

	for (const auto& comp : components)
		comps.push_back(comp.second->Serialize().get<std::string>());

	return json;
}

void Entity::Deserialize(const Json& json)
{
	const auto str = json["name"].get<std::string>();
	name = CastCharSet<Char>(std::string_view{ str.c_str() });

	for (const auto& comp : json["components"])
	{
		const auto strName = comp["name"].get<std::string>();
		const Name name = CastCharSet<Char>(std::string_view{ strName.c_str() });
		components.insert(std::make_pair(name, CreateComponent(name, this)))->second->Deserialize(comp);
	}
}

Name Entity::GetComponentName(StringView functionName)
{
	const size_t begin = functionName.find(STR('<'));
	const size_t end = functionName.find(STR('>'));
	return functionName.substr(begin + 1, end - begin - 1);
}
