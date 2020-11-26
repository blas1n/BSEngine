#include "Entity.h"
#include "JsonHelper.h"
#include "Transform.h"

namespace ArenaBoss
{
	Entity::Entity(const std::string& inName)
		: name(inName), transform(nullptr), components()
	{
		transform = GetManager().CreateComponent<Transform>(this);
	}

	Entity::~Entity()
	{
		for (auto* component : components)
		{
			component->Release();
			delete component;
		}

		transform->Release();
		delete transform;
	}

	void Entity::Init()
	{
		for (auto* component : components)
			component->Init();
	}

	void Entity::Release() noexcept
	{
		for (auto* component : components)
			component->Release();
	}

	void Entity::Load(const Json::Object& inObject)
	{
		for (auto component : components)
			delete component;

		components.clear();

		auto& componentManager = Accessor<ComponentManager>::GetManager();

		const auto& componentsArray = inObject["components"];
		if (!componentsArray.IsArray())
			throw std::exception{ "File is not vaild." };

		for (rapidjson::SizeType i = 0; i < componentsArray.Size(); ++i)
		{
			const auto& componentObj = componentsArray[i];
			if (!componentObj.IsObject()) continue;

			const auto type = Json::JsonHelper::GetString(componentObj, "type");
			if (!type) throw std::exception{ "Component is not vaild." };

			if (*type != "Transform")
			{
				auto* component = componentManager.CreateComponent(*type, this);
				component->Load(componentObj);
			}
			else
				transform->Load(componentObj);
		}
	}

	void Entity::Save(Json::JsonSaver& inSaver) const
	{
		Json::JsonHelper::AddString(inSaver, "name", name);
		rapidjson::Value componentsArray{ rapidjson::kArrayType };

		{
			rapidjson::Value obj{ rapidjson::kObjectType };
			Json::JsonSaver saver{ inSaver, obj };
			transform->Save(saver);
		}

		for (const auto* component : components)
		{
			rapidjson::Value obj{ rapidjson::kObjectType };
			Json::JsonSaver saver{ inSaver, obj };
			component->Save(saver);
			componentsArray.PushBack(obj, inSaver.alloc);
		}

		inSaver.object.AddMember("components", componentsArray, inSaver.alloc);
	}

	Component* Entity::FindComponent(const std::string& componentName)
	{
		for (auto component : components)
			if (component->ClassName() == componentName)
				return component;

		throw std::exception{ "No corresponding component exists." };
	}
}