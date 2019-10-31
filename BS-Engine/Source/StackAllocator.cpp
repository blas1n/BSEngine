#include "StackAllocator.h"
#include "MemoryManager.h"

/// @todo Link memory manager
StackAllocatorBase::StackAllocatorBase(size_t size, bool isSingleFrame /*= false*/) noexcept
	: memoryManager(nullptr),
	cur(nullptr),
	start(nullptr),
	maxNum(size),
	isSingleFrameAlloc(isSingleFrame)
{
	cur = start = static_cast<uint8*>(memoryManager->Allocate(this, maxNum));
}

StackAllocatorBase::StackAllocatorBase(const StackAllocatorBase& other) noexcept
	: memoryManager(other.memoryManager),
	cur(nullptr),
	start(nullptr),
	maxNum(other.maxNum),
	isSingleFrameAlloc(other.isSingleFrameAlloc)
{
	cur = start = static_cast<uint8*>(memoryManager->Allocate(this, maxNum));
}

StackAllocatorBase::StackAllocatorBase(StackAllocatorBase&& other) noexcept
	: memoryManager(std::move(other.memoryManager)),
	cur(std::move(other.cur)),
	start(std::move(other.start)),
	maxNum(std::move(other.maxNum)),
	isSingleFrameAlloc(std::move(other.isSingleFrameAlloc)) {}

StackAllocatorBase::~StackAllocatorBase() noexcept
{
	memoryManager->Deallocate(this, start, maxNum);
}

void* StackAllocatorBase::Allocate(size_t size) noexcept
{
	const auto nextCur = cur + size;
	if (nextCur > start + maxNum)
		return nullptr;

	std::memset(cur, 0, size);
	auto* const ret = cur;
	cur = nextCur;
	return ret;
}

void StackAllocatorBase::Deallocate(void* ptr, size_t size) noexcept
{
	check(cur - size == ptr);
	cur = static_cast<uint8*>(ptr);
}

void StackAllocatorBase::Clear() noexcept
{
	cur = start;
}