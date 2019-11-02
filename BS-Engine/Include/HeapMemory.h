#pragma once

#include "Macro.h"
#include "Type.h"

class BS_API HeapMemory final
{
public:
	bool Init(size_t size) noexcept;
	void Release() noexcept;

	void* Malloc(size_t n) noexcept;
	void Free(void* ptr, size_t n) noexcept;

private:
	uint8* memory;
	uint8* marker;

	size_t curNum;
	size_t maxNum;
	size_t markerSize;
};