#pragma once

#include "MemoryManager.h"
#include "MemoryManagerAccesser.h"

namespace BE
{
	class BS_API ComponentAllocator final : public MemoryManagerAccesser
	{
	public:
		template <class ComponentType>
		inline void* Allocate() noexcept
		{
			return MemoryManagerAccesser::Get()
				->GetComponentMemory().Allocate<ComponentType>();
		}

		template <class ComponentType>
		inline void Deallocate(ComponentType* const ptr) noexcept
		{
			return MemoryManagerAccesser::Get()
				->GetComponentMemory().Deallocate(ptr);
		}
	};
}