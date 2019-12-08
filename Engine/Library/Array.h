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

		using Iterator = typename ContainerType::iterator;
		using ConstIterator = typename ContainerType::const_iterator;

		using ReverseIterator = typename ContainerType::reverse_iterator;
		using ConstReverseIterator = typename ContainerType::const_reverse_iterator;

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

		inline bool operator==(const Array& other) const noexcept { return container == other.container; }
		inline bool operator!=(const Array& other) const noexcept { return container != other.container; }

		inline T& operator[](SizeType index) { return container.at(index); }
		inline const T& operator[](SizeType index) const { return container.at(index); }

		void Insert(const Array& other, ConstIterator pos);
		void Insert(Array&& other, ConstIterator pos);

		void Insert(const T& item, ConstIterator pos);
		void Insert(T&& item, ConstIterator pos);

		void Insert(std::initializer_list<T> elems, ConstIterator pos);
		void Insert(T* ptr, SizeType count, ConstIterator pos);

		template <class IteratorType>
		void Insert(IteratorType begin, IteratorType end, ConstIterator pos);

		inline void Add(const T& item) { Insert(item, End()); }
		inline void Add(T&& item) { Insert(item, End()); }

		void AddUnique(const T& item);
		void AddUnique(T&& item);

		inline void Append(std::initializer_list<T> elems) { Insert(elems, End()); }
		inline void Append(const T* ptr, SizeType count) { Insert(ptr, count, End()); }

		inline void Append(const Array& other) { Insert(other, End()); }
		inline void Append(Array&& other) { Insert(std::move(other), End()); }

		template <class Iterator>
		inline void Array<T, InAllocator>::Insert(Iterator begin, Iterator end) { Insert(begin, end); }

		template <class... Args>
		inline Iterator Emplace(Args&&... args)
		{
			container.emplace_back(std::forward<Args>(args)...);
			return --End();
		}

		template <class... Args>
		inline Iterator EmplaceAt(ConstIterator pos, Args&&... args)
		{
			return container.emplace(pos, std::forward<Args>(args)...);
		}

		void Remove(const T& item);
		void RemoveLast(const T& item);

		SizeType RemoveAll(const T& item) noexcept;

		void RemoveAt(SizeType index, SizeType count = 1);

		template <class Predicate>
		SizeType RemoveByPredicate(Predicate&& pred) noexcept;

		inline void Resize(SizeType size, const T& value = T()) { container.resize(size, value); }
		inline void Reserve(SizeType size) { container.reserve(size); }

		inline Iterator Begin() noexcept { return container.begin(); }
		inline ConstIterator Begin() const noexcept { return container.begin(); }
		inline ConstIterator CBegin() const noexcept { return container.cbegin(); }

		inline Iterator End() noexcept { return container.end(); }
		inline ConstIterator End() const noexcept { return container.end(); }
		inline ConstIterator CEnd() const noexcept { return container.cend(); }

		inline ReverseIterator RBegin() noexcept { return container.rbegin(); }
		inline ConstReverseIterator RBegin() const noexcept { return container.rbegin(); }
		inline ConstReverseIterator CRBegin() const noexcept { return container.crbegin(); }

		inline ReverseIterator REnd() noexcept { return container.rend(); }
		inline ConstReverseIterator REnd() const noexcept { return container.rend(); }
		inline ConstReverseIterator CREnd() const noexcept { return container.crend(); }

		inline bool IsEmpty() const noexcept { return container.empty(); }
		
		inline SizeType GetSize() const noexcept { return container.size(); }
		inline SizeType GetCapacity() const noexcept { return container.capacity(); }

		inline void Shrink() noexcept { container.shrink_to_fit(); }

		inline void Clear() noexcept { container.clear(); }

		inline Iterator begin() noexcept { return container.begin(); }
		inline ConstIterator begin() const noexcept { return container.begin(); }

		inline Iterator end() noexcept { return container.end(); }
		inline ConstIterator end() const noexcept { return container.end(); }

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

	template <class T, class InAllocator>
	template <class IteratorType>
	void Array<T, InAllocator>::Insert(IteratorType begin, IteratorType end, ConstIterator pos)
	{
		container.insert(pos, begin, end);
	}
}