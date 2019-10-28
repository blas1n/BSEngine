#pragma once

#include "Core.h"

class BS_API HeapMemory final
{
public:
	bool Init() noexcept;
	void Release() noexcept;

	void* Malloc(size_t n) noexcept;

	/// @warning This method does not check the range of pointers to free.
	void Free(void* ptr, size_t n) noexcept;

private:
	inline bool IsAllocated(const size_t index) const noexcept
	{
		return (marker[index / 8] & (1 << (index % 8))) > 0;
	}

	inline void Mark(const size_t index) noexcept
	{
		marker[index / 8] |= 1 << (index % 8);
	}

	inline void Unmark(const size_t index) noexcept
	{
		marker[index / 8] &= !(1 << (index % 8));
	}

	uint8* memory;
	uint8* marker;

	size_t curNum;
};