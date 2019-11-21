#pragma once

#include "MemoryManager.h"

namespace BE
{
	class BS_API OneFrameAllocator final : public MemoryManagerAccesser
	{
	public:
		inline void* Allocate(const size_t n) noexcept
		{
			return MemoryManagerAccesser::Get()->GetOneFrameMemory().Allocate(n);
		}
	};
}