#pragma once

#include "Core.h"
#include <vector>

namespace BE
{
	template <class T, template <class> class Allocator>
	class BS_API Array final
	{
	public:
		using ElementType = T;
		using AllocatorType = Allocator<T>;

	private:
		using ContainerType = std::vector<T, Allocator<T>>;
		using Iterator = ContainerType::iterator;
		using ConstIterator = ContainerType::const_iterator;

	public:
		Array() noexcept = default;
		Array(const Array& other) = default;
		Array(Array&& other) noexcept = default;

		Array(const T* ptr, SizeType count) : container(ptr, count) {}
		Array(std::initializer_list<T> elems) : container(elems) {}
		
		explicit Array(SizeType count) : container(count) {}

		explicit Array(SizeType count, const T& value)
			: container(count, value) {}

		template <template <class> class OtherAllocator>
		explicit Array(const Array<T, OtherAllocator>& other);

		template <template <class> class OtherAllocator>
		explicit Array(Array<T, OtherAllocator>&& other);

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
		void Insert(const T* ptr, SizeType count, SizeType pos);

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

		bool Find(const T& value, SizeType& index) const noexcept;
		SizeType Find(const T& value) const noexcept;

		bool FindLast(const T& value, SizeType& index) const noexcept;
		SizeType FindLast(const T& value) const noexcept;

		Array<T&, Allocator> FindAll(const T& value) noexcept;
		Array<const T&, Allocator> FindAll(const T& value) const noexcept;

		template <class Pred>
		bool Find(Pred&& pred, SizeType& index) const noexcept;

		template <class Pred>
		SizeType Find(Pred&& pred) const noexcept;

		template <class Pred>
		bool FindLast(Pred&& pred, SizeType& index) const noexcept;

		template <class Pred>
		SizeType FindLast(Pred&& pred) const noexcept;

		template <class Pred>
		Array<T&, Allocator> FindAll(Pred&& pred) noexcept;

		template <class Pred>
		Array<const T&, Allocator> FindAll(Pred&& pred) const noexcept;

		void Sort() noexcept;

		template <class Pred>
		void Sort(Pred&& pred) noexcept;

		inline void Resize(SizeType size, const T& value = T()) { container.resize(size, value); }
		inline void Reserve(SizeType size) { container.reserve(size); }

		inline bool IsEmpty() const noexcept { return container.empty(); }
		
		inline SizeType GetSize() const noexcept { return container.size(); }
		inline SizeType GetCapacity() const noexcept { return container.capacity(); }

		inline void Shrink() noexcept { container.shrink_to_fit(); }

		inline void Clear() noexcept { container.clear(); }

		constexpr void Swap(Array& other) noexcept(noexcept(container.swap(other.container)))
		{
			container.swap(other.container);
		}

		// Don't use! Only range based for for the function.
		inline Iterator begin() noexcept { return container.begin(); }
		inline ConstIterator begin() const noexcept { return container.begin(); }

		inline Iterator end() noexcept { return container.end(); }
		inline ConstIterator end() const noexcept { return container.end(); }

	private:
		friend bool operator==(const Array& lhs, const Array& rhs) noexcept;

		ContainerType container;
	};

	template <class T, template <class> class Allocator>
	bool operator==(const Array<T, Allocator>& lhs, const Array<T, Allocator>& rhs)
	{
		return lhs.container == rhs.container;
	}

	template <class T, template <class> class Allocator>
	bool operator!=(const Array<T, Allocator>& lhs, const Array<T, Allocator>& rhs)
	{
		return !(lhs == rhs);
	}

	template <class T, template <class> class Allocator>
	constexpr void Swap(Array<T, Allocator>& lhs, Array<T, Allocator>& rhs) noexcept(noexcept(lhs.Swap(rhs)))
	{
		lhs.Swap(rhs);
	}
}

#include "ArrayImpl.h"