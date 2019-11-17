#include "Allocator.h"
#include "MemoryManager.h"

namespace BE
{
	void* AllocatorImpl::Alloc(const size_t n) noexcept
	{
		return MemoryManagerAccesser::Get()->Allocate(n);
	}

	void AllocatorImpl::Dealloc(void* ptr, const size_t n) noexcept
	{
		MemoryManagerAccesser::Get()->Deallocate(ptr, n);
	}
}