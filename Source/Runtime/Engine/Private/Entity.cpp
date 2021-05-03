#include "Entity.h"

Name Entity::GetComponentName(StringView functionName)
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
