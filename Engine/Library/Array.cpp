#include "Array.h"
#include <algorithm>
#include <iterator>
#include <type_traits>

namespace BE
{
	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(const Array<T, InAllocator>& other, const SizeType pos)
	{
		if (IsEmpty())
			container = other.container;
		else
			container.insert(container.cbegin() + pos,
				other.container.cbegin(), other.container.cend());
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(Array<T, InAllocator>&& other, const SizeType pos)
	{
		if (IsEmpty())
		{
			container = std::move(other.container);
			return;
		}


		Reserve(GetSize() + other.GetSize());
		Append(pos, GetSize());
		std::move(other.container.begin(),
			other.container.cend(), std::inserter(container, container.begin() + pos));
		other.Clear();
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(const T& item, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, item);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(T&& item, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, std::move(item));
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(std::initializer_list<T> elems, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, elems);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(T* ptr, SizeType count, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, ptr, ptr + count);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::AddUnique(const T& item)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		if (std::find(begin + , end, item) == end)
			Add(item);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::AddUnique(T&& item)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		if (std::find(begin + , end, item) == end)
			Add(std::move(item));
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Remove(const T& item)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		container.erase(std::find(begin, end, item));
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::RemoveLast(const T& item)
	{
		const auto begin = container.crbegin();
		const auto end = container.crend();

		container.erase(std::find(begin, end, item));
	}

	template <class T, class InAllocator>
	SizeType Array<T, InAllocator>::RemoveAll(const T& item) noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		const auto iter = std::remove(begin, end, item);
		const auto removeNum = end - iter;
		if (removeNum > 0) container.erase(iter, end);
		return removeNum;
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::RemoveAt(const SizeType index, const SizeType count/* = 1*/)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		auto erasePos = begin + index;
		container.erase(erasePos, erasePos + count);
	}

	template <class T, class InAllocator>
	template <class Predicate>
	SizeType Array<T, InAllocator>::RemoveByPredicate(Predicate&& pred) noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		const auto iter = std::remove_if(begin, end, std::forward<Predicate>(pred));
		const auto removeNum = end - iter;
		if (removeNum > 0) container.erase(iter, end);
		return removeNum;
	}
}

// It's just to avoid link error.
void SolveLink()
{
	BE::Array<int, std::allocator<int>> tmp;
}