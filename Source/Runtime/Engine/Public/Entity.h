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
	Entity(class Scene* inScene, uint32 inId) : scene(inScene), id(inId) {}

	Entity(const Entity&) = default;
	Entity(Entity&&) noexcept = default;

	Entity& operator=(Entity&) = default;
	Entity& operator=(Entity&&) noexcept = default;

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
	T* GetComponent()
	{
		return const_cast<T*>(static_cast<const Entity*>(this)->GetComponent<T>());
	}

	template <class T>
	const T* GetComponent() const
	{
		static_assert(std::is_base_of_v<Component, T>);

		const auto iter = components.find(GetComponentName(STR(FUNC_SIG)));
		return iter != components.cend() ? reinterpret_cast<const T*>(iter->second) : nullptr;
	}

	template <class T>
	std::vector<T*> GetComponents()
	{
		const std::vector<const T*> vec = static_cast<const Entity*>(this)->GetComponents<T>();
		const size_t size = vec.size();

		std::vector<T*> ret(vec.size());
		for (size_t i = 0; i < size; ++i)
			ret[i] = const_cast<T*>(vec[i]);
		return ret;
	}

	template <class T>
	std::vector<const T*> GetComponents() const
	{
		static_assert(std::is_base_of_v<Component, T>);

		const auto iters = components.equal_range(GetComponentName(STR(FUNC_SIG)));
		const size_t size = std::distance(iters.first, iters.second);

		std::vector<const T*> ret(size);
		for (size_t i = 0; i < size; ++i)
			ret[i] = reinterpret_cast<const T*>(iters.first->second + i);
		return ret;
	}

	Json Serialize() const;
	void Deserialize(const Json& json);

	Scene* GetScene() noexcept { return scene; }
	const Scene* GetScene() const noexcept { return scene; }

	void SetName(StringView inName) noexcept { name = inName; }
	const String& GetName() const noexcept { return name; }

	uint32 GetId() const noexcept { return id; }

private:
	static Name GetComponentName(StringView functionName);

public:
	constexpr static uint32 IdNone = static_cast<uint32>(-1);

private:
	std::unordered_multimap<Name, Component*, Hash<Name>> components;

	Scene* scene;

	String name;
	uint32 id;
};

#undef FUNC_SIG
