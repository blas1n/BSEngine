#pragma once

#include "Core.h"
#include <type_traits>
#include <unordered_map>
#include "Transform.h"

class ENGINE_API Entity final
{
public:
	template <class T>
	T* AddComponent()
	{
		static_assert(std::is_base_of_v<Component, T> && !std::is_same_v<Transform, T>);
		
		const Name name = GetComponentName(STR(__FUNCSIG__));
		const auto ret = CreateComponent<T>(name);
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
		static_assert(std::is_base_of_v<Component, T >> );

		if constexpr (std::is_same_v<Transform, T>)
			return transform;
		else
		{
			const auto iter = components.find(GetComponentName(STR(__FUNCSIG__)));
			return iter != component.end() ? reinterpret_cast<T*>(iter->second) : nullptr;
		}
	}

	template <class T>
	std::vector<T*> GetComponents()
	{
		const std::vector<const T*> vec = static_cast<const Entity*>(this)->GetComponent<T>();
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

		if constexpr (std::is_same_v<Transform, T>)
			return std::vector<T*>{ transform };
		else
		{
			const auto iters = components.equal_range(GetComponentName(STR(__FUNCSIG__)));
			const size_t size = std::distance(iters.first, iters.second);

			std::vector<T*> ret(size);
			for (size_t i = 0; i < size; ++i)
				ret[i] = reinterpret_cast<T*>(iters.first->second + i);
			return ret;
		}
	}

	void Serialize(Json& json);
	void Deserialize(const Json& json);

	class Transform* GetTransform() noexcept { return transform; }
	const Transform* GetTransform() const noexcept { return transform; }

	void SetName(StringView inName) noexcept { name = inName; }
	const String& GetName() const noexcept { return name; }

private:
	Name GetComponentName(StringView functionName);

private:
	std::unordered_multimap<Name, Component*, Hash<Name>> components;
	Transform* transform;

	String name;
	uint32 id;
};
