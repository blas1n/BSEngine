#include "HeapMemory.h"
#include "MathFunctions.h"
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
	marker[index / 8] &= ~(1 << (index % 8));
}

HeapMemory::HeapMemory(const size_t size) noexcept
{
	check(size != 0);

	maxNum = size;
	markerSize = static_cast<size_t>(
		Math::Ceil(static_cast<float>(maxNum) * 0.125f));

	memory = static_cast<uint8*>(std::malloc(maxNum + markerSize));
	check(memory != nullptr);

	marker = memory + maxNum;
	std::memset(memory, 0, maxNum + markerSize);
}

HeapMemory::~HeapMemory() noexcept
{
	std::free(memory);
}

void* HeapMemory::Malloc(const size_t n) noexcept
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

void HeapMemory::Free(void* const ptr, const size_t n) noexcept
{
	check(ptr >= memory && ptr < memory + maxNum);

	curNum -= n;
	const size_t startIdx = static_cast<uint8*>(ptr) - memory;
	for (auto i = startIdx; i < n + startIdx; ++i)
		Unmark(marker, i);
}