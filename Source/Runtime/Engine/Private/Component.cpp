#include "Component.h"
#include <unordered_map>

namespace
{
	class ComponentRegistry final
	{
	public:
		static ComponentRegistry& Get()
		{
			static std::unique_ptr<ComponentRegistry> inst = std::make_unique<ComponentRegistry>();
			return *inst;
		}

		Component* CreateComponent(Name name, Entity* entity) const
		{
			return registry.find(name)->second(entity);
		}

		void RegisterComponent(Name name, Component*(*ptr)(Entity*))
		{
			registry.insert(std::make_pair(name, ptr));
		}

	private:
		std::unordered_map<Name, Component*(*)(Entity*)> registry;
	};
}

Component* Impl::CreateComponent(Name name, Entity* entity)
{
	ComponentRegistry::Get().CreateComponent(name, entity);
}

void Impl::RegisterComponent(Name name, Component*(*ptr)(Entity*))
{
	ComponentRegistry::Get().RegisterComponent(name, ptr);
}
