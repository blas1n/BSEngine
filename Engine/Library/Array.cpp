#include "Array.h"
#include <algorithm>
#include <iterator>
#include <type_traits>

namespace BE
{
	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(const Array<T, InAllocator>& other, ConstIterator pos)
	{
		if (IsEmpty())
			arr = other.arr;
		else
			Insert(other.Begin(), other.End(), pos);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(Array<T, InAllocator>&& other, ConstIterator pos)
	{
		if (IsEmpty())
		{
			arr = std::move(other.arr);
			return;
		}


		Reserve(GetSize() + other.GetSize());
		Append(pos, CEnd());
		std::move(other.Begin(), other.End(), std::inserter(arr, pos));
		other.Clear();
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(const T& item, ConstIterator pos)
	{
		arr.insert(pos, item);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(T&& item, ConstIterator pos)
	{
		arr.insert(pos, std::move(item));
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(std::initializer_list<T> elems, ConstIterator pos)
	{
		arr.insert(pos, elems);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Insert(T* ptr, SizeType count, ConstIterator pos)
	{
		arr.insert(pos, ptr, ptr + count);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::AddUnique(const T& item)
	{
		if (std::find(CBegin(), CEnd(), item) == CEnd())
			Add(item);
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::AddUnique(T&& item)
	{
		if (std::find(CBegin(), CEnd(), item) == CEnd())
			Add(std::move(item));
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::Remove(const T& item)
	{
		arr.erase(std::find(CBegin(), CEnd(), item));
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::RemoveLast(const T& item)
	{
		arr.erase(std::find(CRBegin(), CREnd(), item));
	}

	template <class T, class InAllocator>
	SizeType Array<T, InAllocator>::RemoveAll(const T& item) noexcept
	{
		const auto iter = std::remove(CBegin(), CEnd(), item);
		const auto removeNum = CEnd() - iter;
		if (removeNum > 0) arr.erase(iter, CEnd());
		return removeNum;
	}

	template <class T, class InAllocator>
	void Array<T, InAllocator>::RemoveAt(SizeType index, SizeType count/* = 1*/)
	{
		auto erasePos = CBegin() + index;
		arr.erase(erasePos, erasePos + count);
	}

	template <class T, class InAllocator>
	template <class Predicate>
	SizeType Array<T, InAllocator>::RemoveByPredicate(Predicate&& pred) noexcept
	{
		const auto iter = std::remove_if(CBegin(), CEnd(), std::forward<Predicate>(pred));
		const auto removeNum = CEnd() - iter;
		if (removeNum > 0) arr.erase(iter, CEnd());
		return removeNum;
	}
}

// It's just to avoid link error.
void SolveLink()
{
	BE::Array<int, std::allocator<int>> tmp;
}