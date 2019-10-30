#pragma once

#include "Macro.h"

class BS_API HeapMemory final
{
public:
	bool Init(size_t size) noexcept;
	void Release() noexcept;

	void* Malloc(size_t n) noexcept;

	/// @warning This method does not check the range of pointers to free.
	void Free(void* ptr, size_t n) noexcept;

private:
	class MarkerMemory* markerMemory;
};