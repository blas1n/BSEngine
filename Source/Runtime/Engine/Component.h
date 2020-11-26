#pragma once

#include <string>
#include "JsonForwarder.h"

namespace ArenaBoss
{
	class Entity;

	class Component
	{
	public:
		Component(const Component&) = delete;
		Component(Component&&) = delete;

		Component& operator=(const Component&) = delete;
		Component& operator=(Component&&) = delete;

		virtual ~Component() { Destruct(); }

		virtual void Construct() {}
		virtual void Destruct() noexcept {}

		virtual void Init() {}
		virtual void Release() noexcept {}

		virtual void Load(const Json::Object& object) {}
		virtual void Save(Json::JsonSaver& saver) const;

		inline static const std::string& StaticClassName() noexcept
		{
			static const std::string name{ "Component" };
			return name;
		}

		inline virtual const std::string& ClassName() const noexcept
		{
			return Component::StaticClassName();
		}

		Entity* GetEntity() noexcept { return entity; }

	protected:
		Component(Entity* inEntity) : entity(inEntity)
		{
			Construct();
		}

	private:
		friend class ComponentManager;
		Entity* entity;
	};

#define GENERATE_COMPONENT_IMPL(name, super) \
public: \
	using Super = super; \
\
	~name() override { Destruct(); } \
\
	inline static const std::string& StaticClassName() noexcept \
	{ \
		static const std::string componentName{ #name }; \
		return componentName; \
	} \
\
	inline const std::string& ClassName() const noexcept override \
	{ \
		return name::StaticClassName(); \
	} \
\
protected: \
	name(Entity* inEntity) : Super(inEntity) { Construct(); } \
\
private: \
	friend class ComponentManager;

#define GENERATE_COMPONENT1(name) GENERATE_COMPONENT_IMPL(name, Component)
#define GENERATE_COMPONENT2(name, super) GENERATE_COMPONENT_IMPL(name, super)
}