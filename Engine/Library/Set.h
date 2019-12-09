#pragma once

#include "Hash.h"
#include <unordered_set>

namespace BE
{
	template <class T, template <class> class Allocator>
	class Array;

	template <class T, template <class> class Allocator>
	class BS_API Set final
	{
	private:
		using ContainerType = std::unordered_set<T, Hash<T>, std::equal_to<T>, Allocator<T>>;
		using Iterator = ContainerType::iterator;
		using ConstIterator = ContainerType::const_iterator;

	public:
		using ElementType = T;
		using AllocatorType = Allocator<T>;

		Set() noexcept = default;
		
		Set(const Set& other) = default;
		Set(Set&& other) noexcept = default;
		
		Set(std::initializer_list<T> elems) : container(elems) {}

		template <template <class> class ArrayAllocator>
		explicit Set(const Array<T, ArrayAllocator>& arr);

		template <template <class> class ArrayAllocator>
		explicit Set(Array<T, ArrayAllocator>&& arr);

		template <template <class> class OtherAllocator>
		explicit Set(const Set<T, OtherAllocator>& other);

		template <template <class> class OtherAllocator>
		explicit Set(Set<T, OtherAllocator>&& other);

		~Set() = default;

		Set& operator=(const Set& other) = default;
		Set& operator=(Set&& other) noexcept = default;

		inline Set& operator=(std::initializer_list<T> elems)
		{
			container = elems;
			return *this;
		}

		inline bool Add(const T& value) { return container.insert(value).second; }
		inline bool Add(T&& value) { return container.insert(std::move(value)).second; }

		inline void Append(std::initializer_list<T> elems) { container.insert(elems); }

		template <template <class> class ArrayAllocator>
		void Append(const Array<T, ArrayAllocator>& arr);

		template <template <class> class OtherAllocator>
		inline void Append(const Set<T, OtherAllocator>& other) { container.insert(other.begin(), other.end()); }

		template <class... Args>
		inline bool Emplace(Args&&... args) { return container.emplace(std::forward<Args>(args)...).second; }

		inline void Remove(const T& value) { container.erase(value); }

		inline void Clear() noexcept { container.clear(); }

		inline bool IsContain(const T& value) const noexcept { return container.find(value) != end() }

		Array<T, Allocator> ToArray() const;

		inline bool IsEmpty() const noexcept { return container.empty(); }
		inline SizeType GetSize() const noexcept { return container.size(); }
		inline SizeType GetMaxSize() const noexcept { return container.max_size(); }

		// Don't use! Only range based for for the function.
		inline Iterator begin() noexcept { return container.begin(); }
		inline ConstIterator begin() const noexcept { return container.begin(); }

		inline Iterator end() noexcept { return container.end(); }
		inline ConstIterator end() const noexcept { return container.end(); }

	private:
		friend bool operator==(const Set& lhs, const Set& rhs) noexcept;

		ContainerType container;
	}
}

#include "SetImpl.h"