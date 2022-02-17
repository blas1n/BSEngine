#pragma once

#include "Core.h"
#include <type_traits>
#include <unordered_map>
#include "Component.h"

#if WINDOWS
#	define FUNC_SIG __FUNCSIG__
#else
#	define FUNC_SIG __PRETTY_FUNCTION__
#endif

class ENGINE_API Entity final
{
public:
	Entity() : components(), name() {}

	Entity(const Entity& other)
		: components(other.components)
		, name(other.name)
	{
		// Todo: Create name appender
	}

	Entity(Entity&& other) noexcept
		: components(std::move(other.components))
		, name(std::move(other.name))
	{
		// Todo: Create name appender
		other.components.clear();
	}

	Entity& operator=(const Entity& other)
	{
		components = other.components;
		return *this;
	}

	Entity& operator=(Entity&& other) noexcept
	{
		components = std::move(other.components);
		other.components.clear();
		return *this;
	}

	~Entity();

	template <class T>
	T* AddComponent()
	{
		static_assert(std::is_base_of_v<Component, T>);
		
		const Name name = GetComponentName(STR(FUNC_SIG));
		const auto ret = CreateComponent<T>(name, this);
		components.insert(std::make_pair(name, ret));
		return ret;
	}

	template <class T>
	[[nodiscard]] T* GetComponent()
	{
		return const_cast<T*>(static_cast<const Entity*>(this)->GetComponent<T>());
	}

	template <class T>
	[[nodiscard]] const T* GetComponent() const
	{
		const auto& comps = GetComponents<T>();
		return comps.empty() ? nullptr : comps[0];
	}

	template <class T>
	[[nodiscard]] std::vector<T*> GetComponents()
	{
		return reinterpret_cast<const std::vector<T*>&>
			(const_cast<const Entity*>(this)->GetComponents<T>());
	}

	template <class T>
	[[nodiscard]] std::vector<const T*> GetComponents() const
	{
		static_assert(std::is_base_of_v<Component, T>);

		const auto iter = components.find(GetComponentName(STR(FUNC_SIG)));
		if (iter == components.cend())
			return std::vector<const T*>{};
		
		const auto& comps = iter->second;
		const size_t size = comps.size();
		std::vector<const T*> ret(size, nullptr);
			
		for (size_t i = 0; i < size; ++i)
			ret[i] = reinterpret_cast<const T*>(comps[i]);

		return ret;
	}

	[[nodiscard]] Json Serialize() const;
	void Deserialize(const Json& json);

	[[nodiscard]] const String& GetName() const noexcept { return name; }
	void SetName(StringView inName) noexcept;

private:
	[[nodiscard]] static Name GetComponentName(StringView functionName);

public:
	Event<void(Entity&, const String&, const String&)> onChangedName;

private:
	std::unordered_map<Name, std::vector<Component*>, Hash<Name>> components;
	String name;
};

#undef FUNC_SIG
