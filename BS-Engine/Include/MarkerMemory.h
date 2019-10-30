#pragma once

#include "Interface.h"
#include "Type.h"

// This architecture is really right?
class BS_API MarkerMemory
{
public:
	MarkerMemory(uint8* inMemory, uint8* inMarker,
		size_t inMemorySize, size_t inMarkerSize) noexcept;

	void* Alloc(size_t n) noexcept;
	void Dealloc(void* ptr, size_t n) noexcept;
	void Clear() noexcept;

	inline size_t GetMemorySize() const noexcept
	{
		return maxNum;
	}

	inline size_t GetMarkerSize() const noexcept
	{
		return markerSize;
	}

	inline uint8* GetMemory() const noexcept
	{
		return memory;
	}

private:
	uint8* const memory;
	uint8* const marker;

	size_t curNum;
	size_t maxNum;
	size_t markerSize;
};