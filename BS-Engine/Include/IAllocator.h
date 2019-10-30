#pragma once

#include "Interface.h"
#include <type_traits>

template <class T>
INTERFACE_BEGIN(Allocator)
	template <class U>
	INTERFACE_STRUCT_DEF(rebind) {};

	INTERFACE_TYPE_DEF(value_type, T);
	INTERFACE_TYPE_DEF(size_type, std::size_t)
	INTERFACE_TYPE_DEF(difference_type, std::ptrdiff_t)
	INTERFACE_TYPE_DEF(propagate_on_container_move_assignment, std::true_type)
	INTERFACE_TYPE_DEF(is_always_equal, std::true_type)

	INTERFACE_DEF(T*, allocate, size_t)
	INTERFACE_DEF(void, deallocate, T*, size_t)
	INTERFACE_DEF(void, clear)

	INTERFACE_CONST_DEF(size_t, max_size)
INTERFACE_END