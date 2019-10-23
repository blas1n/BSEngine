#pragma once

#include "Macro.h"

/**
 * @brief
 * Memory interface used by upper layers to access memory.
 * Use Malloc / Free instead of new / delete.
*/
INTERFACE_BEGIN(Memory)
	INTERFACE_DEF(void*, Malloc, size_t);
	INTERFACE_DEF(void, Free, void*);
INTERFACE_END