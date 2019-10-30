#include "MarkerMemory.h"
#include <memory>

inline bool IsAllocated(const uint8* const marker, const size_t index) noexcept
{
	return (marker[index / 8] & (1 << (index % 8))) > 0;
}

inline void Mark(uint8* const marker, const size_t index) noexcept
{
	marker[index / 8] |= 1 << (index % 8);
}

inline void Unmark(uint8* const marker, const size_t index) noexcept
{
	marker[index / 8] &= !(1 << (index % 8));
}

MarkerMemory::MarkerMemory(uint8* const inMemory, uint8* const inMarker,
	const size_t inMemorySize, const size_t inMarkerSize) noexcept
	: memory(inMemory),
	marker(inMarker),
	curNum(0),
	maxNum(inMemorySize),
	markerSize(inMarkerSize)
{
	std::memset(marker, 0, markerSize);
}

void* MarkerMemory::Alloc(const size_t n) noexcept
{
	if (n == 0 || maxNum - curNum < n)
		return nullptr;

	for (size_t i = 0, clearSectionNum = 0; i < maxNum; ++i) {
		if (IsAllocated(marker, i))
		{
			clearSectionNum = 0;
			continue;
		}

		if (++clearSectionNum < n) continue;

		const auto startIdx = i - n + 1;
		for (auto idx = startIdx; idx <= i; ++idx)
			Mark(marker, idx);

		curNum += n;
		std::memset(memory + startIdx, 0, n);
		return memory + startIdx;
	}

	return nullptr;
}

void MarkerMemory::Dealloc(void* const ptr, const size_t n) noexcept
{
	check(ptr >= memory && ptr < memory + n);

	curNum -= n;
	const size_t startIdx = static_cast<uint8*>(ptr) - memory;
	for (auto i = startIdx; i < n + startIdx; ++i)
		Unmark(marker, i);
}

void MarkerMemory::Clear() noexcept
{
	std::memset(marker, 0, markerSize);
}