#pragma once

#include "IMemory.h"
#include <limits>

/// @brief Memory with fixed order of allocation and deallocation
class BS_API StackMemory final : public IMemory
{
public:
	StackMemory(size_t size) noexcept;
	~StackMemory();

	/**
	 * @brief Allocate new memory.
	 * @param[in] size How much memory to allocate.
	 * @return Address of allocated memory.
	 * @retval nullptr Returned when the memory is full and can no longer be allocated.
	*/
	void* Malloc(size_t size) override;

	/**
	 * @brief Free all memory up to the argument.
	 * @param[in] ptr New top pointer
	 * @warning If you try to free a pointer that is not at the top, all the pointers above it are freed.
	*/
	void Free(void* ptr) override;

	/// @brief Clear memory
	void Clear();

private:
	using byte = unsigned char;

	byte* cur;
	byte* start;
	byte* end;
};