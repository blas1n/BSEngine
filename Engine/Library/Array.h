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
		using Allocator = InAllocator;

		using Iterator = ContainerType::iterator;
		using ConstIterator = ContainerType::const_iterator;

		using ReverseIterator = ContainerType::reverse_iterator;
		using ConstReverseIterator = ContainerType::const_reverse_iterator;

		Array() noexcept = default;
		Array(const Array& other) = default;
		Array(Array&& other) noexcept = default;

		Array(const T* ptr, SizeType count) : arr(ptr, count) {}
		Array(std::initializer_list<T> elems) : arr(elems) {}
		
		template <class OtherElement>
		explicit Array(const Array<OtherElement, InAllocator>& other);

		template <class OtherElement>
		explicit Array(Array<OtherElement, InAllocator>&& other);

		~Array() = default;

		inline Array& operator=(const Array& other)
		{
			arr = other.arr;
			return *this;
		}

		inline Array& operator=(Array&& other) noexcept
		{
			arr = std::move(other.arr);
			return *this;
		}

		inline Array& operator=(std::initializer_list<T> elems)
		{
			arr = elems;
			return *this;
		}

		inline bool operator==(const Array& other) const noexcept { return arr == other.arr; }
		inline bool operator!=(const Array& other) const noexcept { return arr != other.arr; }

		inline T& operator[](SizeType index) { return arr.at(index); }
		inline const T& operator[](SizeType index) const { return arr.at(index); }

		void Insert(const Array& other, ConstIterator pos);
		void Insert(Array&& other, ConstIterator pos);

		void Insert(const T& item, ConstIterator pos);
		void Insert(T&& item, ConstIterator pos);

		void Insert(std::initializer_list<T> elems, ConstIterator pos);
		void Insert(T* ptr, SizeType count, ConstIterator pos);

		template <class Iterator>
		void Array<T, InAllocator>::Insert(Iterator begin, Iterator end, ConstIterator pos);

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
			arr.emplace_back(std::forward<Args>(args)...);
		}

		template <class... Args>
		inline Iterator EmplaceAt(ConstIterator pos, Args&&... args)
		{
			arr.emplace(pos, std::forward<Args>(args)...);
		}

		void Remove(const T& item);
		void RemoveLast(const T& item);

		SizeType RemoveAll(const T& item) noexcept;

		void RemoveAt(SizeType index, SizeType count = 1);

		template <class Predicate>
		SizeType RemoveByPredicate(Predicate&& pred) noexcept;

		inline void Resize(SizeType size, const T& value = T()) { arr.resize(size, value); }
		inline void Reserve(SizeType size) { arr.reserve(size); }

		inline Iterator Begin() noexcept { return arr.begin(); }
		inline ConstIterator Begin() const noexcept { return arr.begin(); }
		inline ConstIterator CBegin() const noexcept { return arr.cbegin(); }

		inline Iterator End() noexcept { return arr.end(); }
		inline ConstIterator End() const noexcept { return arr.end(); }
		inline ConstIterator CEnd() const noexcept { return arr.cend(); }

		inline ReverseIterator RBegin() noexcept { return arr.rbegin(); }
		inline ConstReverseIterator RBegin() const noexcept { return arr.rbegin(); }
		inline ConstReverseIterator CRBegin() const noexcept { return arr.crbegin(); }

		inline ReverseIterator REnd() noexcept { return arr.rend(); }
		inline ConstReverseIterator REnd() const noexcept { return arr.rend(); }
		inline ConstReverseIterator CREnd() const noexcept { return arr.crend(); }

		inline bool IsEmpty() const noexcept { return arr.empty(); }
		
		inline SizeType GetSize() const noexcept { return arr.size(); }
		inline SizeType GetCapacity() const noexcept { return arr.capacity(); }

		inline void Shrink() noexcept { arr.shrink_to_fit(); }

		inline void Clear() noexcept { arr.clear(); }

		//inline Iterator begin() noexcept { return arr.begin(); }
		//inline ConstIterator begin() const noexcept { return arr.begin(); }

		//inline Iterator end() noexcept { return arr.end(); }
		//inline ConstIterator end() const noexcept { return arr.end(); }

	private:
		ContainerType arr;
	};
}