#pragma once

#include "Interface.h"
#include <type_traits>

INTERFACE_BEGIN(AllocatorBase)
	INTERFACE_TYPE_DEF(size_type, std::size_t)
	INTERFACE_TYPE_DEF(difference_type, std::ptrdiff_t)
	INTERFACE_TYPE_DEF(propagate_on_container_move_assignment, std::true_type)
	INTERFACE_TYPE_DEF(is_always_equal, std::true_type)

	INTERFACE_DEF(void*, Allocate, size_t)
	INTERFACE_DEF(void, Deallocate, void*, size_t)
	INTERFACE_DEF(void, Clear)

	INTERFACE_CONST_DEF(size_t, GetMaxSize)
INTERFACE_END