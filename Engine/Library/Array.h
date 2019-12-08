#pragma once

#include "Core.h"
#include <vector>

namespace BE
{
	template <class T, class InAllocator>
	class BS_API Array final
	{
	private:
		using ContainerType = std::vector<T, InAllocator>;

	public:
		using ElementType = T;
		using AllocatorType = InAllocator;

		Array() noexcept = default;
		Array(const Array& other) = default;
		Array(Array&& other) noexcept = default;

		Array(const T* ptr, SizeType count) : container(ptr, count) {}
		Array(std::initializer_list<T> elems) : container(elems) {}
		
		template <class OtherElement>
		explicit Array(const Array<OtherElement, InAllocator>& other);

		template <class OtherElement>
		explicit Array(Array<OtherElement, InAllocator>&& other);

		~Array() = default;

		inline Array& operator=(const Array& other)
		{
			container = other.container;
			return *this;
		}

		inline Array& operator=(Array&& other) noexcept
		{
			container = std::move(other.container);
			return *this;
		}

		inline Array& operator=(std::initializer_list<T> elems)
		{
			container = elems;
			return *this;
		}

		inline T& operator[](SizeType index) { return container.at(index); }
		inline const T& operator[](SizeType index) const { return container.at(index); }

		void Insert(const Array& other, SizeType pos);
		void Insert(Array&& other, SizeType pos);

		void Insert(const T& item, SizeType pos);
		void Insert(T&& item, SizeType pos);

		void Insert(std::initializer_list<T> elems, SizeType pos);
		void Insert(T* ptr, SizeType count, SizeType pos);

		inline void Add(const T& item) { Insert(item, GetSize()); }
		inline void Add(T&& item) { Insert(item, GetSize()); }

		void AddUnique(const T& item);
		void AddUnique(T&& item);

		inline void Append(std::initializer_list<T> elems) { Insert(elems, GetSize()); }
		inline void Append(const T* ptr, SizeType count) { Insert(ptr, count, GetSize()); }

		inline void Append(const Array& other) { Insert(other, GetSize()); }
		inline void Append(Array&& other) { Insert(std::move(other), GetSize()); }
	
		template <class... Args>
		inline void Emplace(Args&&... args)
		{
			container.emplace_back(std::forward<Args>(args)...);
		}

		template <class... Args>
		inline void EmplaceAt(SizeType pos, Args&&... args)
		{
			container.emplace(container.cbegin() + pos, std::forward<Args>(args)...);
		}

		void Remove(const T& item);
		void RemoveLast(const T& item);

		SizeType RemoveAll(const T& item) noexcept;

		void RemoveAt(SizeType index, SizeType count = 1);

		template <class Predicate>
		SizeType RemoveByPredicate(Predicate&& pred) noexcept;

		inline void Resize(SizeType size, const T& value = T()) { container.resize(size, value); }
		inline void Reserve(SizeType size) { container.reserve(size); }

		inline bool IsEmpty() const noexcept { return container.empty(); }
		
		inline SizeType GetSize() const noexcept { return container.size(); }
		inline SizeType GetCapacity() const noexcept { return container.capacity(); }

		inline void Shrink() noexcept { container.shrink_to_fit(); }

		inline void Clear() noexcept { container.clear(); }

		// Don't use! Only range based for for the function.
		inline typename ContainerType::iterator begin() noexcept { return container.begin(); }
		inline typename ContainerType::const_iterator begin() const noexcept { return container.begin(); }

		inline typename ContainerType::iterator end() noexcept { return container.end(); }
		inline typename ContainerType::const_iterator end() const noexcept { return container.end(); }

	private:
		friend bool operator==(const Array& lhs, const Array& rhs) noexcept;

		ContainerType container;
	};

	template <class T, class InAllocator>
	bool operator==(const Array<T, InAllocator>& lhs, const Array<T, InAllocator>& rhs)
	{
		return lhs.container == rhs.container;
	}

	template <class T, class InAllocator>
	bool operator!=(const Array<T, InAllocator>& lhs, const Array<T, InAllocator>& rhs)
	{
		return !(lhs == rhs);
	}

	template <class T, class InAllocator>
	template <class OtherElement>
	Array<T, InAllocator>::Array(const Array<OtherElement, InAllocator>& other)
	{
		Reserve(other.GetSize());
		for (auto& elem : other)
			Emplace(elem);
	}

	template <class T, class InAllocator>
	template <class OtherElement>
	Array<T, InAllocator>::Array(Array<OtherElement, InAllocator>&& other)
	{
		Reserve(other.GetSize());
		for (auto&& elem : std::move(other))
			Emplace(std::move(elem));
	}
}