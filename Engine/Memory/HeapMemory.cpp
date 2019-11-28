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
	void HeapMemory::Init(const size_t size)
	{
		const auto markerSize = size / 8 + 1;
		auto ptr = std::malloc(size + markerSize);
		
		if (ptr == nullptr)
		{
			throw BadAllocException
			{
				TEXT("Memory required for heap memory cannot be allocated."),
				Exception::MessageType::Shallow
			};
		}

		curMemory = static_cast<Uint8*>(ptr);
		marker = curMemory + size;
		maxNum = size;
	}

	void HeapMemory::Release() noexcept
	{
		std::free(curMemory);
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
			std::memset(curMemory + startIdx, 0, size);
			return curMemory + startIdx;
		}

		return nullptr;
	}

	void HeapMemory::Deallocate(void* const ptr, const size_t size)
	{
		if (ptr >= curMemory && ptr < curMemory + maxNum)
			throw InvalidArgumentException{ };

		curNum -= size;
		const size_t startIdx = static_cast<Uint8*>(ptr) - curMemory;
		for (auto i = startIdx; i < size + startIdx; ++i)
			Unmark(marker, i);
	}
}