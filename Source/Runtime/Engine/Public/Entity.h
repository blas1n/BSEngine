#pragma once

#include "Core.h"
#include <vector>
#include <type_traits>

class ENGINE_API Entity final
{
public:
	template <class T>
	T* AddComponent()
	{
		static_assert(std::is_base_of_v<Component, T> && !std::is_same_v<Transform, T>);
		return reinterpret_cast<T*>(AddComponent(GetComponentName(__FUNCSIG__)));
	}

	template <class T>
	T* GetComponent()
	{
		static_assert(std::is_base_of_v<Component, T>>);

		if constexpr (std::is_same_v<Transform, T>)
			return transform;
		else
			return reinterpret_cast<T*>(GetComponent(GetComponentName(__FUNCSIG__)));
	}

	template <class T>
	const T* GetComponent() const
	{
		static_assert(std::is_base_of_v<Component, T>);

		if constexpr (std::is_same_v<Transform, T>)
			return transform;
		else
			return reinterpret_cast<const T*>(GetComponent(GetComponentName(__FUNCSIG__)));
	}

	template <class T>
	std::vector<T*> GetComponents()
	{
		static_assert(std::is_base_of_v<Component, T>);

		if constexpr (std::is_same_v<Transform, T>)
			return std::vector<T*>{ transform };
		else
		{
			const auto comps = GetComponents(GetComponentName(__FUNCSIG__));
			const size_t size = comps.size();
			
			std::vector<T*> ret(size);
			for (size_t i = 0; i < size; ++i)
				ret[i] = reinterpret_cast<T*>(comps[i]);
			return ret;
		}
	}

	template <class T>
	std::vector<const T*> GetComponents() const
	{
		static_assert(std::is_base_of_v<Component, T> && !std::is_same_v<Transform, T>);

		if constexpr (std::is_same_v<Transform, T>)
			return std::vector<T*>{ transform };
		else
		{
			const auto comps = GetComponents(GetComponentName(__FUNCSIG__));
			const size_t size = comps.size();

			std::vector<T*> ret(size);
			for (size_t i = 0; i < size; ++i)
				ret[i] = reinterpret_cast<T*>(comps[i]);
			return ret;
		}
	}

	class Component* AddComponent(Name type);
	Component* GetComponent(Name type);
	const Component* GetComponent(Name type) const;
	std::vector<Component*> GetComponents(Name type);
	std::vector<const Component*> GetComponents(Name type) const;

	void Serialize(Json& json);
	void Deserialize(const Json& json);

	class Transform* GetTransform() noexcept { return transform; }
	const Transform* GetTransform() const noexcept { return transform; }

	void SetName(StringView inName) noexcept { name = inName; }
	const String& GetName() const noexcept { return name; }
	uint32 GetId() const noexcept { return id; }

private:
	StringView GetComponentName(StringView functionName);

private:
	std::vector<Component*> components;
	Transform* transform;

	String name;
	uint32 id;
};
