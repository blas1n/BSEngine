#include "../Public/StackMemory.h"
#include <cstdlib>

StackMemory::StackMemory(size_t size)
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

void* StackMemory::Malloc(size_t size)
{
	const auto nextCur = cur + size;

	if (nextCur > end)
		return nullptr;

	void* ret = cur;
	cur = nextCur;
	return ret;
}

void StackMemory::Free(void* ptr)
{
	if (ptr < start || ptr > cur)
	{
		// todo : If debug mode, log
		return;
	}

	cur = static_cast<byte*>(ptr);
}

void StackMemory::FreeAll()
{
	cur = start;
}