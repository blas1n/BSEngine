#include "ComponentManager.h"
#include <algorithm>
#include <exception>
#include <map>
#include <type_traits>
#include "DrawableComponent.h"
#include "MeshComponent.h"
#include "RenderManager.h"
#include "SpriteComponent.h"
#include "Transform.h"
#include "UpdatableComponent.h"

#define FACTORY_PAIR(name) std::make_pair(#name, &Create<name>)

namespace ArenaBoss
{
	namespace
	{
		template <class T>
		Component* Create(ComponentManager* manager, Entity* entity)
		{
			return static_cast<Component*>(manager->CreateComponent<T>(entity));
		}

		const std::map<std::string, Component*(*)(ComponentManager*, Entity*)> factoryMap
		{
			FACTORY_PAIR(Component), FACTORY_PAIR(Transform), FACTORY_PAIR(MeshComponent)
		};
	}

	void ComponentManager::Update()
	{
		for (auto& component : updatableComponents)
			component->Update();
	}

	Component* ComponentManager::CreateComponent(const std::string& name, Entity* inEntity)
	{
		return factoryMap.at(name)(this, inEntity);
	}

	void ComponentManager::DeleteComponent(Component* component)
	{
		delete component;
	}

	void ComponentManager::DeleteComponent(MeshComponent* component)
	{
		Accessor<RenderManager>::GetManager().UnregisterComponent(component);
		delete component;
	}

	void ComponentManager::DeleteComponent(SpriteComponent* component)
	{
		Accessor<RenderManager>::GetManager().UnregisterComponent(component);
		delete component;
	}

	void ComponentManager::DeleteComponent(UpdatableComponent* component)
	{
		const auto iter = std::find(updatableComponents.begin(), updatableComponents.end(), component);
		updatableComponents.erase(iter);
		delete component;
	}

	void ComponentManager::RegisterComponent(MeshComponent* component)
	{
		Accessor<RenderManager>::GetManager().RegisterComponent(component);
	}

	void ComponentManager::RegisterComponent(SpriteComponent* component)
	{
		Accessor<RenderManager>::GetManager().RegisterComponent(component);
	}
}