#include "Allocator.h"
#include "MemoryManager.h"

namespace BE
{
	void* AllocatorImpl::Alloc(const size_t n) noexcept
	{
		return MemoryManagerAccesser::GetMemoryManager()->Allocate(n);
	}

	void AllocatorImpl::Dealloc(void* ptr, const size_t n) noexcept
	{
		MemoryManagerAccesser::GetMemoryManager()->Deallocate(ptr, n);
	}
}