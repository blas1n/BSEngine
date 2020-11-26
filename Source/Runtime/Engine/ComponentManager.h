#pragma once

#include <string>
#include <vector>
#include "Accessor.h"

namespace ArenaBoss
{
	class Component;
	class MeshComponent;
	class SpriteComponent;
	class UpdatableComponent;

	class ComponentManager final : private Accessor<class RenderManager>
	{
	public:
		ComponentManager() = default;

		ComponentManager(const ComponentManager&) = delete;
		ComponentManager(ComponentManager&&) = delete;

		ComponentManager& operator=(const ComponentManager&) = delete;
		ComponentManager& operator=(ComponentManager&&) = delete;

		~ComponentManager() = default;

		void Update();

		template <class ComponentType>
		ComponentType* CreateComponent(class Entity* inEntity)
		{
			auto* component = new ComponentType{ inEntity };
			RegisterComponent(component);
			return component;
		}

		Component* CreateComponent(const std::string& name, Entity* inEntity);
		
		void DeleteComponent(Component* component);
		void DeleteComponent(MeshComponent* component);
		void DeleteComponent(SpriteComponent* component);
		void DeleteComponent(UpdatableComponent* component);

	private:
		inline void RegisterComponent(Component* component) {}

		void RegisterComponent(MeshComponent* component);
		void RegisterComponent(SpriteComponent* component);

		inline void RegisterComponent(UpdatableComponent* component)
			{ updatableComponents.push_back(component); }

	private:
		std::vector<UpdatableComponent*> updatableComponents;
	};
}