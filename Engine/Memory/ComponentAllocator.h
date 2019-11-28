#pragma once

#include "MemoryManager.h"
#include "MemoryManagerAccesser.h"

namespace BE
{
	class BS_API ComponentAllocator final : public MemoryManagerAccesser
	{
	public:
		template <class ComponentType>
		inline void* Allocate()
		{
			return MemoryManagerAccesser::Get()
				->GetComponentMemory().Allocate<ComponentType>();
		}

		template <class ComponentType>
		inline void Deallocate(ComponentType* const ptr)
		{
			return MemoryManagerAccesser::Get()
				->GetComponentMemory().Deallocate(ptr);
		}
	};
}