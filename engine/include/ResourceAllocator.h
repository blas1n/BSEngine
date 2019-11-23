#pragma once

#include "MemoryManager.h"
#include "MemoryManagerAccesser.h"

namespace BE
{
	class BS_API ResourceAllocator : public MemoryManagerAccesser
	{
	public:
		inline void* Allocate(const size_t n) noexcept
		{
			return MemoryManagerAccesser::Get()->GetResourceMemory().Allocate(n);
		}

		inline void Deallocate(void* const ptr, const size_t n = 1) noexcept
		{
			MemoryManagerAccesser::Get()->GetResourceMemory().Deallocate(ptr, n);
		}
	};
}