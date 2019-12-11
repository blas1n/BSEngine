#pragma once

#include "Comparison.h"
#include "Pair.h"
#include <map>

namespace BE
{
	template <class Key, class Value, template <class> class Allocator, template <class> class Comp = Less>
	class BS_API Map final
	{
	public:
		using KeyType = Key;
		using ValueType = Value;
		using CompType = Less<Key>;
		using AllocatorType = Allocator<std::pair<const Key, Value>>;

	private:
		using ContainerType = std::map<Key, Value, CompType, AllocatorType>;
		using Iterator = ContainerType::iterator;
		using ConstIterator = ContainerType::const_iterator;

	public:
		Map() noexcept = default;

		Map(const Map& other) = default;
		Map(Map&& other) noexcept = default;

		Map(std::initializer_list<Pair<Key, Value>> elems)
			: container(elems.begin(), elems.end()) {}

		template <template <class> class OtherAllocator>
		explicit Map(const Map<Key, Value, OtherAllocator, Comp>& other)
			: container(other.begin(), other.end()) {}

		template <template <class> class OtherAllocator>
		explicit Map(Map<Key, Value, OtherAllocator, Comp>&& other)
			: container(std::make_move_iterator(other.begin()), std::make_move_iterator(other.end())) {}

		~Map() = default;

		Map& operator=(const Map& other) = default;
		Map& operator=(Map&& other) noexcept = default;

		inline Value& operator[](const Key& key) { return container.at(key); }
		inline const Value& operator[](const Key& key) const { return container.at(key); }

		inline bool Add(const Key& key, const Value& value) { return Emplace(key, value); }
		inline bool Add(const Key& key, Value&& value) { return Emplace(key, Move(value)); }
		inline bool Add(Key&& key, const Value& value) { return Emplace(Move(key), value); }
		inline bool Add(Key&& key, Value&& value) { return Emplace(Move(key), Move(value)); }

		inline bool Add(const Pair<Key, Value>& pair) { return Emplace(pair); }
		inline bool Add(Pair<Key, Value>&& pair) { return Emplace(Move(pair)); }

		template <template <class> class OtherAllocator>
		inline void Append(const Map<Key, Value, OtherAllocator, Comp>& other)
		{
			container.insert(other.begin(), other.end());
		}

		template <template <class> class OtherAllocator>
		inline void Append(Map<Key, Value, OtherAllocator, Comp>&& other)
		{
			container.insert(std::make_move_iterator(other.begin()), std::make_move_iterator(other.end()));
		}

		template <class... Args>
		inline bool Emplace(Args&&... args)
		{
			return container.emplace(std::forward<Args>(args)...).second;
		}

		inline void Remove(const Key& value) { container.erase(value); }

		inline void Clear() noexcept { container.clear(); }

		Value* Find(const Key& key)
		{
			const auto iter = container.find(key);
			if (iter == end()) return nullptr;
			return &*iter;
		}

		const Value* Find(const Key& key) const
		{
			const auto iter = container.find(key);
			if (iter == end()) return nullptr;
			return &*iter;
		}

		Value* FindOrAdd(const Key& key)
		{
			const auto iter = container.find(key);

			if (iter == end())
				iter = container.insert(std::make_pair(key, Value());

			return &*iter;
		}

		const Value* FindOrAdd(const Key& key) const
		{
			const auto iter = container.find(key);

			if (iter == end())
				iter = container.insert(std::make_pair(key, Value());

			return &*iter;
		}

		inline bool IsEmpty() const noexcept { return container.empty(); }
		inline SizeType GetSize() const noexcept { return container.size(); }
		inline SizeType GetMaxSize() const noexcept { return container.max_size(); }

		constexpr void Swap(Map& other) noexcept(noexcept(std::swap(container, other.container)))
		{
			std::swap(container, other.container);
		}

		// Don't use! Only range based for for the function.
		inline Iterator begin() noexcept { return container.begin(); }
		inline ConstIterator begin() const noexcept { return container.begin(); }

		inline Iterator end() noexcept { return container.end(); }
		inline ConstIterator end() const noexcept { return container.end(); }

	private:
		friend bool operator==(const Map& lhs, const Map& rhs) noexcept;

		ContainerType container;
	}

	template <class Key, class Value, template <class> class Allocator, template <class> class Comp = Less>
	bool operator==(const Map<Key, Value, Allocator, Comp>& lhs, const Map<Key, Value, Allocator, Comp>& rhs)
	{
		return lhs.container == rhs.container;
	}

	template <class Key, class Value, template <class> class Allocator, template <class> class Comp = Less>
	bool operator!=(const Map<Key, Value, Allocator, Comp>& lhs, const Map<Key, Value, Allocator, Comp>& rhs)
	{
		return !(lhs == rhs);
	}

	template <class Key, class Value, template <class> class Allocator, template <class> class Comp = Less>
	constexpr void Swap(Map<Key, Value, Allocator, Comp>& lhs, Map<Key, Value, Allocator, Comp>& rhs) noexcept(noexcept(lhs.Swap(rhs)))
	{
		lhs.Swap(rhs);
	}
}

#include "MapImpl.h"