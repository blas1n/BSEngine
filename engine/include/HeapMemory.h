#pragma once

#include "Macro.h"
#include "Type.h"

/// @brief Memory that can be allocated and freed.
class BS_API HeapMemory final
{
public:
	HeapMemory(size_type size) noexcept;
	~HeapMemory() noexcept;

	/**
	 * @brief Allocate memory.
	 * @param n Size to be allocated.
	 * @return Allocated pointer.
	 * @retval nullptr Can not allocate.
	*/
	void* Malloc(size_type n) noexcept;

	/**
	 * @brief Deallocate memory.
	 * @param ptr Pointer to be deallocated.
	 * @param n Size to be deallocated.
	*/
	void Free(void* ptr, size_type n) noexcept;

private:
	uint8* memory;
	uint8* marker;

	size_type curNum;
	size_type maxNum;
	size_type markerSize;
};