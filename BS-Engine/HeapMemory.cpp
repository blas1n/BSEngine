#include "HeapMemory.h"
#include "MathFunctions.h"
#include <memory>

constexpr static size_t MEMORY_SIZE = 30000;

bool HeapMemory::Init() noexcept
{
	memory = static_cast<uint8*>(std::malloc(MEMORY_SIZE));

	const auto markerSize = static_cast<size_t>(
		Math::Ceil(static_cast<float>(MEMORY_SIZE) * 0.25f));

	marker = static_cast<uint8*>(std::malloc(markerSize));

	return memory != nullptr && marker != nullptr;
}

void HeapMemory::Release() noexcept
{
	if (marker)
		std::free(marker);

	if (memory)
		std::free(memory);
}

void* HeapMemory::Allocate(size_t n) noexcept
{
	if (n == 0 || MEMORY_SIZE - curNum < n)
		return nullptr;

	for (size_t i = 0, clearSectionNum = 0; i < MEMORY_SIZE; ++i) {
		if (IsAllocated(i))
		{
			clearSectionNum = 0;
			continue;
		}

		if (++clearSectionNum < n) continue;

		auto idx = i - n + 1;
		for (; idx <= i; ++idx)
			Mark(idx);

		curNum += n;
		return memory + idx;
	}

	return nullptr;
}

void HeapMemory::Dealloate(void* ptr, size_t n) noexcept
{
	curNum -= n;
	const auto startIdx = static_cast<uint8*>(ptr) - memory;
	for (auto i = startIdx; i < n + startIdx; ++i)
		Unmark(i);
}