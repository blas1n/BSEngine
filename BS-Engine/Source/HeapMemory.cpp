#include "HeapMemory.h"
#include "MarkerMemory.h"
#include "MathFunctions.h"

bool HeapMemory::Init(const size_t size) noexcept
{
	const auto markerSize = static_cast<size_t>(
		Math::Ceil(static_cast<float>(size) * 0.125f));

	auto* const memory = static_cast<uint8*>(std::malloc(size + markerSize + sizeof(MarkerMemory)));
	if (memory == nullptr) return false;

	auto* const marker = memory + size;
	
#pragma push_macro("new")
#undef new
	auto* markerPtr = reinterpret_cast<MarkerMemory*>(marker + markerSize);
	markerMemory = new(markerPtr)MarkerMemory{ memory, marker, size, markerSize };
#pragma pop_macro("new")

	return true;
}

void HeapMemory::Release() noexcept
{
	auto* const memory = markerMemory->GetMemory();
	if (memory) std::free(memory);

	markerMemory->~MarkerMemory();
}

void* HeapMemory::Malloc(const size_t n) noexcept
{
	return markerMemory->Alloc(n);
}

void HeapMemory::Free(void* const ptr, const size_t n) noexcept
{
	markerMemory->Dealloc(ptr, n);
}