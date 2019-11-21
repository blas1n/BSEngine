#include "HeapMemory.h"
#include "MathFunctions.h"
#include <cstdlib>
#include <cstring>

inline bool IsAllocated(const BE::Uint8* const marker, const size_t index) noexcept
{
	return (marker[index / 8] & (1 << (index % 8))) > 0;
}

inline void Mark(BE::Uint8* const marker, const size_t index) noexcept
{
	marker[index / 8] |= 1 << (index % 8);
}

inline void Unmark(BE::Uint8* const marker, const size_t index) noexcept
{
	marker[index / 8] &= ~(1 << (index % 8));
}

namespace BE
{
	constexpr HeapMemory::HeapMemory() noexcept
		: memory(nullptr),
		marker(nullptr),
		curNum(0),
		maxNum(0) {}

	void HeapMemory::Init(void* const inMemory, const size_t inSize) noexcept
	{
		memory = static_cast<Uint8*>(inMemory);
		maxNum = inSize;

		size_t markerSize = inSize / 8 + 1;
		marker = static_cast<Uint8*>(malloc(markerSize));
	}

	void HeapMemory::Release() noexcept
	{
		if (marker)
			free(marker);
	}

	void* HeapMemory::Allocate(const size_t size)
	{
		if (size == 0 || maxNum - curNum < size)
			return nullptr;

		for (size_t i = 0, clearSectionNum = 0; i < maxNum; ++i) {
			if (IsAllocated(marker, i))
			{
				clearSectionNum = 0;
				continue;
			}

			if (++clearSectionNum < size) continue;

			const auto startIdx = i - size + 1;
			for (auto idx = startIdx; idx <= i; ++idx)
				Mark(marker, idx);

			curNum += size;
			std::memset(memory + startIdx, 0, size);
			return memory + startIdx;
		}

		return nullptr;
	}

	void HeapMemory::Deallocate(void* const ptr, const size_t size)
	{
		check(ptr >= memory && ptr < memory + maxNum);

		curNum -= size;
		const size_t startIdx = static_cast<Uint8*>(ptr) - memory;
		for (auto i = startIdx; i < size + startIdx; ++i)
			Unmark(marker, i);
	}
}