#pragma once

#include "Interface.h"

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

	/// @brief Get the number of bytes of memory that assigned
	CONST_INTERFACE_DEF(size_t, GetAssignedByte);

	/// @brief Get the number of bytes of memory that can be allocated
	CONST_INTERFACE_DEF(size_t, GetAssignableByte);

	/// @brief Get the maximum memory size
	CONST_INTERFACE_DEF(size_t, GetMaxByte);
INTERFACE_END