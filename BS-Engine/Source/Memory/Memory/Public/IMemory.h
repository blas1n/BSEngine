#pragma once

#include "Macro.h"

/**
 * @brief
 * Memory interface used by upper layers to access memory.
 * Use Malloc / Free instead of new / delete.
*/
INTERFACE_BEGIN(Memory)
	/// @brief Allocate new memory.
	INTERFACE_DEF(void*, Malloc, size_t);

	/// @brief Free all memory up to the argument.
	INTERFACE_DEF(void, Free, void*);

	/// @brief Clear memory
	INTERFACE_DEF(void, Clear);
INTERFACE_END