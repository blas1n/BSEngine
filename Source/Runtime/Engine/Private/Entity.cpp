#include "Entity.h"

Component* Entity::AddComponent(Name type)
{
	return nullptr;
}

Component* Entity::GetComponent(Name type)
{
	return nullptr;
}

const Component* Entity::GetComponent(Name type) const
{
	return nullptr;
}

std::vector<Component*> Entity::GetComponents(Name type)
{
	return std::vector<Component*>();
}

std::vector<const Component*> Entity::GetComponents(Name type) const
{
	return std::vector<const Component*>();
}

StringView Entity::GetComponentName(StringView functionName)
{
	const size_t begin = functionName.find(STR('<'));
	const size_t end = functionName.find(STR('>'));
	return functionName.substr(begin + 1, end - begin - 1);
}

void Entity::Serialize(Json& json)
{

}

void Entity::Deserialize(const Json& json)
{

}
