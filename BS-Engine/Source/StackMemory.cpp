#include "StackMemory.h"
#include <memory>

StackMemory::StackMemory(size_t size) noexcept
	: cur(nullptr),
	start(nullptr),
	end(nullptr)
{
	cur = start = static_cast<byte*>(std::malloc(size));
	end = start + size;
}

StackMemory::~StackMemory()
{
	std::free(start);
}

void* StackMemory::Malloc(size_t size) noexcept
{
	const auto nextCur = cur + size;

	if (nextCur > end)
		return nullptr;

	std::memset(cur, 0, nextCur - cur);
	void* ret = cur;
	cur = nextCur;
	return ret;
}

void StackMemory::Free(void* ptr) noexcept
{
	check(ptr < start && ptr > cur);
	cur = static_cast<byte*>(ptr);
}

void StackMemory::Clear() noexcept
{
	cur = start;
}