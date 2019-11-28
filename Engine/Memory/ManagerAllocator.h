#pragma once

#include "MemoryManager.h"
#include "MemoryManagerAccesser.h"

namespace BE
{
	class BS_API ManagerAllocator final : public MemoryManagerAccesser
	{
	private:
		template <class ManagerType>
		inline void* Allocate()
		{
			return MemoryManagerAccesser::Get()
				->GetManagerMemory().Allocate<ManagerType>();
		}

		friend class System;
	};
}