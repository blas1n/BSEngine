#pragma once

#include "Array.h"
#include <algorithm>
#include <iterator>
#include <type_traits>

namespace BE
{
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
	template <class OtherElement, template <class> class OtherAllocator>
	Array<T, Allocator>::Array(const Array<OtherElement, OtherAllocator>& other)
	{
		Reserve(other.GetSize());
		for (const auto& elem : other)
			Emplace(elem);
	}

	template <class T, template <class> class Allocator>
	template <class OtherElement, template <class> class OtherAllocator>
	Array<T, Allocator>::Array(Array<OtherElement, OtherAllocator>&& other)
	{
		Reserve(other.GetSize());
		for (auto&& elem : std::move(other))
			Emplace(std::move(elem));
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Insert(const Array<T, Allocator>& other, const SizeType pos)
	{
		if (IsEmpty())
			container = other.container;
		else
			container.insert(container.cbegin() + pos,
				other.container.cbegin(), other.container.cend());
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Insert(Array<T, Allocator>&& other, const SizeType pos)
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

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Insert(const T& item, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, item);
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Insert(T&& item, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, std::move(item));
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Insert(std::initializer_list<T> elems, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, elems);
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Insert(const T* ptr, const SizeType count, const SizeType pos)
	{
		container.insert(container.cbegin() + pos, ptr, ptr + count);
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::AddUnique(const T& item)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		if (std::find(begin + , end, item) == end)
			Add(item);
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::AddUnique(T&& item)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		if (std::find(begin + , end, item) == end)
			Add(std::move(item));
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Remove(const T& item)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		container.erase(std::find(begin, end, item));
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::RemoveLast(const T& item)
	{
		const auto begin = container.crbegin();
		const auto end = container.crend();

		container.erase(std::find(begin, end, item));
	}

	template <class T, template <class> class Allocator>
	SizeType Array<T, Allocator>::RemoveAll(const T& item) noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		const auto iter = std::remove(begin, end, item);
		const auto removeNum = end - iter;
		if (removeNum > 0) container.erase(iter, end);
		return removeNum;
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::RemoveAt(const SizeType index, const SizeType count/* = 1*/)
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		auto erasePos = begin + index;
		container.erase(erasePos, erasePos + count);
	}

	template <class T, template <class> class Allocator>
	template <class Predicate>
	SizeType Array<T, Allocator>::RemoveByPredicate(Predicate&& pred) noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();

		const auto iter = std::remove_if(begin, end, std::forward<Predicate>(pred));
		const auto removeNum = end - iter;
		if (removeNum > 0) container.erase(iter, end);
		return removeNum;
	}

	template <class T, template <class> class Allocator>
	bool Array<T, Allocator>::Find(const T& value, SizeType& index) const noexcept
	{
		index = Find(value);
		return index == GetSize() + 1;
	}

	template <class T, template <class> class Allocator>
	SizeType Array<T, Allocator>::Find(const T& value) const noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();
		return std::find(begin, end, value) - begin;
	}

	template <class T, template <class> class Allocator>
	bool Array<T, Allocator>::FindLast(const T& value, SizeType& index) const noexcept
	{
		index = FindLast(value);
		return index == GetSize() + 1;
	}

	template <class T, template <class> class Allocator>
	SizeType Array<T, Allocator>::FindLast(const T& value) const noexcept
	{
		const auto begin = container.crbegin();
		const auto end = container.crend();
		return std::find(begin, end, value) - begin;
	}

	template <class T, template <class> class Allocator>
	Array<T&, Allocator> Array<T, Allocator>::FindAll(const T& value) noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find(iter, end, value);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, template <class> class Allocator>
	Array<const T&, Allocator> Array<T, Allocator>::FindAll(const T& value) const noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find(iter, end, value);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	bool Array<T, Allocator>::Find(Pred&& pred, SizeType& index) const noexcept
	{
		index = Find(std::forward<Pred>(pred));
		return index == GetSize() + 1;
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	SizeType Array<T, Allocator>::Find(Pred&& pred) const noexcept
	{
		const auto begin = container.cbegin();
		const auto end = container.cend();
		return std::find_if(begin, end, pred) - begin;
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	bool Array<T, Allocator>::FindLast(Pred&& pred, SizeType& index) const noexcept
	{
		index = FindLast(std::forward<Pred>(pred));
		return index == GetSize() + 1;
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	SizeType Array<T, Allocator>::FindLast(Pred&& pred) const noexcept
	{
		const auto begin = container.crbegin();
		const auto end = container.crend();
		return std::find_if(begin, end, pred) - begin;
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	Array<T&, Allocator> Array<T, Allocator>::FindAll(Pred&& pred) noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find(iter, end, pred);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	Array<const T&, Allocator> Array<T, Allocator>::FindAll(Pred&& pred) const noexcept
	{
		Array<T, Allocator> ret;

		const auto iter = container.begin();
		const auto end = container.end();

		while (true)
		{
			iter = std::find_if(iter, end, pred);

			if (iter != end)
				ret.Add(*iter);
			else
				break;
		}

		return ret;
	}

	template <class T, template <class> class Allocator>
	void Array<T, Allocator>::Sort() noexcept
	{
		std::sort(begin(), end());
	}

	template <class T, template <class> class Allocator>
	template <class Pred>
	void Array<T, Allocator>::Sort(Pred&& pred) noexcept
	{
		std::sort(begin(), end(), std::forward<Pred>(pred));
	}
}