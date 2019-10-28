#pragma once

#include "Interface.h"

/**
 * @brief
 * Allocator interface used by upper layers to access memory.
 * Use this instead of new / delete.
 * Follow C ++ standards for compatibility with STL.
 * @see https://en.cppreference.com/w/cpp/memory/allocator
*/
template <class T>
INTERFACE_BEGIN(Allocator)
	INTERFACE_TYPE_DEF(value_type, T)

	INTERFACE_DEF(T*, allocate, size_t)
	INTERFACE_DEF(void, deallocate, T*, size_t)

	template <class U, class... Args>
	INTERFACE_DEF(void, construct, U*, Args&&...)

	template <class U>
	INTERFACE_DEF(void, destroy, U*);

	INTERFACE_CONST_DEF(size_t, max_size);
INTERFACE_END