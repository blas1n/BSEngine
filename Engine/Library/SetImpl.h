#pragma once

#include "Set.h"
#include "Array.h"

namespace BE
{
	template <class T, template <class> class Allocator>
	template <template <class> class ArrayAllocator>
	Set<T, Allocator>::Set(const Array<T, ArrayAllocator>& arr)
		: container(arr.begin(), arr.end()) {}

	template <class T, template <class> class Allocator>
	template <template <class> class ArrayAllocator>
	Set<T, Allocator>::Set(Array<T, ArrayAllocator>&& arr)
		: container(std::make_move_iterator(arr.begin()), std::make_move_iterator(arr.end())) {}

	template <class T, template <class> class Allocator>
	template <template <class> class OtherAllocator>
	Set<T, Allocator>::Set(const Set<T, OtherAllocator>& other)
		: container(other.begin(), other.end()) {}

	template <class T, template <class> class Allocator>
	template <template <class> class OtherAllocator>
	Set<T, Allocator>::Set(Set<T, OtherAllocator>&& other)
		: container(std::make_move_iterator(other.begin()), std::make_move_iterator(other.end())) {}

	template <class T, template <class> class Allocator>
	template <template <class> class ArrayAllocator>
	void Set<T, Allocator>::Append(const Array<T, ArrayAllocator>& arr)
	{
		container.insert(arr.begin(), arr.end());
	}

	template <class T, template <class> class Allocator>
	Array<T, Allocator> Set<T, Allocator>::ToArray() const
	{
		Array<T, Allocator> ret(GetSize());
		for (const auto& elem : *this)
			ret.Add(elem);
		return ret;
	}
}