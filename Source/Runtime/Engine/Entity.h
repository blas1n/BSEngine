#pragma once

#include <string>
#include <vector>
#include "ComponentManager.h"
#include "Game.h"
#include "JsonForwarder.h"
#include "Log.h"

namespace ArenaBoss
{
	class Component;
	class Transform;

	class Entity final : private Accessor<ComponentManager>
	{
	public:
		explicit Entity(const std::string& inName);
		
		Entity(const Entity&) = delete;
		Entity(Entity&&) = delete;

		Entity& operator=(const Entity&) = delete;
		Entity& operator=(Entity&&) = delete;

		~Entity();

		void Init();
		void Release() noexcept;

		void Load(const Json::Object& inObject);
		void Save(Json::JsonSaver& inSaver) const;

		template <class ComponentType>
		ComponentType& GetComponent() 
		{
			const auto component = FindComponent(ComponentType::StaticClassName());
			return *(static_cast<ComponentType*>(component));
		}

		template <class ComponentType>
		const ComponentType& GetComponent() const
		{
			const auto component = FindComponent(ComponentType::StaticClassName());
			return *(static_cast<ComponentType*>(component));
		}

		template <>
		inline Transform& GetComponent<Transform>() noexcept { return *transform; }

		template <>
		inline const Transform& GetComponent<Transform>() const noexcept { return *transform; }

		template <class ComponentType>
		ComponentType& AddComponent()
		{
			auto* component = GetManager().CreateComponent<ComponentType>(this);
			components.push_back(component);
			return component;
		}

		template <>
		Transform& AddComponent<Transform>() noexcept
		{
			Log("Transform cannot be added.");
			Game::Exit();
		}

		inline const std::string& GetName() const noexcept { return name; }
		inline void SetName(const std::string& inName) noexcept { name = inName; }
		inline void SetName(std::string&& inName) noexcept { name = std::move(inName); }

	private:
		Component* FindComponent(const std::string& componentName);

	private:
		friend bool operator==(const Entity& lhs, const Entity& rhs);
		friend bool operator<(const Entity& lhs, const Entity& rhs);

		friend bool operator==(const Entity& lhs, const std::string& rhs);
		friend bool operator<(const Entity& lhs, const std::string& rhs);

		friend bool operator==(const std::string& lhs, const Entity& rhs);
		friend bool operator<(const std::string& lhs, const Entity& rhs);

		std::string name;
		Transform* transform;
		std::vector<Component*> components;
	};

	inline bool operator==(const Entity& lhs, const Entity& rhs) { return lhs.name == rhs.name; }
	inline bool operator!=(const Entity& lhs, const Entity& rhs) { return !(lhs == rhs); }
	inline bool operator<(const Entity& lhs, const Entity& rhs) { return lhs.name < rhs.name; }
	inline bool operator>(const Entity& lhs, const Entity& rhs) { return rhs < lhs; }
	inline bool operator<=(const Entity& lhs, const Entity& rhs) { return !(rhs < lhs); }
	inline bool operator>=(const Entity& lhs, const Entity& rhs) { return !(lhs < rhs); }

	inline bool operator==(const Entity& lhs, const std::string& rhs) { return lhs.name == rhs; }
	inline bool operator!=(const Entity& lhs, const std::string& rhs) { return !(lhs == rhs); }
	inline bool operator<(const Entity& lhs, const std::string& rhs) { return lhs.name < rhs; }
	inline bool operator>(const Entity& lhs, const std::string& rhs) { return rhs < lhs; }
	inline bool operator<=(const Entity& lhs, const std::string& rhs) { return !(rhs < lhs); }
	inline bool operator>=(const Entity& lhs, const std::string& rhs) { return !(lhs < rhs); }

	inline bool operator==(const std::string& lhs, const Entity& rhs) { return lhs == rhs.name; }
	inline bool operator!=(const std::string& lhs, const Entity& rhs) { return !(lhs == rhs); }
	inline bool operator<(const std::string& lhs, const Entity& rhs) { return lhs < rhs.name; }
	inline bool operator>(const std::string& lhs, const Entity& rhs) { return rhs < lhs; }
	inline bool operator<=(const std::string& lhs, const Entity& rhs) { return !(rhs < lhs); }
	inline bool operator>=(const std::string& lhs, const Entity& rhs) { return !(lhs < rhs); }
}