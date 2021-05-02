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

		Component* CreateComponent(Name name) const
		{
			return registry.find(name)->second();
		}

		void RegisterComponent(Name name, Component*(*ptr)())
		{
			registry.insert(std::make_pair(name, ptr));
		}

	private:
		std::unordered_map<Name, Component*(*)()> registry;
	};
}

Component* Impl::CreateComponent(Name name)
{
	ComponentRegistry::Get().CreateComponent(name);
}

void Impl::RegisterComponent(Name name, Component*(*ptr)())
{
	ComponentRegistry::Get().RegisterComponent(name, ptr);
}
