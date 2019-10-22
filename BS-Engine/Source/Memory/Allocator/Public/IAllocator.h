#pragma once

#include "Macro.h"

template <class T>
INTERFACE_BEGIN(Allocator)
	INTERFACE_DEF(T*, allocate, size_t)
	INTERFACE_DEF(void, deallocate, T*, size_t)
INTERFACE_END